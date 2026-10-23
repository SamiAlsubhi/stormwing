//! Developer harness driven by environment variables. Handy for testing and
//! automated screenshots; it does nothing unless one of these is set:
//!
//! - `STORMWING_STAGE=1..3`  jump straight into a stage
//! - `STORMWING_SKIP=secs`   fast-forward the stage script (e.g. to the boss)
//! - `STORMWING_GOD=1`       invulnerable player flown by a simple autopilot
//! - `STORMWING_SCREEN=hangar|select|results` open a menu screen on launch
//! - `STORMWING_STARS=n`     grant stars (also unlocks every stage)
//! - `STORMWING_SHOTS=dir`   save screenshots into `dir` ...
//! - `STORMWING_SHOT_AT=3,8` ... at these times (seconds since launch)
//! - `STORMWING_QUIT=secs`   exit after this many seconds
//! - `STORMWING_MUTE=1`      start muted
//! - `STORMWING_FPS=1`       log frame rate to the console
//! - `STORMWING_PROFILE=path` use a different save file

use crate::combat::GodMode;
use crate::common::*;
use crate::level::{skip_to, Director};
use crate::player::Player;
use crate::save::{Profile, STAGE_COUNT};
use crate::state::{AppState, Run};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

#[derive(Resource, Default)]
struct Dev {
    stage: Option<usize>,
    skip: Option<f32>,
    god: bool,
    shots_dir: Option<String>,
    shot_at: Vec<f32>,
    quit: Option<f32>,
    skipped: bool,
    shot_index: usize,
}

fn env<T: std::str::FromStr>(key: &str) -> Option<T> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        let dev = Dev {
            stage: env::<usize>("STORMWING_STAGE").map(|s| s.clamp(1, STAGE_COUNT) - 1),
            skip: env("STORMWING_SKIP"),
            god: env::<u32>("STORMWING_GOD").is_some_and(|g| g != 0),
            shots_dir: std::env::var("STORMWING_SHOTS").ok(),
            shot_at: std::env::var("STORMWING_SHOT_AT")
                .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
                .unwrap_or_default(),
            quit: env("STORMWING_QUIT"),
            ..default()
        };
        let active = dev.stage.is_some()
            || dev.god
            || dev.shots_dir.is_some()
            || dev.quit.is_some()
            || std::env::var("STORMWING_SCREEN").is_ok()
            || std::env::var("STORMWING_STARS").is_ok()
            || std::env::var("STORMWING_MUTE").is_ok();
        if env::<u32>("STORMWING_FPS").is_some_and(|f| f != 0) {
            app.add_plugins((
                bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
                bevy::diagnostic::LogDiagnosticsPlugin::default(),
            ));
        }
        if !active {
            return;
        }
        app.insert_resource(GodMode(dev.god))
            .insert_resource(dev)
            .add_systems(Startup, dev_startup)
            .add_systems(Update, (dev_skip, dev_shots))
            .add_systems(Update, autopilot.in_set(GameSet::Move));
    }
}

fn dev_startup(
    dev: Res<Dev>,
    mut profile: ResMut<Profile>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<AppState>>,
) {
    if env::<u32>("STORMWING_MUTE").is_some_and(|m| m != 0) {
        profile.muted = true;
    }
    if let Some(stars) = env::<u32>("STORMWING_STARS") {
        profile.stars = stars;
        profile.unlocked = STAGE_COUNT;
    }
    if let Some(stage) = dev.stage {
        *run = Run::new(stage);
        next.set(AppState::Playing);
    } else if let Ok(screen) = std::env::var("STORMWING_SCREEN") {
        match screen.as_str() {
            "hangar" => next.set(AppState::Hangar),
            "select" => next.set(AppState::StageSelect),
            "results" => {
                // A sample victory, for previewing the results screen.
                *run = Run {
                    stage: 0,
                    score: 48_250,
                    stars: 212,
                    kills: 61,
                    spawned: 70,
                    survivors: 5,
                    rescued: 5,
                    outcome: Some(crate::state::Outcome::Victory),
                    medals: [true, false, false, true],
                    new_medals: [true, false, false, true],
                    medal_bonus: 300,
                    unlocked_next: true,
                    ..default()
                };
                next.set(AppState::Results);
            }
            _ => {}
        }
    }
}

fn dev_skip(
    mut dev: ResMut<Dev>,
    state: Res<State<AppState>>,
    director: Option<ResMut<Director>>,
) {
    if dev.skipped || *state.get() != AppState::Playing {
        return;
    }
    let (Some(skip), Some(mut director)) = (dev.skip, director) else {
        return;
    };
    if director.t > 0.0 {
        skip_to(&mut director, skip);
        dev.skipped = true;
    }
}

fn dev_shots(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut dev: ResMut<Dev>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if let Some(dir) = dev.shots_dir.clone() {
        if dev.shot_index < dev.shot_at.len() && now >= dev.shot_at[dev.shot_index] {
            let _ = std::fs::create_dir_all(&dir);
            let path = format!("{dir}/shot-{:02}.png", dev.shot_index);
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            dev.shot_index += 1;
        }
    }
    if dev.quit.is_some_and(|q| now >= q) {
        exit.write(AppExit::Success);
    }
}

/// Weaves left and right near the bottom of the screen so screenshots show
/// the game in action.
fn autopilot(dev: Res<Dev>, time: Res<Time>, mut q: Query<(&Player, &mut Transform)>) {
    if !dev.god {
        return;
    }
    let Ok((player, mut t)) = q.single_mut() else { return };
    if !player.controllable() {
        return;
    }
    let s = time.elapsed_secs();
    t.translation.x = (s * 0.7).sin() * (HALF_W - 80.0);
    t.translation.y = -HALF_H + 170.0 + (s * 0.45).sin() * 70.0;
}
