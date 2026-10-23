//! Sound effects and music, synthesised at startup into in-memory WAV files.

use crate::save::Profile;
use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use std::f32::consts::TAU;

const RATE: u32 = 44_100;
const MUSIC_VOLUME: f32 = 0.42;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Shoot,
    EnemyShoot,
    Hit,
    ExplodeSmall,
    ExplodeBig,
    Star,
    PowerUp,
    Hurt,
    Bomb,
    Warning,
    Click,
    Rescue,
    Shield,
    Missile,
    Denied,
}

const SFX_COUNT: usize = 15;

impl Sfx {
    /// Base volume and the minimum gap between two plays of the same sound.
    fn mix(self) -> (f32, f32) {
        match self {
            Sfx::Shoot => (0.16, 0.075),
            Sfx::EnemyShoot => (0.18, 0.09),
            Sfx::Hit => (0.22, 0.045),
            Sfx::ExplodeSmall => (0.55, 0.05),
            Sfx::ExplodeBig => (0.8, 0.12),
            Sfx::Star => (0.22, 0.035),
            Sfx::PowerUp => (0.5, 0.1),
            Sfx::Hurt => (0.6, 0.1),
            Sfx::Bomb => (0.9, 0.2),
            Sfx::Warning => (0.5, 1.0),
            Sfx::Click => (0.4, 0.03),
            Sfx::Rescue => (0.5, 0.1),
            Sfx::Shield => (0.45, 0.1),
            Sfx::Missile => (0.18, 0.08),
            Sfx::Denied => (0.4, 0.1),
        }
    }
}

#[derive(Message, Clone, Copy)]
pub struct PlaySfx {
    pub sfx: Sfx,
    pub volume: f32,
    pub pitch: f32,
}

impl PlaySfx {
    pub fn new(sfx: Sfx) -> Self {
        Self {
            sfx,
            volume: 1.0,
            pitch: 1.0,
        }
    }
    pub fn pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch;
        self
    }
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }
}

#[derive(Resource)]
struct SoundBank {
    sfx: Vec<Handle<AudioSource>>,
    last_played: [f64; SFX_COUNT],
}

#[derive(Component)]
struct Music;

pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .add_systems(Startup, synthesize)
            .add_systems(Update, (play_sfx, toggle_mute, sync_music_volume));
    }
}

fn synthesize(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |samples: Vec<f32>| {
        sources.add(AudioSource {
            bytes: wav(&samples).into(),
        })
    };
    // Order must match the `Sfx` enum.
    let sfx = vec![
        add(sfx_shoot()),
        add(sfx_enemy_shoot()),
        add(sfx_hit()),
        add(sfx_explosion(0.55, 7.0, 0.9, 1)),
        add(sfx_explosion(1.7, 2.4, 0.97, 2)),
        add(sfx_star()),
        add(sfx_arpeggio(&[523.3, 659.3, 784.0, 1046.5], 0.085)),
        add(sfx_hurt()),
        add(sfx_bomb()),
        add(sfx_warning()),
        add(sfx_click()),
        add(sfx_arpeggio(&[784.0, 987.8, 1174.7, 1568.0, 1975.5], 0.07)),
        add(sfx_shield()),
        add(sfx_missile()),
        add(sfx_denied()),
    ];
    debug_assert_eq!(sfx.len(), SFX_COUNT);
    let music = add(music_loop());
    commands.insert_resource(SoundBank {
        sfx,
        last_played: [-10.0; SFX_COUNT],
    });
    commands.spawn((
        Music,
        AudioPlayer(music),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(MUSIC_VOLUME)),
    ));
}

fn play_sfx(
    mut commands: Commands,
    mut reader: MessageReader<PlaySfx>,
    bank: Option<ResMut<SoundBank>>,
    profile: Res<Profile>,
    time: Res<Time<Real>>,
) {
    let Some(mut bank) = bank else {
        reader.clear();
        return;
    };
    let now = time.elapsed_secs_f64();
    for msg in reader.read() {
        if profile.muted {
            continue;
        }
        let i = msg.sfx as usize;
        let (base, gap) = msg.sfx.mix();
        if now - bank.last_played[i] < gap as f64 {
            continue;
        }
        bank.last_played[i] = now;
        commands.spawn((
            AudioPlayer(bank.sfx[i].clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(base * msg.volume))
                .with_speed(msg.pitch),
        ));
    }
}

