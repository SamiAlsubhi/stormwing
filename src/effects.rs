//! Particles, explosions, floating score text, screen shake and flashes.
//!
//! Particles are camera-facing quads drawn with self-lit materials: fire,
//! sparks and shockwaves blend additively (and bloom), smoke and debris blend
//! normally. Colours fade through a cache of materials quantised along each
//! fade, so no per-frame material churn is needed.

use crate::art::{fx_material, Art};
use crate::common::*;
use crate::rng::Rng;
use crate::state::AppState;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Which texture a particle is drawn with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FxTex {
    Glow,
    Smoke,
    Ring,
    Streak,
    Solid,
}

type FxKey = (FxTex, bool, [i16; 4]);

#[derive(Resource, Default)]
struct FxMaterials {
    cache: HashMap<FxKey, Handle<StandardMaterial>>,
}

fn fx_key(tex: FxTex, additive: bool, c: LinearRgba) -> FxKey {
    let q = |v: f32| (v * 24.0).round() as i16;
    (tex, additive, [q(c.red), q(c.green), q(c.blue), q(c.alpha.clamp(0.0, 1.0))])
}

#[derive(Component)]
pub struct Particle {
    pub tex: FxTex,
    pub additive: bool,
    pub vel: Vec2,
    pub drag: f32,
    pub life: f32,
    pub max_life: f32,
    pub size: (Vec2, Vec2),
    pub color: (LinearRgba, LinearRgba),
    pub spin: f32,
    /// Rotate to point along the velocity.
    pub align: bool,
    /// Drift with the ground scroll (smoke, scorch marks).
    pub grounded: bool,
    key: Option<FxKey>,
}

impl Particle {
    fn new(vel: Vec2, life: f32, size: Vec2, color: LinearRgba) -> Self {
        Self {
            tex: FxTex::Glow,
            additive: true,
            vel,
            drag: 2.0,
            life,
            max_life: life,
            size: (size, size),
            color: (color, color.with_alpha(0.0)),
            spin: 0.0,
            align: false,
            grounded: false,
            key: None,
        }
    }

    fn tex(mut self, tex: FxTex) -> Self {
        self.tex = tex;
        self
    }

    fn blended(mut self) -> Self {
        self.additive = false;
        self
    }
}

#[derive(Component)]
pub struct FloatingText {
    world: Vec3,
    life: f32,
    max_life: f32,
}

#[derive(Component)]
pub struct ScreenFlash {
    life: f32,
    max_life: f32,
    alpha: f32,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxMaterials>()
            .add_systems(Update, (update_particles, update_floating_text, update_flash, apply_shake))
            .add_systems(PostUpdate, apply_particle_materials);
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    scroll: Res<ScrollSpeed>,
    mut q: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut t) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        let k = 1.0 - p.life / p.max_life;
        let drag = (-p.drag * dt).exp();
        p.vel *= drag;
        t.translation += (p.vel * dt).extend(0.0);
        if p.grounded {
            t.translation.y -= scroll.0 * dt;
        }
        if p.align && p.vel.length_squared() > 1.0 {
            t.rotation = Quat::from_rotation_z(p.vel.to_angle() - std::f32::consts::FRAC_PI_2);
        } else if p.spin != 0.0 {
            t.rotate_z(p.spin * dt);
        }
        let size = p.size.0.lerp(p.size.1, k);
        t.scale = size.max(Vec2::splat(0.01)).extend(1.0);
    }
}

