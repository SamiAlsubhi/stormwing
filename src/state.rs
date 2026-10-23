//! App states and the per-attempt run statistics.

use crate::save::{Profile, STAGE_COUNT};
use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub enum AppState {
    #[default]
    Menu,
    StageSelect,
    Hangar,
    Playing,
    Results,
}

#[derive(SubStates, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
#[source(AppState = AppState::Playing)]
pub enum PlayState {
    #[default]
    Running,
    Paused,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Victory,
    Defeat,
}

pub const MEDAL_NAMES: [&str; 4] = [
    "Destroy 70% of enemies",
    "Destroy every enemy",
    "Take no damage",
    "Rescue every survivor",
];

/// Statistics for the current attempt at a stage.
#[derive(Resource, Default)]
pub struct Run {
    pub stage: usize,
    pub score: u32,
    pub stars: u32,
    pub kills: u32,
    pub spawned: u32,
    pub survivors: u32,
    pub rescued: u32,
    pub damaged: bool,
    pub outcome: Option<Outcome>,
    /// Counts down after victory/defeat so the final explosions can play out.
    pub end_timer: Option<f32>,
    pub medals: [bool; 4],
    pub new_medals: [bool; 4],
    pub medal_bonus: u32,
    pub new_best: bool,
    pub unlocked_next: bool,
}

impl Run {
    pub fn new(stage: usize) -> Self {
        Self {
            stage,
            ..default()
        }
    }

    pub fn kill_ratio(&self) -> f32 {
        if self.spawned == 0 {
            1.0
        } else {
            self.kills as f32 / self.spawned as f32
        }
    }

    pub fn end(&mut self, outcome: Outcome, delay: f32) {
        if self.outcome.is_none() {
            self.outcome = Some(outcome);
            self.end_timer = Some(delay);
        }
    }
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<PlayState>()
            .init_resource::<Run>()
            .add_systems(OnEnter(PlayState::Paused), pause_time)
            .add_systems(OnExit(PlayState::Paused), resume_time)
            .add_systems(OnExit(AppState::Playing), resume_time)
            .add_systems(
                Update,
                (
                    toggle_pause.run_if(in_state(AppState::Playing)),
                    finish_run.run_if(in_state(PlayState::Running)),
                ),
            );
    }
}

fn pause_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn resume_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    state: Res<State<PlayState>>,
    run: Res<Run>,
    mut next: ResMut<NextState<PlayState>>,
) {
    let pad_start = pads.iter().any(|p| p.just_pressed(GamepadButton::Start));
    if run.outcome.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyP) || pad_start {
        next.set(match state.get() {
            PlayState::Running => PlayState::Paused,
            PlayState::Paused => PlayState::Running,
        });
    }
}

/// Once the end-of-stage delay runs out, bank the rewards and show results.
fn finish_run(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut profile: ResMut<Profile>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(timer) = run.end_timer.as_mut() else {
        return;
    };
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    run.end_timer = None;
    let stage = run.stage.min(STAGE_COUNT - 1);

    if run.outcome == Some(Outcome::Victory) {
        let ratio = run.kill_ratio();
        run.medals = [
            ratio >= 0.7,
            run.kills >= run.spawned,
            !run.damaged,
            run.survivors > 0 && run.rescued >= run.survivors,
        ];
        let mut bonus = 0;
        for i in 0..4 {
            if run.medals[i] && !profile.medals[stage][i] {
                profile.medals[stage][i] = true;
                run.new_medals[i] = true;
                bonus += 150 * (stage as u32 + 1);
            }
        }
        run.medal_bonus = bonus;
        if profile.unlocked < (stage + 2).min(STAGE_COUNT) {
            profile.unlocked = (stage + 2).min(STAGE_COUNT);
            run.unlocked_next = stage + 1 < STAGE_COUNT;
        }
        if !profile.cleared[stage] {
            profile.cleared[stage] = true;
        }
    }

    if run.score > profile.best[stage] {
        profile.best[stage] = run.score;
        run.new_best = true;
    }
    // Stars are kept even when the mission fails, so every attempt makes progress.
    profile.stars += run.stars + run.medal_bonus;
    profile.save();
    next.set(AppState::Results);
}
