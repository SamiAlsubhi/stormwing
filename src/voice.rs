//! Spoken callouts for major events ("Hull critical", "Shield online"...).
//!
//! The lines were recorded ahead of time with the Piper text-to-speech
//! engine (see `tools/record_voice.py`) and ship inside the game. They play
//! through the game's own mixer, over the music, which dips while a line is
//! being spoken.

use crate::audio::RATE;
use crate::save::Profile;
#[cfg(not(target_arch = "wasm32"))]
use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Line {
    MissionStart,
    PowerUp,
    MaxPower,
    Shield,
    Repair,
    BombReady,
    BombAway,
    HullCritical,
    BossWarning,
    CoreExposed,
    MissionComplete,
    MissionFailed,
    Rescued,
}

impl Line {
    /// Urgent lines cut off whatever is being said.
    fn urgent(self) -> bool {
        matches!(
            self,
            Line::HullCritical | Line::BossWarning | Line::MissionComplete | Line::MissionFailed
        )
    }

    fn index(self) -> usize {
        self as usize
    }
}

const LINE_COUNT: usize = 13;
const VOICE_VOLUME: f32 = 0.95;

/// The recordings, in `Line` order.
const CLIPS: [&[u8]; LINE_COUNT] = [
    include_bytes!("../assets/voice/mission_start.wav"),
    include_bytes!("../assets/voice/power_up.wav"),
    include_bytes!("../assets/voice/max_power.wav"),
    include_bytes!("../assets/voice/shield.wav"),
    include_bytes!("../assets/voice/repair.wav"),
    include_bytes!("../assets/voice/bomb_ready.wav"),
    include_bytes!("../assets/voice/bomb_away.wav"),
    include_bytes!("../assets/voice/hull_critical.wav"),
    include_bytes!("../assets/voice/boss_warning.wav"),
    include_bytes!("../assets/voice/core_exposed.wav"),
    include_bytes!("../assets/voice/mission_complete.wav"),
    include_bytes!("../assets/voice/mission_failed.wav"),
    include_bytes!("../assets/voice/rescued.wav"),
];

#[derive(Message, Clone, Copy)]
pub struct Say(pub Line);

#[derive(Resource)]
pub struct VoiceState {
    last: [f64; LINE_COUNT],
    /// Clip lengths in seconds.
    lengths: Vec<f64>,
    /// When the line being spoken ends (real time).
    until: f64,
    was_muted: bool,
    #[cfg(not(target_arch = "wasm32"))]
    clips: Vec<Handle<AudioSource>>,
    #[cfg(not(target_arch = "wasm32"))]
    current: Option<Entity>,
}

impl VoiceState {
    /// Whether a line is being spoken at `now` (real time).
    pub fn speaking(&self, now: f64) -> bool {
        now < self.until
    }
}

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Say>()
            .insert_resource(VoiceState {
                last: [-100.0; LINE_COUNT],
                lengths: Vec::new(),
                until: 0.0,
                was_muted: false,
                #[cfg(not(target_arch = "wasm32"))]
                clips: Vec::new(),
                #[cfg(not(target_arch = "wasm32"))]
                current: None,
            })
            .add_systems(Startup, load_clips)
            .add_systems(Update, speak);
    }
}

fn load_clips(
    mut state: ResMut<VoiceState>,
    #[cfg(not(target_arch = "wasm32"))] mut sources: ResMut<Assets<AudioSource>>,
) {
    let clips: Vec<Vec<f32>> = CLIPS.iter().map(|bytes| radio(&decode(bytes))).collect();
    state.lengths = clips.iter().map(|c| c.len() as f64 / RATE as f64).collect();
    #[cfg(not(target_arch = "wasm32"))]
    {
        state.clips = clips
            .iter()
            .map(|c| sources.add(AudioSource { bytes: crate::audio::wav(c).into() }))
            .collect();
    }
    #[cfg(target_arch = "wasm32")]
    crate::audio::web::store_voices(clips);
}

fn speak(
    mut commands: Commands,
    mut reader: MessageReader<Say>,
    mut state: ResMut<VoiceState>,
    profile: Res<Profile>,
    time: Res<Time<Real>>,
    sinks: Query<&AudioSink>,
) {
    let now = time.elapsed_secs_f64();
    if profile.muted && !state.was_muted {
        stop(&mut commands, &mut state, &sinks);
    }
    state.was_muted = profile.muted;
    for Say(line) in reader.read() {
        if profile.muted || state.lengths.is_empty() {
            continue;
        }
        // Don't repeat the same line too often.
        if now - state.last[line.index()] < 3.0 {
            continue;
        }
        let busy = state.speaking(now);
        if busy && !line.urgent() {
            continue;
        }
        if busy {
            stop(&mut commands, &mut state, &sinks);
        }
        let i = line.index();
        state.last[i] = now;
        state.until = now + state.lengths[i];
        #[cfg(not(target_arch = "wasm32"))]
        {
            let entity = commands
                .spawn((
                    AudioPlayer(state.clips[i].clone()),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOICE_VOLUME)),
                ))
                .id();
            state.current = Some(entity);
        }
        #[cfg(target_arch = "wasm32")]
        crate::audio::web::play_voice(i, VOICE_VOLUME);
    }
}

