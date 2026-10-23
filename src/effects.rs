//! Particles, explosions, floating score text, screen shake and flashes.

use crate::art::Art;
use crate::common::*;
use crate::rng::Rng;
use crate::state::AppState;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::sprite_render::AlphaMode2d;

/// Which texture a particle is drawn with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FxTex {
    Glow,
    Ring,
    Streak,
    Solid,
}

/// Particles are textured quads whose colour comes from a cache of
/// materials, quantised along each fade. Changing a `Sprite`'s colour every
/// frame would instead create a fresh material per particle per frame.
#[derive(Resource, Default)]
struct FxMaterials {
    cache: HashMap<(FxTex, [i16; 4]), Handle<ColorMaterial>>,
}

type FxKey = (FxTex, [i16; 4]);

fn fx_key(tex: FxTex, c: LinearRgba) -> FxKey {
    let q = |v: f32| (v * 24.0).round() as i16;
    (tex, [q(c.red), q(c.green), q(c.blue), q(c.alpha.clamp(0.0, 1.0))])
}

#[derive(Component)]
pub struct Particle {
    pub tex: FxTex,
    pub vel: Vec2,
    pub drag: f32,
    pub life: f32,
    pub max_life: f32,
    pub size: (Vec2, Vec2),
    pub color: (LinearRgba, LinearRgba),
    pub spin: f32,
    /// Rotate the sprite to point along its velocity.
    pub align: bool,
    /// Drift with the ground scroll (smoke, scorch marks).
    pub grounded: bool,
    /// Ignore the game clock (screen flashes keep fading while paused).
    pub real_time: bool,
    key: Option<FxKey>,
}

impl Particle {
    fn new(vel: Vec2, life: f32, size: Vec2, color: LinearRgba) -> Self {
        Self {
            tex: FxTex::Glow,
            vel,
            drag: 2.0,
            life,
            max_life: life,
            size: (size, size),
            color: (color, color.with_alpha(0.0)),
            spin: 0.0,
            align: false,
            grounded: false,
            real_time: false,
            key: None,
        }
    }

    fn tex(mut self, tex: FxTex) -> Self {
        self.tex = tex;
        self
    }
}

#[derive(Component)]
pub struct FloatingText {
    life: f32,
    max_life: f32,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxMaterials>()
            .add_systems(Update, (update_particles, update_floating_text, apply_shake))
            .add_systems(PostUpdate, apply_particle_materials);
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    real: Res<Time<Real>>,
    scroll: Res<ScrollSpeed>,
    mut q: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    for (e, mut p, mut t) in &mut q {
        let dt = if p.real_time { real.delta_secs() } else { time.delta_secs() };
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
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut q: Query<(Entity, &mut Particle, Option<&mut MeshMaterial2d<ColorMaterial>>)>,
) {
    for (e, mut p, mat) in &mut q {
        let k = (1.0 - p.life / p.max_life).clamp(0.0, 1.0);
        let (a, b) = p.color;
        let key = fx_key(p.tex, a.mix(&b, k));
        if p.key == Some(key) && mat.is_some() {
            continue;
        }
        p.key = Some(key);
        let handle = cache
            .cache
            .entry(key)
            .or_insert_with(|| {
                let (tex, [r, g, bl, al]) = key;
                let texture = match tex {
                    FxTex::Glow => Some(art.tex_glow.clone()),
                    FxTex::Ring => Some(art.tex_ring.clone()),
                    FxTex::Streak => Some(art.tex_streak.clone()),
                    FxTex::Solid => None,
                };
                materials.add(ColorMaterial {
                    color: Color::LinearRgba(LinearRgba::new(
                        r as f32 / 24.0,
                        g as f32 / 24.0,
                        bl as f32 / 24.0,
                        al as f32 / 24.0,
                    )),
                    texture,
                    alpha_mode: AlphaMode2d::Blend,
                    ..default()
                })
            })
            .clone();
        match mat {
            Some(mut m) => m.0 = handle,
            None => {
                commands.entity(e).insert(MeshMaterial2d(handle));
            }
        }
    }
}

fn update_floating_text(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut FloatingText, &mut Transform, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (e, mut f, mut t, mut color) in &mut q {
        f.life -= dt;
        if f.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        t.translation.y += 40.0 * dt;
        let k = f.life / f.max_life;
        color.0.set_alpha((k * 2.0).min(1.0));
        let pop = 1.0 + 0.4 * ((1.0 - k) * 12.0).min(1.0) * (1.0 - (1.0 - k) * 2.0).max(0.0);
        t.scale = Vec3::splat(pop);
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
        Mesh2d(art.quad.clone()),
        Transform::from_translation(pos).with_scale(p.size.0.extend(1.0)),
        p,
        DespawnOnExit(AppState::Playing),
    )
}

/// A fiery explosion. `size` is roughly the radius of the destroyed object.
pub fn explosion(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2, size: f32, ground: bool) {
    let s = (size / 24.0).clamp(0.5, 6.0);
    let z = if ground { z::SCORCH + 0.5 } else { z::FX };

    // Bright central flash.
    let mut flash = Particle::new(Vec2::ZERO, 0.18, Vec2::splat(70.0 * s), LinearRgba::rgb(9.0, 6.0, 3.0));
    flash.size.1 = Vec2::splat(150.0 * s);
    commands.spawn(particle_bundle(art, pos.extend(z + 0.3), flash));

    // Fireballs.
    let n = (6.0 + 5.0 * s) as usize;
    for _ in 0..n {
        let vel = rng.dir() * rng.range(30.0, 170.0) * s.sqrt();
        let life = rng.range(0.35, 0.8);
        let r = rng.range(22.0, 40.0) * s.sqrt();
        let mut p = Particle::new(vel, life, Vec2::splat(r), LinearRgba::rgb(5.0, 2.6, 0.6));
        p.size.1 = Vec2::splat(r * 1.8);
        p.color.1 = LinearRgba::new(0.6, 0.08, 0.02, 0.0);
        p.drag = 3.0;
        p.grounded = ground;
        commands.spawn(particle_bundle(art, (pos + rng.in_disc(8.0 * s)).extend(z + 0.2), p));
    }

    // Sparks.
    for _ in 0..(5.0 + 4.0 * s) as usize {
        let vel = rng.dir() * rng.range(250.0, 600.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.2, 0.45), Vec2::new(5.0, 22.0), LinearRgba::rgb(6.0, 4.0, 1.2));
        p.size.1 = Vec2::new(2.0, 8.0);
        p.drag = 4.0;
        p.align = true;
        commands.spawn(particle_bundle(art, pos.extend(z + 0.4), p.tex(FxTex::Streak)));
    }

