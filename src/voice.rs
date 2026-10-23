//! Spoken callouts for major events ("Hull critical", "Shield online"...).
//!
//! The web build speaks through the browser's speech synthesis; desktop
//! builds use the operating system's own text-to-speech. No voice recordings
//! ship with the game.

use crate::save::Profile;
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
    fn text(self) -> &'static str {
        match self {
            Line::MissionStart => "Mission start. Good hunting.",
            Line::PowerUp => "Weapons upgraded.",
            Line::MaxPower => "Maximum firepower.",
            Line::Shield => "Shield online.",
            Line::Repair => "Hull repaired.",
            Line::BombReady => "Bomb ready.",
            Line::BombAway => "Bomb away!",
            Line::HullCritical => "Warning. Hull integrity critical.",
            Line::BossWarning => "Warning. Large hostile approaching.",
            Line::CoreExposed => "Core exposed. Fire at will.",
            Line::MissionComplete => "Target destroyed. Mission complete.",
            Line::MissionFailed => "Mission failed.",
            Line::Rescued => "Survivor rescued.",
        }
    }

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

#[derive(Message, Clone, Copy)]
pub struct Say(pub Line);

#[derive(Resource)]
struct VoiceState {
    last: [f64; LINE_COUNT],
    was_muted: bool,
    #[cfg(not(target_arch = "wasm32"))]
    process: Option<std::process::Child>,
}

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Say>()
            .insert_resource(VoiceState {
                last: [-100.0; LINE_COUNT],
                was_muted: false,
                #[cfg(not(target_arch = "wasm32"))]
                process: None,
            })
            .add_systems(Update, speak);
    }
}

fn speak(
    mut reader: MessageReader<Say>,
    mut state: ResMut<VoiceState>,
    profile: Res<Profile>,
    time: Res<Time<Real>>,
) {
    if profile.muted && !state.was_muted {
        stop(&mut state);
    }
    state.was_muted = profile.muted;
    let now = time.elapsed_secs_f64();
    for Say(line) in reader.read() {
        if profile.muted {
            continue;
        }
        // Don't repeat the same line too often.
        if now - state.last[line.index()] < 3.0 {
            continue;
        }
        let busy = is_speaking(&mut state);
        if busy && !line.urgent() {
            continue;
        }
        if busy {
            stop(&mut state);
        }
        state.last[line.index()] = now;
        say(&mut state, line.text());
    }
}

#[cfg(target_arch = "wasm32")]
fn synth() -> Option<web_sys::SpeechSynthesis> {
    web_sys::window()?.speech_synthesis().ok()
}

#[cfg(target_arch = "wasm32")]
fn is_speaking(_: &mut VoiceState) -> bool {
    synth().is_some_and(|s| s.speaking() || s.pending())
}

#[cfg(target_arch = "wasm32")]
fn stop(_: &mut VoiceState) {
    if let Some(s) = synth() {
        s.cancel();
    }
}

#[cfg(target_arch = "wasm32")]
fn say(_: &mut VoiceState, text: &str) {
    use wasm_bindgen::JsCast;
    let Some(synth) = synth() else { return };
    let Ok(utterance) = web_sys::SpeechSynthesisUtterance::new_with_text(text) else {
        return;
    };
    utterance.set_rate(1.05);
    utterance.set_pitch(0.9);
    utterance.set_volume(0.95);
    // Prefer a clear English voice when the browser offers a choice.
    let preferred = ["Google UK English Male", "Daniel", "Google US English", "Alex", "Samantha"];
    let voices: Vec<web_sys::SpeechSynthesisVoice> = synth
        .get_voices()
        .iter()
        .filter_map(|v| v.dyn_into::<web_sys::SpeechSynthesisVoice>().ok())
        .filter(|v| v.lang().starts_with("en"))
        .collect();
    let chosen = preferred
        .iter()
        .find_map(|name| voices.iter().find(|v| v.name().contains(name)))
        .or_else(|| voices.first());
    if let Some(voice) = chosen {
        utterance.set_voice(Some(voice));
    }
    synth.speak(&utterance);
}

#[cfg(not(target_arch = "wasm32"))]
fn is_speaking(state: &mut VoiceState) -> bool {
    match state.process.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(None)),
        None => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn stop(state: &mut VoiceState) {
    if let Some(mut child) = state.process.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn say(state: &mut VoiceState, text: &str) {
    use std::process::{Command, Stdio};
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("say");
        c.args(["-r", "185", text]);
        c
    } else if cfg!(target_os = "windows") {
        let script = format!(
            "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Speak('{}')",
            text.replace('\'', "''")
        );
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script]);
        c
    } else {
        let mut c = Command::new("spd-say");
        c.arg(text);
        c
    };
    // No text-to-speech available is fine: the game just stays quiet.
    state.process = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok();
}
