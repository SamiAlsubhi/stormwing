//! Dynamic lights: explosions, the bomb, gunfire and rescue beacons throw
//! real light onto the islands, dunes, ice, ships and aircraft around them.
//! (Deep water is dark, so it shows them only faintly, as it would.)
//!
//! The world is measured in screen pixels (the arena is 720 wide), so light
//! intensities, in lumens, are far larger than real-world values. A light
//! `d` units above the sea lights it with `intensity / (4π d²)` lux; the sun
//! gives 9000.

use crate::state::AppState;
use bevy::prelude::*;
use std::f32::consts::PI;

/// At most this many flashes at once; the faintest go first.
const MAX_FLASHES: usize = 14;

/// The intensity that lights a surface `height` below with `lux`.
pub fn lumens_for(lux: f32, height: f32) -> f32 {
    lux * 4.0 * PI * height * height
}

/// A burst of light that fades out.
#[derive(Component)]
pub struct Flash {
    age: f32,
    life: f32,
    peak: f32,
}

/// A light that blinks on and off (survivor beacons).
#[derive(Component)]
pub struct Blink {
    pub base: f32,
    pub rate: f32,
}

/// The gun flash on the player's nose, kicked to full by each volley.
#[derive(Component)]
pub struct MuzzleLight {
    pub kick: f32,
    pub peak: f32,
}

pub struct LightsPlugin;

impl Plugin for LightsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (fade_flashes, limit_flashes, blink, fade_muzzle));
    }
}

/// A point light with no shadows (point-light shadows are expensive).
pub fn point(color: Color, intensity: f32, range: f32) -> PointLight {
    PointLight {
        color,
        intensity,
        range,
        radius: 6.0,
        shadow_maps_enabled: false,
        ..default()
    }
}

/// Spawns a flash at `pos` that fades out over `life` seconds.
pub fn flash(commands: &mut Commands, pos: Vec3, color: Color, peak: f32, range: f32, life: f32) {
    commands.spawn((
        point(color, peak, range),
        Transform::from_translation(pos),
        Flash { age: 0.0, life, peak },
        DespawnOnExit(AppState::Playing),
    ));
}

fn fade_flashes(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Flash, &mut PointLight)>) {
    let dt = time.delta_secs();
    for (e, mut f, mut light) in &mut q {
        f.age += dt;
        let t = f.age / f.life;
        if t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        light.intensity = f.peak * (1.0 - t) * (1.0 - t);
    }
}

fn limit_flashes(mut commands: Commands, q: Query<(Entity, &PointLight), With<Flash>>) {
    let count = q.iter().len();
    if count <= MAX_FLASHES {
        return;
    }
    let mut all: Vec<(Entity, f32)> = q.iter().map(|(e, l)| (e, l.intensity)).collect();
    all.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (e, _) in all.into_iter().take(count - MAX_FLASHES) {
        commands.entity(e).despawn();
    }
}

fn blink(time: Res<Time>, mut q: Query<(&Blink, &mut PointLight)>) {
    let now = time.elapsed_secs();
    for (b, mut light) in &mut q {
        let on = (now * b.rate).fract() < 0.22;
        light.intensity = if on { b.base } else { 0.0 };
    }
}

fn fade_muzzle(time: Res<Time>, mut q: Query<(&mut MuzzleLight, &mut PointLight)>) {
    let decay = (-time.delta_secs() * 30.0).exp();
    for (mut m, mut light) in &mut q {
        m.kick *= decay;
        light.intensity = m.peak * m.kick;
    }
}