fn stop(commands: &mut Commands, state: &mut VoiceState, sinks: &Query<&AudioSink>) {
    state.until = 0.0;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(entity) = state.current.take() {
        if let Ok(sink) = sinks.get(entity) {
            sink.stop();
        }
        if let Ok(mut e) = commands.get_entity(entity) {
            e.despawn();
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (commands, sinks);
        crate::audio::web::stop_voice();
    }
}

/// Reads a 16-bit PCM WAV file into mono samples at the game's sample rate.
fn decode(bytes: &[u8]) -> Vec<f32> {
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Vec::new();
    }
    let (mut channels, mut rate, mut data) = (1usize, RATE, &bytes[0..0]);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32_at(at + 4) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        if id == b"fmt " && body.len() >= 16 {
            channels = u16_at(at + 10).max(1) as usize;
            rate = u32_at(at + 12);
        } else if id == b"data" {
            data = body;
        }
        at += 8 + len + (len & 1);
    }
    let source: Vec<f32> = data
        .chunks_exact(2 * channels)
        .map(|frame| i16::from_le_bytes([frame[0], frame[1]]) as f32 / 32768.0)
        .collect();
    // Linear resampling to the mixer's rate.
    let step = rate as f64 / RATE as f64;
    let len = ((source.len() as f64) / step) as usize;
    (0..len)
        .map(|i| {
            let x = i as f64 * step;
            let k = x as usize;
            let f = (x - k as f64) as f32;
            let a = source[k.min(source.len() - 1)];
            let b = source[(k + 1).min(source.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

/// Trims the silence, then gives the voice a light cockpit-radio colour:
/// thinner lows, a little compression and a soft squelch at either end.
fn radio(samples: &[f32]) -> Vec<f32> {
    let loud = |s: &f32| s.abs() > 0.02;
    let (Some(first), Some(last)) = (samples.iter().position(loud), samples.iter().rposition(loud)) else {
        return Vec::new();
    };
    let pad = (RATE as f32 * 0.03) as usize;
    let voice = &samples[first.saturating_sub(pad)..(last + pad).min(samples.len())];

    // One-pole high-pass (~220 Hz) and low-pass (~5.5 kHz).
    let dt = 1.0 / RATE as f32;
    let hp_a = 1.0 / (1.0 + dt * std::f32::consts::TAU * 220.0);
    let lp_a = {
        let rc = 1.0 / (std::f32::consts::TAU * 5500.0);
        dt / (rc + dt)
    };
    let (mut prev_in, mut hp, mut lp) = (0.0f32, 0.0f32, 0.0f32);
    let mut out: Vec<f32> = voice
        .iter()
        .map(|&s| {
            hp = hp_a * (hp + s - prev_in);
            prev_in = s;
            lp += lp_a * (hp - lp);
            lp
        })
        .collect();
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-3);
    let drive = 1.8f32;
    let norm = drive.tanh();
    for s in &mut out {
        *s = (*s / peak * drive).tanh() / norm * 0.88;
    }

    // Squelch: a short burst of hiss as the channel opens and closes.
    let mut noise = 0x2545_f491u32;
    let mut hiss = move || {
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        (noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let open = (RATE as f32 * 0.035) as usize;
    let close = (RATE as f32 * 0.07) as usize;
    let mut clip = Vec::with_capacity(open + out.len() + close);
    clip.extend((0..open).map(|i| hiss() * 0.05 * (1.0 - i as f32 / open as f32)));
    clip.extend(out);
    clip.extend((0..close).map(|i| {
        let t = i as f32 / close as f32;
        hiss() * 0.07 * (t * 12.0).min(1.0) * (1.0 - t).powi(2)
    }));
    clip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_clip_decodes_to_a_short_callout() {
        for (i, bytes) in CLIPS.iter().enumerate() {
            let clip = radio(&decode(bytes));
            let secs = clip.len() as f32 / RATE as f32;
            let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((0.4..4.0).contains(&secs), "clip {i} lasts {secs} s");
            assert!((0.5..=1.0).contains(&peak), "clip {i} peaks at {peak}");
        }
    }
}