fn toggle_mute(keys: Res<ButtonInput<KeyCode>>, mut profile: ResMut<Profile>) {
    if keys.just_pressed(KeyCode::KeyM) {
        profile.muted = !profile.muted;
        profile.save();
    }
}

fn sync_music_volume(profile: Res<Profile>, mut sinks: Query<&mut AudioSink, With<Music>>) {
    let want = if profile.muted { 0.0 } else { MUSIC_VOLUME };
    for mut sink in &mut sinks {
        if (sink.volume().to_linear() - want).abs() > 1e-3 {
            sink.set_volume(Volume::Linear(want));
        }
    }
}

// ---------------------------------------------------------------------------
// Synthesis helpers
// ---------------------------------------------------------------------------

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

fn samples(dur: f32) -> usize {
    (dur * RATE as f32) as usize
}

fn square(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty { 1.0 } else { -1.0 }
}

fn tri(phase: f32) -> f32 {
    4.0 * (phase.fract() - 0.5).abs() - 1.0
}

fn saw(phase: f32) -> f32 {
    2.0 * phase.fract() - 1.0
}

fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

fn sfx_shoot() -> Vec<f32> {
    let n = samples(0.08);
    let mut noise = Noise(7);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 1700.0 * (-t * 30.0).exp() + 420.0;
            phase += f / RATE as f32;
            let env = (1.0 - t / 0.08).max(0.0).powi(2);
            (square(phase, 0.3) * 0.6 + noise.next() * 0.25) * env * 0.5
        })
        .collect()
}

fn sfx_enemy_shoot() -> Vec<f32> {
    let n = samples(0.1);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 760.0 * (-t * 14.0).exp() + 240.0;
            phase += f / RATE as f32;
            let env = (1.0 - t / 0.1).max(0.0);
            tri(phase) * env * 0.5
        })
        .collect()
}

fn sfx_hit() -> Vec<f32> {
    let n = samples(0.05);
    let mut noise = Noise(99);
    let mut prev = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let s = noise.next();
            let hp = s - prev;
            prev = s;
            hp * (1.0 - t / 0.05).max(0.0) * 0.5
        })
        .collect()
}

/// Low rumbling noise burst with a pitch-dropping thump underneath.
fn sfx_explosion(dur: f32, decay: f32, smooth: f32, seed: u32) -> Vec<f32> {
    let n = samples(dur);
    let mut noise = Noise(seed * 7919);
    let mut lp = 0.0;
    let mut lp2 = 0.0;
    let mut phase = 0.0;
    let mut crackle = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let s = noise.next();
            lp = lp * smooth + s * (1.0 - smooth);
            lp2 = lp2 * 0.6 + lp * 0.4;
            let rumble = lp2 * (1.0 / (1.0 - smooth)).sqrt() * 0.9;
            let f = 120.0 * (-t * 5.0).exp() + 28.0;
            phase += f / RATE as f32;
            let thump = (phase * TAU).sin() * (-t * 7.0).exp();
            if noise.next() > 0.9985 - 0.002 * (-t * 3.0).exp() {
                crackle = 0.6;
            }
            crackle *= 0.985;
            let attack = (t / 0.004).min(1.0);
            let env = (-t * decay).exp() * attack;
            ((rumble + crackle * noise.next() * 0.5) * env + thump * 0.8).tanh() * 0.8
        })
        .collect()
}

fn sfx_star() -> Vec<f32> {
    let n = samples(0.15);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = if t < 0.045 { 1568.0 } else { 2093.0 };
            phase += f / RATE as f32;
            let env = if t < 0.045 { 1.0 } else { (-(t - 0.045) * 26.0).exp() };
            ((phase * TAU).sin() + 0.3 * (phase * 2.0 * TAU).sin()) * env * 0.4
        })
        .collect()
}

