//! Components, resources and system sets shared across the game.

use crate::state::PlayState;
use bevy::prelude::*;

/// Logical play-field size. The camera always shows at least this area.
pub const ARENA: Vec2 = Vec2::new(720.0, 1080.0);
pub const HALF_W: f32 = ARENA.x / 2.0;
pub const HALF_H: f32 = ARENA.y / 2.0;

/// Altitude (z) of each layer. The camera looks straight down from above, so
/// higher things are drawn over lower ones and cast shadows onto them.
pub mod z {
    pub const SEA_FLOOR: f32 = -22.4;
    pub const WATER: f32 = 0.0;
    pub const CLOUD_SHADOW: f32 = 50.0;
    pub const CLOUD_LOW: f32 = 78.0;
    pub const PICKUP: f32 = 100.0;
    pub const BOSS: f32 = 112.0;
    pub const ENEMY: f32 = 132.0;
    pub const PLAYER: f32 = 150.0;
    pub const FX: f32 = 170.0;
    pub const CLOUD_HIGH: f32 = 215.0;
    pub const PLAYER_SHOT: f32 = 240.0;
    pub const ENEMY_SHOT: f32 = 250.0;
    pub const MASK: f32 = 900.0;
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

/// Current ground scroll speed in units per second.
#[derive(Resource)]
pub struct ScrollSpeed(pub f32);

impl Default for ScrollSpeed {
    fn default() -> Self {
        Self(70.0)
    }
}

/// Total distance the ground has scrolled since the scenery was reset, so
/// endless terrain (the dune field) can be sampled seamlessly.
#[derive(Resource, Default)]
pub struct GroundScroll {
    pub distance: f32,
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

/// Gameplay systems run in this order. Input, spawning, combat and
/// resolution only run while a stage is being played; movement and cleanup
/// also run in the menus so the scenery keeps scrolling behind them.
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
        let running = in_state(PlayState::Running);
        app.init_resource::<ScrollSpeed>()
            .init_resource::<GroundScroll>()
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
                    .chain(),
            )
            .configure_sets(Update, GameSet::Input.run_if(running.clone()))
            .configure_sets(Update, GameSet::Spawn.run_if(running.clone()))
            .configure_sets(Update, GameSet::Combat.run_if(running.clone()))
            .configure_sets(Update, GameSet::Resolve.run_if(running))
            .add_systems(Update, (move_velocity, scroll_with_ground).in_set(GameSet::Move))
            .add_systems(Update, despawn_offscreen.in_set(GameSet::Cleanup))
            .add_systems(Update, update_hit_flash);
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
    mut ground: ResMut<GroundScroll>,
    mut q: Query<&mut Transform, With<Scrolls>>,
) {
    let dy = speed.0 * time.delta_secs();
    ground.distance += dy;
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
    mut q: Query<(&mut HitFlash, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (mut flash, mut mat) in &mut q {
        if flash.timer > 0.0 {
            flash.timer -= time.delta_secs();
            let want = if flash.timer > 0.0 {
                &art.mat_flash
            } else {
                &art.mat_paint
            };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        }
    }
}

/// World position (on the ground plane) under a point on the screen.
pub fn screen_to_world(camera: &Camera, camera_transform: &GlobalTransform, screen: Vec2) -> Option<Vec2> {
    camera
        .viewport_to_world(camera_transform, screen)
        .ok()
        .map(|ray| ray.origin.truncate())
}

pub fn flash(f: &mut HitFlash) {
    f.timer = 0.06;
}

/// Spawns a model's glass and glow parts as children of `entity`.
pub fn attach_model_parts(
    commands: &mut Commands,
    entity: Entity,
    set: &crate::art::ModelSet,
    art: &crate::art::Art,
) {
    commands.entity(entity).with_children(|c| {
        if let Some(glass) = &set.glass {
            c.spawn((Mesh3d(glass.clone()), MeshMaterial3d(art.mat_glass.clone())));
        }
        if let Some(glow) = &set.glow {
            c.spawn((
                Mesh3d(glow.clone()),
                MeshMaterial3d(art.mat_glow.clone()),
                bevy::light::NotShadowCaster,
            ));
        }
    });
}