    // Smoke.
    for _ in 0..(3.0 + 2.0 * s) as usize {
        let vel = rng.dir() * rng.range(10.0, 60.0);
        let r = rng.range(30.0, 50.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.9, 1.6), Vec2::splat(r), LinearRgba::new(0.12, 0.11, 0.1, 0.55));
        p.size.1 = Vec2::splat(r * 2.2);
        p.drag = 1.0;
        p.grounded = true;
        commands.spawn(particle_bundle(art, (pos + rng.in_disc(10.0 * s)).extend(z), p));
    }

    // Debris chunks.
    for _ in 0..(3.0 + 2.0 * s) as usize {
        let vel = rng.dir() * rng.range(100.0, 300.0) * s.sqrt();
        let d = rng.range(4.0, 8.0) * s.sqrt();
        let mut p = Particle::new(vel, rng.range(0.5, 0.9), Vec2::splat(d), LinearRgba::new(0.08, 0.07, 0.07, 1.0));
        p.spin = rng.range(-12.0, 12.0);
        p.drag = 2.5;
        p.size.1 = Vec2::splat(d * 0.6);
        commands.spawn(particle_bundle(art, pos.extend(z + 0.1), p.tex(FxTex::Solid)));
    }

    // Shockwave ring for larger blasts.
    if s >= 1.2 {
        let mut p = Particle::new(Vec2::ZERO, 0.4, Vec2::splat(30.0 * s), LinearRgba::new(2.0, 1.6, 1.2, 0.8));
        p.size.1 = Vec2::splat(260.0 * s.sqrt());
        commands.spawn(particle_bundle(art, pos.extend(z + 0.5), p.tex(FxTex::Ring)));
    }

    // Burnt patch left on the ground.
    if ground {
        commands.spawn((
            Sprite {
                image: art.tex_glow.clone(),
                custom_size: Some(Vec2::splat(70.0 * s)),
                color: Color::srgba(0.05, 0.04, 0.03, 0.6),
                ..default()
            },
            Transform::from_translation(pos.extend(z::SCORCH)),
            Scrolls,
            Offscreen::new(150.0),
            DespawnOnExit(AppState::Playing),
        ));
    }
}

/// Small spray of sparks where a bullet hits.
pub fn hit_sparks(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2, color: LinearRgba) {
    for _ in 0..3 {
        let vel = (Vec2::Y + rng.dir() * 0.9).normalize_or_zero() * rng.range(120.0, 320.0);
        let mut p = Particle::new(vel, rng.range(0.1, 0.22), Vec2::new(4.0, 14.0), color);
        p.align = true;
        p.drag = 5.0;
        commands.spawn(particle_bundle(art, pos.extend(z::FX), p.tex(FxTex::Streak)));
    }
    let mut glow = Particle::new(Vec2::ZERO, 0.08, Vec2::splat(26.0), color);
    glow.size.1 = Vec2::splat(10.0);
    commands.spawn(particle_bundle(art, pos.extend(z::FX), glow));
}

/// Generic glowing puff, e.g. muzzle flashes, pickup sparkles and trails.
pub fn puff(commands: &mut Commands, art: &Art, pos: Vec2, vel: Vec2, life: f32, size: (f32, f32), color: LinearRgba) {
    let mut p = Particle::new(vel, life, Vec2::splat(size.0), color);
    p.size.1 = Vec2::splat(size.1);
    p.drag = 1.5;
    commands.spawn(particle_bundle(art, pos.extend(z::FX - 1.0), p));
}

/// Expanding ring, e.g. for bombs and power-up pickups.
pub fn shockwave(commands: &mut Commands, art: &Art, pos: Vec2, from: f32, to: f32, life: f32, color: LinearRgba) {
    let mut p = Particle::new(Vec2::ZERO, life, Vec2::splat(from), color);
    p.size.1 = Vec2::splat(to);
    commands.spawn(particle_bundle(art, pos.extend(z::FX + 1.0), p.tex(FxTex::Ring)));
}

pub fn floating_text(commands: &mut Commands, pos: Vec2, text: impl Into<String>, color: Color, size: f32) {
    commands.spawn((
        Text2d::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        Transform::from_translation(pos.extend(z::TEXT)),
        FloatingText {
            life: 0.9,
            max_life: 0.9,
        },
        DespawnOnExit(AppState::Playing),
    ));
}

pub fn screen_flash(commands: &mut Commands, art: &Art, color: Color, alpha: f32, life: f32) {
    let mut p = Particle::new(Vec2::ZERO, life, Vec2::splat(4000.0), color.to_linear().with_alpha(alpha));
    p.real_time = true;
    commands.spawn(particle_bundle(art, Vec3::new(0.0, 0.0, z::FLASH), p.tex(FxTex::Solid)));
}