fn sfx_arpeggio(notes: &[f32], step: f32) -> Vec<f32> {
    let total = step * notes.len() as f32 + 0.25;
    let n = samples(total);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let k = ((t / step) as usize).min(notes.len() - 1);
            let local = t - k as f32 * step;
            phase += notes[k] / RATE as f32;
            let env = (-local * 9.0).exp() * if k == notes.len() - 1 { 1.0 } else { 0.9 };
            (tri(phase) * 0.7 + square(phase, 0.25) * 0.2) * env * 0.45
        })
        .collect()
}

fn sfx_hurt() -> Vec<f32> {
    let n = samples(0.4);
    let mut noise = Noise(3);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 280.0 * (-t * 6.0).exp() + 55.0;
            phase += f / RATE as f32;
            let env = (-t * 6.0).exp();
            ((saw(phase) + noise.next() * 0.4) * 2.0).tanh() * env * 0.5
        })
        .collect()
}

fn sfx_bomb() -> Vec<f32> {
    let n = samples(2.2);
    let mut noise = Noise(1234);
    let mut lp = 0.0;
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            lp = lp * 0.985 + noise.next() * 0.015;
            let f = 150.0 * (-t * 2.5).exp() + 22.0;
            phase += f / RATE as f32;
            let sub = (phase * TAU).sin() * (-t * 1.8).exp();
            let env = (-t * 1.6).exp() * (t / 0.01).min(1.0);
            ((lp * 9.0 * env) + sub).tanh() * 0.85
        })
        .collect()
}

fn sfx_warning() -> Vec<f32> {
    let n = samples(1.8);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = if ((t / 0.225) as usize) % 2 == 0 { 520.0 } else { 390.0 };
            phase += f / RATE as f32;
            let gate = (1.0 - (t % 0.45) / 0.45).powf(0.3);
            let env = (1.0 - t / 1.8).max(0.0).powf(0.5);
            (square(phase, 0.5) * 0.35 + tri(phase) * 0.65) * gate * env * 0.4
        })
        .collect()
}

fn sfx_click() -> Vec<f32> {
    let n = samples(0.05);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            (t * 1100.0 * TAU).sin() * (-t * 80.0).exp() * 0.5
        })
        .collect()
}

fn sfx_shield() -> Vec<f32> {
    let n = samples(0.4);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 300.0 + 1100.0 * (t / 0.4) + (t * 40.0 * TAU).sin() * 30.0;
            phase += f / RATE as f32;
            let env = (1.0 - t / 0.4).max(0.0);
            (phase * TAU).sin() * env * 0.45
        })
        .collect()
}

fn sfx_missile() -> Vec<f32> {
    let n = samples(0.3);
    let mut noise = Noise(55);
    let mut lp = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let k = 0.6 + 0.35 * (t / 0.3);
            lp = lp * (1.0 - k * 0.5) + noise.next() * k * 0.5;
            let env = (t / 0.03).min(1.0) * (1.0 - t / 0.3).max(0.0);
            lp * env * 0.7
        })
        .collect()
}

fn sfx_denied() -> Vec<f32> {
    let n = samples(0.22);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            phase += 150.0 / RATE as f32;
            let gate = if t < 0.08 || (t > 0.12 && t < 0.2) { 1.0 } else { 0.0 };
            square(phase, 0.5) * gate * 0.3
        })
        .collect()
}