/// Picks (or creates) the cached material matching each particle's colour.
fn apply_particle_materials(
    mut commands: Commands,
    art: Res<Art>,
    mut cache: ResMut<FxMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &mut Particle, Option<&mut MeshMaterial3d<StandardMaterial>>)>,
) {
    for (e, mut p, mat) in &mut q {
        let k = (1.0 - p.life / p.max_life).clamp(0.0, 1.0);
        let (a, b) = p.color;
        let key = fx_key(p.tex, p.additive, a.mix(&b, k));
        if p.key == Some(key) && mat.is_some() {
            continue;
        }
        p.key = Some(key);
        let handle = cache
            .cache
            .entry(key)
            .or_insert_with(|| {
                let (tex, additive, [r, g, bl, al]) = key;
                let texture = match tex {
                    FxTex::Glow => Some(art.tex_glow.clone()),
                    FxTex::Smoke => Some(art.tex_smoke.clone()),
                    FxTex::Ring => Some(art.tex_ring.clone()),
                    FxTex::Streak => Some(art.tex_streak.clone()),
                    FxTex::Solid => None,
                };
                let color = LinearRgba::new(r as f32 / 24.0, g as f32 / 24.0, bl as f32 / 24.0, al as f32 / 24.0);
                materials.add(fx_material(texture, Color::LinearRgba(color), additive))
            })
            .clone();
        match mat {
            Some(mut m) => m.0 = handle,
            None => {
                commands.entity(e).insert(MeshMaterial3d(handle));
            }
        }
    }
}

fn update_floating_text(
    mut commands: Commands,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q: Query<(Entity, &mut FloatingText, &mut Node, &mut TextColor, &mut UiTransform)>,
) {
    let dt = time.delta_secs();
    let Ok((cam, cam_t)) = camera.single() else { return };
    for (e, mut f, mut node, mut color, mut ui_t) in &mut q {
        f.life -= dt;
        if f.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        f.world.y += 40.0 * dt;
        if let Ok(vp) = cam.world_to_viewport(cam_t, f.world) {
            node.left = px(vp.x / ui_scale.0 - 150.0);
            node.top = px(vp.y / ui_scale.0 - 16.0);
        }
        let k = f.life / f.max_life;
        color.0.set_alpha((k * 2.0).min(1.0));
        let pop = 1.0 + 0.4 * ((1.0 - k) * 12.0).min(1.0) * (1.0 - (1.0 - k) * 2.0).max(0.0);
        ui_t.scale = Vec2::splat(pop);
    }
}

fn update_flash(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut q: Query<(Entity, &mut ScreenFlash, &mut BackgroundColor)>,
) {
    for (e, mut f, mut bg) in &mut q {
        f.life -= time.delta_secs();
        if f.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        bg.0.set_alpha(f.alpha * (f.life / f.max_life).powi(2));
    }
}

fn apply_shake(
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    mut rng: ResMut<Rng>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut t) = cam.single_mut() else { return };
    shake.trauma = (shake.trauma - time.delta_secs() * 1.4).max(0.0);
    let amount = shake.trauma * shake.trauma * 16.0;
    t.translation.x = rng.range(-1.0, 1.0) * amount;
    t.translation.y = rng.range(-1.0, 1.0) * amount;
}

// ---------------------------------------------------------------------------
// Spawning helpers
// ---------------------------------------------------------------------------

fn particle_bundle(art: &Art, pos: Vec3, p: Particle) -> impl Bundle {
    (
        Mesh3d(art.quad.clone()),
        Transform::from_translation(pos).with_scale(p.size.0.extend(1.0)),
        p,
        NotShadowCaster,
        NotShadowReceiver,
        DespawnOnExit(AppState::Playing),
    )
}

