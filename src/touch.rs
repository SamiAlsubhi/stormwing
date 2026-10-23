//! Touch-screen controls (phones and tablets, including the web build).
//!
//! - Drag with one finger anywhere to fly; the plane moves with your finger.
//! - Lifting every finger pauses the game; touch the lower half of the screen
//!   to carry on.
//! - Tap with a second finger to drop a bomb.

use crate::player::Player;
use crate::state::{AppState, PlayState, Run};
use bevy::prelude::*;

/// Finger movement is multiplied by this, so you don't have to drag as far.
pub const TOUCH_SENSITIVITY: f32 = 1.25;

#[derive(Resource, Default)]
pub struct TouchInput {
    /// The player is using touch right now. Turned on by touching the screen
    /// and off again by using the keyboard, so touch-screen laptops still
    /// play normally with keys.
    pub enabled: bool,
    /// The device's main input is a finger (phones, tablets): show touch hints.
    pub touch_device: bool,
    /// The finger currently steering the plane.
    pub drag: Option<u64>,
}

impl TouchInput {
    pub fn show_touch_hints(&self) -> bool {
        self.enabled || self.touch_device
    }
}

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TouchInput {
            enabled: false,
            touch_device: is_touch_device(),
            drag: None,
        })
        .add_systems(Update, detect_touch)
        .add_systems(Update, touch_autopause.run_if(in_state(AppState::Playing)));
    }
}

/// A "coarse" primary pointer means a finger is the main input.
#[cfg(target_arch = "wasm32")]
fn is_touch_device() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(pointer: coarse)").ok().flatten())
        .is_some_and(|m| m.matches())
}

#[cfg(not(target_arch = "wasm32"))]
fn is_touch_device() -> bool {
    false
}

fn detect_touch(touches: Res<Touches>, keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<TouchInput>) {
    if !input.enabled && touches.any_just_pressed() {
        input.enabled = true;
    } else if input.enabled && keys.get_just_pressed().next().is_some() {
        input.enabled = false;
    }
}

/// Pause as soon as no finger is on the screen; resume on a new touch in the
/// lower part of the screen (the top half is left free for the pause menu).
fn touch_autopause(
    touches: Res<Touches>,
    input: Res<TouchInput>,
    run: Res<Run>,
    state: Option<Res<State<PlayState>>>,
    mut next: ResMut<NextState<PlayState>>,
    player: Query<&Player>,
    windows: Query<&Window>,
) {
    let Some(state) = state else { return };
    if !input.enabled || run.outcome.is_some() {
        return;
    }
    match state.get() {
        PlayState::Running => {
            let flying = player.single().is_ok_and(|p| p.controllable());
            if flying && touches.iter().next().is_none() {
                next.set(PlayState::Paused);
            }
        }
        PlayState::Paused => {
            let height = windows.single().map(|w| w.height()).unwrap_or(1.0);
            if touches.iter_just_pressed().any(|t| t.position().y > height * 0.45) {
                next.set(PlayState::Running);
            }
        }
    }
}

/// World-space movement of the steering finger this frame, if any.
pub fn drag_delta(
    touches: &Touches,
    input: &mut TouchInput,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec2> {
    if let Some(id) = input.drag {
        if touches.get_pressed(id).is_none() {
            input.drag = None;
        }
    }
    if input.drag.is_none() {
        input.drag = touches.iter().next().map(|t| t.id());
    }
    let touch = touches.get_pressed(input.drag?)?;
    let now = camera.viewport_to_world_2d(camera_transform, touch.position()).ok()?;
    let before = camera
        .viewport_to_world_2d(camera_transform, touch.previous_position())
        .ok()?;
    Some((now - before) * TOUCH_SENSITIVITY)
}

/// A new finger (other than the steering one) tapped the screen.
pub fn second_finger_tap(touches: &Touches, input: &TouchInput) -> bool {
    input.drag.is_some() && touches.iter_just_pressed().any(|t| Some(t.id()) != input.drag)
}