/// An original looping chiptune: bass, arpeggio, pad and drums over an
/// eight-bar minor progression. Note tails wrap around so the loop is seamless.
fn music_loop() -> Vec<f32> {
    let bpm = 140.0;
    let step = 60.0 / bpm / 4.0; // sixteenth note
    let bars = 8;
    let steps = bars * 16;
    let len = samples(step * steps as f32);
    let mut buf = vec![0.0f32; len];

    // Root note (MIDI) and chord tones for each bar.
    let chords: [(f32, [f32; 3]); 8] = [
        (45.0, [57.0, 60.0, 64.0]), // Am
        (41.0, [57.0, 60.0, 65.0]), // F
        (48.0, [60.0, 64.0, 67.0]), // C
        (43.0, [59.0, 62.0, 67.0]), // G
        (45.0, [57.0, 60.0, 64.0]), // Am
        (41.0, [57.0, 60.0, 65.0]), // F
        (43.0, [59.0, 62.0, 67.0]), // G
        (40.0, [56.0, 59.0, 64.0]), // E
    ];

    let add_note = |buf: &mut Vec<f32>, start: f32, dur: f32, voice: &dyn Fn(f32, f32) -> f32| {
        let s0 = samples(start);
        let n = samples(dur);
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let idx = (s0 + i) % len;
            buf[idx] += voice(t, dur);
        }
    };

    let mut noise = Noise(42);
    let noise_table: Vec<f32> = (0..samples(0.3)).map(|_| noise.next()).collect();

    for (bar, (root, tones)) in chords.iter().enumerate() {
        let bar_start = bar as f32 * 16.0 * step;

        // Bass: driving eighth notes alternating root and octave.
        for e in 0..8 {
            let note = if e % 2 == 0 { *root } else { root + 12.0 };
            let f = midi(note);
            let voice = move |t: f32, d: f32| {
                let env = (-t * 7.0).exp() * (1.0 - t / d).max(0.0);
                let s = saw(t * f) * 0.6 + square(t * f, 0.5) * 0.25;
                s * env * 0.32
            };
            add_note(&mut buf, bar_start + e as f32 * 2.0 * step, step * 1.9, &voice);
        }

        // Arpeggio: sixteenth notes walking up and down the chord.
        let pattern = [0, 1, 2, 3, 4, 3, 2, 1, 0, 2, 4, 5, 4, 2, 3, 1];
        for (s, &p) in pattern.iter().enumerate() {
            let note = tones[p % 3] + 12.0 * (p / 3) as f32 + 12.0;
            let f = midi(note);
            // Second half of the loop gets a brighter arpeggio.
            let duty = if bar >= 4 { 0.25 } else { 0.5 };
            let voice = move |t: f32, _d: f32| {
                let env = (-t * 16.0).exp();
                (square(t * f, duty) * 0.5 + tri(t * f) * 0.5) * env * 0.085
            };
            add_note(&mut buf, bar_start + s as f32 * step, step * 1.5, &voice);
        }

        // Pad: soft sustained chord.
        for tone in tones {
            let f = midi(*tone);
            let voice = move |t: f32, d: f32| {
                let env = (t / 0.15).min(1.0) * (1.0 - t / d).max(0.0).powf(0.6);
                (tri(t * f) + tri(t * f * 1.003) * 0.6) * env * 0.035
            };
            add_note(&mut buf, bar_start, 16.0 * step, &voice);
        }

        // Drums.
        for s in 0..16 {
            let at = bar_start + s as f32 * step;
            let kick = s == 0 || s == 8 || (s == 10 && bar % 2 == 1) || (s == 14 && bar == 7);
            if kick {
                let voice = |t: f32, _d: f32| {
                    // Phase of a 140 Hz -> 42 Hz exponential sweep, integrated analytically.
                    let phase = 42.0 * t + 140.0 * (1.0 - (-t * 22.0).exp()) / 22.0;
                    (phase * TAU).sin() * (-t * 9.0).exp() * 0.55
                };
                add_note(&mut buf, at, 0.3, &voice);
            }
            if s == 4 || s == 12 {
                let table = &noise_table;
                let voice = move |t: f32, _d: f32| {
                    let n = table[(t * RATE as f32) as usize % table.len()];
                    let tone = (t * 190.0 * TAU).sin() * (-t * 25.0).exp();
                    (n * (-t * 14.0).exp() * 0.32) + tone * 0.18
                };
                add_note(&mut buf, at, 0.22, &voice);
            }
            if s % 2 == 0 {
                let table = &noise_table;
                let open = s % 4 == 2;
                let voice = move |t: f32, _d: f32| {
                    let i = (t * RATE as f32) as usize;
                    let n = table[i % table.len()] - table[(i + 1) % table.len()];
                    n * (-t * if open { 30.0 } else { 70.0 }).exp() * 0.07
                };
                add_note(&mut buf, at, 0.12, &voice);
            }
        }
    }

    let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-3);
    buf.iter().map(|s| (s / peak * 0.85).tanh()).collect()
}

/// Encodes mono samples as a 16-bit PCM WAV file.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}