/// A fiery explosion. `size` is roughly the radius of the destroyed object.
pub fn explosion(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec3, size: f32, ground: bool) {
    let s = (size / 24.0).clamp(0.5, 6.0);
    let base = pos.truncate();
    let z = pos.z + 6.0;

    // A burst of real light over the sea, islands and nearby aircraft.
    // Over the ground it's a bright pool of firelight; in the air it mostly
    // lights up the aircraft nearby.
    let (lift, height, lux) = if ground { (55.0, 55.0, 25000.0) } else { (80.0, pos.z + 80.0, 6000.0) };
    crate::lights::flash(
        commands,
        pos + Vec3::Z * lift,
        Color::srgb(1.0, 0.6, 0.28),
        crate::lights::lumens_for(lux * s.sqrt(), height),
        height + 150.0 + 100.0 * s,
        0.3 + 0.07 * s,
    );

    // Bright central flash.
    let mut flash = Particle::new(Vec2::ZERO, 0.18, Vec2::splat(70.0 * s), LinearRgba::rgb(9.0, 6.0, 3.0));
    flash.size.1 = Vec2::splat(150.0 * s);
    commands.spawn(particle_bundle(art, base.extend(z + 3.0), flash));

    // Fireballs.
    for _ in 0..(6.0 + 5.0 * s) as usize {
        let vel = rng.dir() * rng.range(30.0, 170.0) * s.sqrt();
        let r = rng.range(22.0, 40.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.35, 0.8), Vec2::splat(r), LinearRgba::rgb(5.0, 2.6, 0.6));
        p.size.1 = Vec2::splat(r * 1.8);
        p.color.1 = LinearRgba::new(0.6, 0.08, 0.02, 0.0);
        p.drag = 3.0;
        p.grounded = ground;
        commands.spawn(particle_bundle(art, (base + rng.in_disc(8.0 * s)).extend(z + 2.0), p));
    }

    // Sparks.
    for _ in 0..(5.0 + 4.0 * s) as usize {
        let vel = rng.dir() * rng.range(250.0, 600.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.2, 0.45), Vec2::new(5.0, 22.0), LinearRgba::rgb(6.0, 4.0, 1.2));
        p.size.1 = Vec2::new(2.0, 8.0);
        p.drag = 4.0;
        p.align = true;
        commands.spawn(particle_bundle(art, base.extend(z + 4.0), p.tex(FxTex::Streak)));
    }

    // Billowing smoke.
    for _ in 0..(3.0 + 2.0 * s) as usize {
        let vel = rng.dir() * rng.range(10.0, 60.0);
        let r = rng.range(30.0, 50.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.9, 1.7), Vec2::splat(r), LinearRgba::new(0.16, 0.14, 0.13, 0.7));
        p.size.1 = Vec2::splat(r * 2.3);
        p.drag = 1.0;
        p.grounded = true;
        p.spin = rng.range(-0.8, 0.8);
        commands.spawn(particle_bundle(art, (base + rng.in_disc(10.0 * s)).extend(z), p.tex(FxTex::Smoke).blended()));
    }

    // Debris chunks.
    for _ in 0..(3.0 + 2.0 * s) as usize {
        let vel = rng.dir() * rng.range(100.0, 300.0) * s.sqrt();
        let d = rng.range(4.0, 8.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.5, 0.9), Vec2::splat(d), LinearRgba::new(0.08, 0.07, 0.07, 1.0));
        p.spin = rng.range(-12.0, 12.0);
        p.drag = 2.5;
        p.size.1 = Vec2::splat(d * 0.6);
        commands.spawn(particle_bundle(art, base.extend(z + 1.0), p.tex(FxTex::Solid).blended()));
    }

    // Shockwave ring for larger blasts.
    if s >= 1.2 {
        let mut p = Particle::new(Vec2::ZERO, 0.4, Vec2::splat(30.0 * s), LinearRgba::new(2.0, 1.6, 1.2, 0.8));
        p.size.1 = Vec2::splat(260.0 * s.sqrt());
        commands.spawn(particle_bundle(art, base.extend(z + 5.0), p.tex(FxTex::Ring)));
    }

    // Burnt patch left on the ground.
    if ground {
        commands.spawn((
            Mesh3d(art.quad.clone()),
            MeshMaterial3d(art.mat_scorch.clone()),
            Transform::from_translation(base.extend(pos.z + 1.5)).with_scale(Vec3::splat(70.0 * s)),
            Scrolls,
            Offscreen::new(150.0),
            NotShadowCaster,
            NotShadowReceiver,
            DespawnOnExit(AppState::Playing),
        ));
    }
}

