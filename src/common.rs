//! Components, resources and system sets shared across the game.

use crate::state::PlayState;
use bevy::prelude::*;

/// Logical play-field size. The camera always shows at least this area.
pub const ARENA: Vec2 = Vec2::new(720.0, 1080.0);
pub const HALF_W: f32 = ARENA.x / 2.0;
pub const HALF_H: f32 = ARENA.y / 2.0;

/// Draw order (z) for each layer of the scene.
pub mod z {
    pub const WATER_FX: f32 = 1.0;
    pub const ISLAND: f32 = 2.0;
    pub const SCORCH: f32 = 3.0;
    pub const GROUND_UNIT: f32 = 4.0;
    pub const SURVIVOR: f32 = 5.0;
    pub const SHADOW: f32 = 6.0;
    pub const PICKUP: f32 = 14.0;
    pub const ENEMY: f32 = 16.0;
    pub const BOSS: f32 = 15.0;
    pub const PLAYER: f32 = 20.0;
    pub const PLAYER_SHOT: f32 = 22.0;
    pub const ENEMY_SHOT: f32 = 26.0;
    pub const FX: f32 = 30.0;
    pub const CLOUD: f32 = 40.0;
    pub const TEXT: f32 = 45.0;
    pub const FLASH: f32 = 60.0;
    pub const MASK: f32 = 90.0;
}

#[derive(Component, Deref, DerefMut, Default, Clone, Copy)]
pub struct Velocity(pub Vec2);

/// Entity scrolls down with the ground layer.
#[derive(Component)]
pub struct Scrolls;

/// Removed once it leaves the play field by more than the given margin.
#[derive(Component)]
pub struct Offscreen {
    pub margin: f32,
}

impl Offscreen {
    pub fn new(margin: f32) -> Self {
        Self { margin }
    }
}

#[derive(Component)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
}

impl Health {
    pub fn new(hp: f32) -> Self {
        Self { hp, max: hp }
    }
    pub fn frac(&self) -> f32 {
        (self.hp / self.max).clamp(0.0, 1.0)
    }
}

/// Circular collision radius.
#[derive(Component, Deref, Clone, Copy)]
pub struct Hitbox(pub f32);

/// Short white flash when an entity takes a hit.
#[derive(Component, Default)]
pub struct HitFlash {
    pub timer: f32,
}

/// A drop shadow cast onto the ground, drawn as a child of a flying object.
#[derive(Component)]
pub struct Shadow {
    /// World-space offset from the owner.
    pub offset: Vec2,
}

/// Current ground scroll speed in units per second.
#[derive(Resource)]
pub struct ScrollSpeed(pub f32);

impl Default for ScrollSpeed {
    fn default() -> Self {
        Self(70.0)
    }
}

/// Screen shake, accumulated as "trauma" in `[0, 1]`.
#[derive(Resource, Default)]
pub struct Shake {
    pub trauma: f32,
}

impl Shake {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Gameplay systems run in this order, and only while a stage is running.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    Input,
    Spawn,
    Move,
    Combat,
    Resolve,
    Cleanup,
}

pub struct CommonPlugin;

impl Plugin for CommonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScrollSpeed>()
            .init_resource::<Shake>()
            .insert_resource(crate::rng::Rng::from_time())
            .configure_sets(
                Update,
                (
                    GameSet::Input,
                    GameSet::Spawn,
                    GameSet::Move,
                    GameSet::Combat,
                    GameSet::Resolve,
                    GameSet::Cleanup,
                )
                    .chain()
                    .run_if(in_state(PlayState::Running)),
            )
            .add_systems(
                Update,
                (move_velocity, scroll_with_ground)
                    .in_set(GameSet::Move),
            )
            .add_systems(Update, (despawn_offscreen,).in_set(GameSet::Cleanup))
            .add_systems(Update, (update_hit_flash, update_shadows));
    }
}

fn move_velocity(time: Res<Time>, mut q: Query<(&mut Transform, &Velocity)>) {
    let dt = time.delta_secs();
    for (mut t, v) in &mut q {
        t.translation += (v.0 * dt).extend(0.0);
    }
}

fn scroll_with_ground(
    time: Res<Time>,
    speed: Res<ScrollSpeed>,
    mut q: Query<&mut Transform, With<Scrolls>>,
) {
    let dy = speed.0 * time.delta_secs();
    for mut t in &mut q {
        t.translation.y -= dy;
    }
}

fn despawn_offscreen(mut commands: Commands, q: Query<(Entity, &Transform, &Offscreen)>) {
    for (e, t, o) in &q {
        let p = t.translation;
        // Things spawn above the screen, so allow a generous margin at the top.
        if p.x.abs() > HALF_W + o.margin
            || p.y < -HALF_H - o.margin
            || p.y > HALF_H + o.margin + 700.0
        {
            commands.entity(e).try_despawn();
        }
    }
}

fn update_hit_flash(
    time: Res<Time>,
    art: Res<crate::art::Art>,
    mut q: Query<(&mut HitFlash, &mut MeshMaterial2d<ColorMaterial>)>,
) {
    for (mut flash, mut mat) in &mut q {
        if flash.timer > 0.0 {
            flash.timer -= time.delta_secs();
            let want = if flash.timer > 0.0 {
                &art.mat_flash
            } else {
                &art.mat_base
            };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        }
    }
}

/// Keeps each shadow at a fixed world-space offset from its owner, even when
/// the owner rotates or banks.
fn update_shadows(
    parents: Query<&Transform, Without<Shadow>>,
    mut shadows: Query<(&Shadow, &ChildOf, &mut Transform)>,
) {
    for (shadow, child_of, mut t) in &mut shadows {
        let Ok(parent) = parents.get(child_of.parent()) else {
            continue;
        };
        let inv = parent.rotation.inverse();
        let local = inv * shadow.offset.extend(0.0);
        let scale = parent.scale.truncate().max(Vec2::splat(0.01));
        t.translation = Vec3::new(local.x / scale.x, local.y / scale.y, z::SHADOW - parent.translation.z);
    }
}

pub fn flash(f: &mut HitFlash) {
    f.timer = 0.06;
}