/// Small spray of sparks where a bullet hits.
pub fn hit_sparks(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec3, color: LinearRgba) {
    for _ in 0..3 {
        let vel = (Vec2::Y + rng.dir() * 0.9).normalize_or_zero() * rng.range(120.0, 320.0);
        let mut p = Particle::new(vel, rng.range(0.1, 0.22), Vec2::new(4.0, 14.0), color);
        p.align = true;
        p.drag = 5.0;
        commands.spawn(particle_bundle(art, pos, p.tex(FxTex::Streak)));
    }
    let mut glow = Particle::new(Vec2::ZERO, 0.08, Vec2::splat(26.0), color);
    glow.size.1 = Vec2::splat(10.0);
    commands.spawn(particle_bundle(art, pos, glow));
}

/// Generic glowing puff, e.g. muzzle flashes, pickup sparkles and trails.
pub fn puff(commands: &mut Commands, art: &Art, pos: Vec3, vel: Vec2, life: f32, size: (f32, f32), color: LinearRgba) {
    let mut p = Particle::new(vel, life, Vec2::splat(size.0), color);
    p.size.1 = Vec2::splat(size.1);
    p.drag = 1.5;
    commands.spawn(particle_bundle(art, pos, p));
}

/// Soft grey smoke puff (missile trails, damaged bosses).
pub fn smoke_puff(commands: &mut Commands, art: &Art, pos: Vec3, vel: Vec2, life: f32, size: (f32, f32), alpha: f32) {
    let mut p = Particle::new(vel, life, Vec2::splat(size.0), LinearRgba::new(0.8, 0.8, 0.82, alpha));
    p.size.1 = Vec2::splat(size.1);
    p.drag = 1.5;
    commands.spawn(particle_bundle(art, pos, p.tex(FxTex::Smoke).blended()));
}

/// White water: wakes, splashes and whale spouts. Drifts with the sea.
pub fn foam(commands: &mut Commands, art: &Art, pos: Vec3, vel: Vec2, life: f32, size: (f32, f32), alpha: f32) {
    let mut p = Particle::new(vel, life, Vec2::splat(size.0), LinearRgba::new(1.0, 1.0, 1.0, alpha));
    p.size.1 = Vec2::splat(size.1);
    p.drag = 1.2;
    p.grounded = true;
    p.spin = 0.4;
    commands.spawn(particle_bundle(art, pos, p.tex(FxTex::Smoke).blended()));
}

/// Expanding ripple on the sea surface that drifts with the water.
pub fn ripple(commands: &mut Commands, art: &Art, pos: Vec3, from: f32, to: f32, life: f32, alpha: f32) {
    let mut p = Particle::new(Vec2::ZERO, life, Vec2::splat(from), LinearRgba::new(1.0, 1.0, 1.0, alpha));
    p.size.1 = Vec2::splat(to);
    p.grounded = true;
    commands.spawn(particle_bundle(art, pos, p.tex(FxTex::Ring).blended()));
}

/// Expanding ring, e.g. for bombs and power-up pickups.
pub fn shockwave(commands: &mut Commands, art: &Art, pos: Vec3, from: f32, to: f32, life: f32, color: LinearRgba) {
    let mut p = Particle::new(Vec2::ZERO, life, Vec2::splat(from), color);
    p.size.1 = Vec2::splat(to);
    commands.spawn(particle_bundle(art, pos, p.tex(FxTex::Ring)));
}

/// Score or label that pops up in the world and floats away.
pub fn floating_text(commands: &mut Commands, pos: Vec3, text: impl Into<String>, color: Color, size: f32) {
    commands.spawn((
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::new(0.0, 2.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.5),
        },
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: px(300),
            left: px(-1000),
            ..default()
        },
        UiTransform::default(),
        Pickable::IGNORE,
        FloatingText {
            world: pos,
            life: 0.9,
            max_life: 0.9,
        },
        DespawnOnExit(AppState::Playing),
    ));
}

pub fn screen_flash(commands: &mut Commands, color: Color, alpha: f32, life: f32) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(color.with_alpha(alpha)),
        GlobalZIndex(5),
        Pickable::IGNORE,
        ScreenFlash {
            life,
            max_life: life,
            alpha,
        },
    ));
}
