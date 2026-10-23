//! The scrolling world below the planes: sea or desert, procedurally
//! sculpted islands, mesas and ice floes, and two layers of clouds that cast
//! shadows. Everything is real 3D geometry lit by the sun.

use crate::art::Art;
use crate::common::*;
use crate::model::{lin, rgb, Model};
use crate::noise::fbm;
use crate::rng::Rng;
use crate::state::{AppState, Run};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::Affine2;
use bevy::prelude::*;
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainKind {
    Tropical,
    Desert,
    Arctic,
}

pub struct Theme {
    pub name: &'static str,
    pub kind: TerrainKind,
    pub sky: Color,
}

pub const THEMES: [Theme; 3] = [
    Theme {
        name: "Coral Coast",
        kind: TerrainKind::Tropical,
        sky: Color::srgb(0.02, 0.12, 0.2),
    },
    Theme {
        name: "Ember Dunes",
        kind: TerrainKind::Desert,
        sky: Color::srgb(0.55, 0.4, 0.26),
    },
    Theme {
        name: "Frostbite Reach",
        kind: TerrainKind::Arctic,
        sky: Color::srgb(0.01, 0.04, 0.08),
    },
];

/// How far shadows fall per unit of height (matches the sun direction).
pub const SHADOW_SLANT: Vec2 = Vec2::new(0.23, -0.3);

const CHUNK_W: f32 = 960.0;
const CHUNK_H: f32 = 400.0;
const CHUNK_STEP: f32 = 12.0;
const WATER_TILE: f32 = 430.0;
const WHITECAP_TILE: f32 = 520.0;

#[derive(Resource)]
pub struct Backdrop {
    pub theme: usize,
    island_timer: f32,
    wildlife_timer: f32,
    ice_timer: f32,
    cloud_timer: f32,
    high_cloud_timer: f32,
    next_chunk: i32,
    water_offset: Vec2,
    whitecap_offset: Vec2,
    rng: Rng,
}

impl Backdrop {
    pub fn kind(&self) -> TerrainKind {
        THEMES[self.theme].kind
    }
}

/// Everything that belongs to the scenery (cleared when the stage changes).
#[derive(Component)]
pub struct BackdropItem;

/// Circular footprint used to keep islands from overlapping.
#[derive(Component, Clone, Copy)]
pub struct Footprint {
    pub radius: f32,
    /// Purely decorative islands may be cleared to make room for scripted ones.
    pub decorative: bool,
}

/// Ground units that drive over the dunes and must follow their height.
#[derive(Component)]
pub struct FollowGround;

/// A whale on its surfacing cycle. Pure scenery: nothing can hit it.
#[derive(Component)]
struct Whale {
    t: f32,
    heading: f32,
    speed: f32,
    spout: f32,
    splash: f32,
    phase: f32,
    surfaced: bool,
    called: bool,
    dived: bool,
}

#[derive(Component)]
struct WhaleTail;

#[derive(Component)]
struct WhaleFin {
    side: f32,
}

/// A lighthouse's rotating beam.
#[derive(Component)]
struct LighthouseBeam {
    speed: f32,
}

/// The lighthouse lamp, flashing as the beam sweeps round.
#[derive(Component)]
struct LighthouseLamp {
    beam: Entity,
}

/// A band of surf rolling in towards an island's shore.
#[derive(Component)]
struct SurfWave {
    t: f32,
    period: f32,
}

/// The whitecap layer drifting over the sea.
#[derive(Component)]
struct Whitecaps;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Backdrop {
            theme: 0,
            island_timer: 0.0,
            wildlife_timer: 4.0,
            ice_timer: 0.0,
            cloud_timer: 0.0,
            high_cloud_timer: 0.0,
            next_chunk: 0,
            water_offset: Vec2::ZERO,
            whitecap_offset: Vec2::ZERO,
            rng: Rng::new(77),
        })
        .add_systems(Startup, |mut c: Commands, mut m: ResMut<Assets<Mesh>>, art: Res<Art>, mut b: ResMut<Backdrop>, q: Query<Entity, With<BackdropItem>>, mut gs: ResMut<GroundScroll>| {
            reset(&mut c, &mut m, &art, &mut b, &q, &mut gs, 0);
        })
        .add_systems(
            OnEnter(AppState::Playing),
            |mut c: Commands, mut m: ResMut<Assets<Mesh>>, art: Res<Art>, mut b: ResMut<Backdrop>, q: Query<Entity, With<BackdropItem>>, mut gs: ResMut<GroundScroll>, run: Res<Run>| {
                reset(&mut c, &mut m, &art, &mut b, &q, &mut gs, run.stage);
            },
        )
        .add_systems(
            OnExit(AppState::Playing),
            |mut c: Commands, mut m: ResMut<Assets<Mesh>>, art: Res<Art>, mut b: ResMut<Backdrop>, q: Query<Entity, With<BackdropItem>>, mut gs: ResMut<GroundScroll>, run: Res<Run>| {
                reset(&mut c, &mut m, &art, &mut b, &q, &mut gs, run.stage);
            },
        )
        .add_systems(
            Update,
            (spawn_scenery, animate_surface, follow_ground, sweep_beams, flash_lamps, roll_surf, swim_whales, flex_whales),
        );
    }
}

fn reset(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    art: &Art,
    backdrop: &mut Backdrop,
    existing: &Query<Entity, With<BackdropItem>>,
    scroll: &mut GroundScroll,
    theme: usize,
) {
    for e in existing {
        commands.entity(e).try_despawn();
    }
    backdrop.theme = theme.min(THEMES.len() - 1);
    let theme_def = &THEMES[backdrop.theme];
    commands.insert_resource(ClearColor(theme_def.sky));
    scroll.distance = 0.0;
    backdrop.water_offset = Vec2::ZERO;
    backdrop.whitecap_offset = Vec2::ZERO;

    match theme_def.kind {
        TerrainKind::Desert => {
            backdrop.next_chunk = ((-HALF_H - CHUNK_H) / CHUNK_H).floor() as i32;
            fill_dunes(commands, meshes, art, backdrop, scroll.distance);
        }
        _ => {
            let water = Rectangle::new(2600.0, 3800.0).mesh().build().with_generated_tangents();
            if let Ok(water) = water {
                commands.spawn((
                    Mesh3d(meshes.add(water)),
                    MeshMaterial3d(art.mat_water[backdrop.theme].clone()),
                    Transform::from_xyz(0.0, 0.0, z::WATER),
                    NotShadowCaster,
                    BackdropItem,
                ));
            }
            commands.spawn((
                Whitecaps,
                Mesh3d(meshes.add(Rectangle::new(2600.0, 3800.0))),
                MeshMaterial3d(art.mat_whitecaps[backdrop.theme].clone()),
                Transform::from_xyz(0.0, 0.0, z::WATER + 0.2),
                NotShadowCaster,
                NotShadowReceiver,
                BackdropItem,
            ));
            commands.spawn((
                Mesh3d(art.quad.clone()),
                MeshMaterial3d(art.mat_floor[backdrop.theme].clone()),
                Transform::from_xyz(0.0, 0.0, z::SEA_FLOOR).with_scale(Vec3::new(2600.0, 3800.0, 1.0)),
                NotShadowCaster,
                NotShadowReceiver,
                BackdropItem,
            ));
        }
    }

    // Pre-populate the screen so a stage never starts empty.
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    let mut y = -HALF_H - 60.0;
    while y < HALF_H + 220.0 {
        let r = backdrop.rng.range(55.0, 125.0);
        if let Some(x) = free_x(&placed, y, footprint_radius(r), None, &mut backdrop.rng) {
            let mut rng = Rng::new(backdrop.rng.next_u64());
            let island = Island::new(&mut rng, r, theme_def.kind, true);
            spawn_island_entity(commands, meshes, art, &island, &mut rng, Vec2::new(x, y), true);
            placed.push((Vec2::new(x, y), island.footprint()));
        }
        y += backdrop.rng.range(240.0, 380.0);
    }
    for i in 0..3 {
        let p = Vec2::new(backdrop.rng.range(-HALF_W, HALF_W), -350.0 + i as f32 * 420.0);
        spawn_cloud(commands, art, &mut backdrop.rng, p, i % 2 == 0);
    }
}

fn spawn_scenery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    art: Res<Art>,
    time: Res<Time>,
    scroll: Res<ScrollSpeed>,
    ground: Res<GroundScroll>,
    mut b: ResMut<Backdrop>,
    islands: Query<(&Transform, &Footprint)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let kind = b.kind();
    let speed_k = scroll.0 / 70.0;

    if kind == TerrainKind::Desert {
        fill_dunes(&mut commands, &mut meshes, &art, &mut b, ground.distance);
    }

    b.island_timer -= dt * speed_k;
    if b.island_timer <= 0.0 {
        b.island_timer = b.rng.range(3.0, 5.5);
        let r = b.rng.range(50.0, 130.0);
        let mut rng = Rng::new(b.rng.next_u64());
        let island = Island::new(&mut rng, r, kind, true);
        let y = HALF_H + island.footprint() + 40.0;
        let existing: Vec<(Vec2, f32)> = islands.iter().map(|(t, f)| (t.translation.truncate(), f.radius)).collect();
        if let Some(x) = free_x(&existing, y, island.footprint(), None, &mut b.rng) {
            spawn_island_entity(&mut commands, &mut meshes, &art, &island, &mut rng, Vec2::new(x, y), true);
        }
    }

    if kind == TerrainKind::Arctic {
        // Whales surface now and then.
        b.wildlife_timer -= dt;
        if b.wildlife_timer <= 0.0 {
            b.wildlife_timer = b.rng.range(9.0, 16.0);
            let x = b.rng.range(-HALF_W + 80.0, HALF_W - 80.0);
            let heading = std::f32::consts::PI + b.rng.range(-0.7, 0.7);
            let whale = commands
                .spawn((
                    Mesh3d(art.whale_body.clone()),
                    MeshMaterial3d(art.mat_paint.clone()),
                    Transform::from_xyz(x, HALF_H + 120.0, -18.0).with_rotation(Quat::from_rotation_z(heading)),
                    Whale {
                        t: 0.0,
                        heading,
                        speed: b.rng.range(20.0, 32.0),
                        spout: 0.3,
                        splash: 0.0,
                        phase: b.rng.range(0.0, TAU),
                        surfaced: false,
                        called: false,
                        dived: false,
                    },
                    Scrolls,
                    Offscreen::new(220.0),
                    BackdropItem,
                ))
                .id();
            commands.spawn((
                Mesh3d(art.whale_tail.clone()),
                MeshMaterial3d(art.mat_paint.clone()),
                Transform::from_xyz(0.0, -22.0, 0.0),
                WhaleTail,
                ChildOf(whale),
            ));
            for (i, side) in [(0usize, 1.0f32), (1, -1.0)] {
                commands.spawn((
                    Mesh3d(art.whale_fins[i].clone()),
                    MeshMaterial3d(art.mat_paint.clone()),
                    Transform::from_xyz(13.0 * side, 24.0, -3.5),
                    WhaleFin { side },
                    ChildOf(whale),
                ));
            }
        }
        // Drifting sea ice.
        b.ice_timer -= dt * speed_k;
        while b.ice_timer <= 0.0 {
            b.ice_timer += b.rng.range(0.12, 0.4);
            let i = b.rng.int(0, art.ice_chunks.len() as i32 - 1) as usize;
            // Mostly small floes and brash ice, the odd big slab.
            let size = if b.rng.chance(0.12) { b.rng.range(18.0, 32.0) } else { b.rng.range(3.5, 12.0) };
            commands.spawn((
                Mesh3d(art.ice_chunks[i].clone()),
                MeshMaterial3d(art.mat_terrain.clone()),
                Transform::from_xyz(b.rng.range(-HALF_W - 40.0, HALF_W + 40.0), HALF_H + 40.0, 0.0)
                    .with_rotation(Quat::from_rotation_z(b.rng.range(0.0, TAU)))
                    .with_scale(Vec3::new(size, size * b.rng.range(0.6, 1.0), 1.0)),
                Scrolls,
                Offscreen::new(60.0),
                BackdropItem,
            ));
        }
    }

    b.cloud_timer -= dt;
    if b.cloud_timer <= 0.0 {
        b.cloud_timer = b.rng.range(4.0, 8.0);
        let p = Vec2::new(b.rng.range(-HALF_W, HALF_W), HALF_H + 300.0);
        spawn_cloud(&mut commands, &art, &mut b.rng, p, true);
    }
    b.high_cloud_timer -= dt;
    if b.high_cloud_timer <= 0.0 {
        b.high_cloud_timer = b.rng.range(3.0, 6.0);
        let p = Vec2::new(b.rng.range(-HALF_W, HALF_W), HALF_H + 320.0);
        spawn_cloud(&mut commands, &art, &mut b.rng, p, false);
    }
}

/// Scrolls the sea's wave normal map with the ground and lets it drift.
fn animate_surface(
    time: Res<Time>,
    scroll: Res<ScrollSpeed>,
    art: Res<Art>,
    mut b: ResMut<Backdrop>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || b.kind() == TerrainKind::Desert {
        return;
    }
    // The surface moves exactly with the ground (so islands sit in the sea)
    // plus a slow swell of its own; whitecaps drift with the wind.
    let ground = scroll.0 * dt;
    b.water_offset += Vec2::new(0.018 * dt, -ground / WATER_TILE - 0.012 * dt);
    b.whitecap_offset += Vec2::new(-0.01 * dt, -ground / WHITECAP_TILE - 0.028 * dt);
    let (water, caps) = (b.water_offset, b.whitecap_offset);
    if let Some(mut mat) = materials.get_mut(&art.mat_water[b.theme]) {
        mat.uv_transform = Affine2::from_scale_angle_translation(
            Vec2::new(2600.0 / WATER_TILE, 3800.0 / WATER_TILE),
            0.0,
            water,
        );
    }
    if let Some(mut mat) = materials.get_mut(&art.mat_whitecaps[b.theme]) {
        mat.uv_transform = Affine2::from_scale_angle_translation(
            Vec2::new(2600.0 / WHITECAP_TILE, 3800.0 / WHITECAP_TILE),
            0.0,
            caps,
        );
    }
}

/// Cruise below the surface, rise and breathe, then arch and dive tail-first.
fn swim_whales(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    mut sfx: MessageWriter<crate::audio::PlaySfx>,
    mut q: Query<(Entity, &mut Whale, &mut Transform)>,
) {
    use crate::audio::{PlaySfx, Sfx};
    use crate::effects::{foam, ripple};
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut w, mut t) in &mut q {
        w.t += dt;
        let tt = w.t;
        let dir = Vec2::from_angle(w.heading + std::f32::consts::FRAC_PI_2);
        let side = Vec2::new(-dir.y, dir.x);
        // Gentle weaving while swimming.
        w.heading += (tt * 0.4 + w.phase).sin() * 0.06 * dt;
        t.translation += (dir * w.speed * dt).extend(0.0);
        let (depth, pitch) = if tt < 3.0 {
            (-18.0, 0.0)
        } else if tt < 4.5 {
            let k = smoothstep(3.0, 4.5, tt);
            (-18.0 + 15.0 * k, -0.1 * k)
        } else if tt < 9.5 {
            (-3.0 + (tt * 1.6).sin() * 0.9, (tt * 1.6).cos() * 0.03)
        } else if tt < 12.0 {
            // Arch and dive: the nose drops and the flukes lift clear.
            let k = smoothstep(9.5, 12.0, tt);
            (-3.0 - 20.0 * k, 1.0 * k)
        } else {
            (-26.0, 1.0)
        };
        t.translation.z = depth;
        let roll = (tt * 0.9 + w.phase).sin() * 0.07;
        t.rotation = Quat::from_rotation_z(w.heading) * Quat::from_rotation_x(-pitch) * Quat::from_rotation_y(roll);
        let pos = t.translation.truncate();
        let at_sea = |p: Vec2| p.extend(0.7);

        // A call from below as it comes up, then a splash as it breaks surface.
        if tt > 2.8 && !w.called {
            w.called = true;
            sfx.write(PlaySfx::new(Sfx::WhaleCall).pitch(0.85 + (w.phase % 0.3)));
        }
        if tt > 4.2 && !w.surfaced {
            w.surfaced = true;
            ripple(&mut commands, &art, at_sea(pos), 50.0, 190.0, 1.4, 0.6);
            for k in 0..10 {
                let a = k as f32 / 10.0 * TAU;
                foam(&mut commands, &art, at_sea(pos + Vec2::from_angle(a) * 22.0), Vec2::from_angle(a) * 45.0, 1.1, (12.0, 34.0), 0.55);
            }
        }
        if (4.5..9.5).contains(&tt) {
            // Breathing: a bushy blow that rises and drifts downwind.
            w.spout -= dt;
            if w.spout <= 0.0 {
                w.spout = 1.9;
                let blowhole = pos + dir * 30.0;
                let wind = Vec2::new(16.0, -24.0);
                for k in 0..14 {
                    let rise = k as f32 / 13.0;
                    let spread = side * (if k % 2 == 0 { 1.0 } else { -1.0 }) * 6.0 * rise;
                    foam(
                        &mut commands,
                        &art,
                        (blowhole + spread).extend(8.0 + rise * 60.0),
                        wind * (0.4 + rise) + spread * 2.0,
                        1.2 + rise * 0.8,
                        (6.0 + rise * 10.0, 26.0 + rise * 46.0),
                        0.85 - rise * 0.35,
                    );
                }
                sfx.write(PlaySfx::new(Sfx::Spout).pitch(0.85 + (w.phase % 0.3)));
            }
            // Water rushing off its back and a wake behind.
            w.splash -= dt;
            if w.splash <= 0.0 {
                w.splash = 0.12;
                let along = dir * ((tt * 37.0).sin() * 28.0 + 6.0);
                for s in [-1.0, 1.0] {
                    foam(&mut commands, &art, at_sea(pos + along + side * s * 17.0), side * s * 22.0 - dir * 8.0, 1.3, (8.0, 24.0), 0.42);
                }
                foam(&mut commands, &art, at_sea(pos - dir * 55.0), -dir * 6.0, 1.8, (14.0, 34.0), 0.32);
            }
        }
        if (9.5..12.5).contains(&tt) {
            // Water streams off the flukes as they rise.
            w.splash -= dt;
            if w.splash <= 0.0 {
                w.splash = 0.08;
                let tail = pos - dir * (60.0 - 26.0 * smoothstep(9.5, 12.0, tt));
                for s in [-1.0, 1.0] {
                    let tip = tail + side * s * 26.0;
                    foam(&mut commands, &art, tip.extend(6.0 + 20.0 * smoothstep(9.5, 11.5, tt)), Vec2::ZERO, 0.9, (5.0, 18.0), 0.6);
                }
            }
        }
        if tt > 12.3 && !w.dived {
            // The smooth "footprint" a diving whale leaves on the surface.
            w.dived = true;
            ripple(&mut commands, &art, at_sea(pos - dir * 40.0), 40.0, 200.0, 3.0, 0.35);
        }
        if tt > 18.0 {
            commands.entity(e).try_despawn();
        }
    }
}

/// The tail beats up and down and the flippers sweep as the whale swims.
fn flex_whales(
    whales: Query<&Whale>,
    mut tails: Query<(&ChildOf, &mut Transform), (With<WhaleTail>, Without<WhaleFin>)>,
    mut fins: Query<(&ChildOf, &WhaleFin, &mut Transform), Without<WhaleTail>>,
) {
    for (parent, mut t) in &mut tails {
        if let Ok(w) = whales.get(parent.parent()) {
            let diving = smoothstep(9.5, 11.5, w.t);
            let beat = (w.t * 2.1 + w.phase).sin() * 0.32 * (1.0 - diving * 0.6);
            t.rotation = Quat::from_rotation_x(beat - diving * 0.45);
        }
    }
    for (parent, fin, mut t) in &mut fins {
        if let Ok(w) = whales.get(parent.parent()) {
            let sweep = 0.22 + (w.t * 1.15 + w.phase).sin() * 0.16;
            t.rotation = Quat::from_rotation_y(-fin.side * sweep) * Quat::from_rotation_z(fin.side * ((w.t * 0.8).sin() * 0.12));
        }
    }
}

/// Surf bands roll in from offshore and break on the beach, then repeat.
fn roll_surf(
    time: Res<Time>,
    art: Res<Art>,
    mut q: Query<(&mut SurfWave, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let dt = time.delta_secs();
    for (mut wave, mut t, mut mat) in &mut q {
        wave.t = (wave.t + dt / wave.period).fract();
        let u = wave.t;
        let s = 1.13 - 0.13 * u.powf(0.8);
        t.scale = Vec3::new(s, s, 1.0);
        // Fades in offshore, brightest as it breaks, gone once it's ashore.
        let alpha = (u * std::f32::consts::PI).sin().powf(1.5) * if u > 0.85 { (1.0 - u) / 0.15 } else { 1.0 };
        let level = ((alpha * 9.0).round() as usize).min(art.mat_surf.len() - 1);
        if mat.0 != art.mat_surf[level] {
            mat.0 = art.mat_surf[level].clone();
        }
    }
}

/// Lamps flash each time one of the beams sweeps past the viewer.
fn flash_lamps(
    time: Res<Time>,
    beams: Query<&Transform, (With<LighthouseBeam>, Without<LighthouseLamp>)>,
    mut lamps: Query<(&LighthouseLamp, &mut Transform), Without<LighthouseBeam>>,
) {
    let pulse = 1.0 + 0.15 * (time.elapsed_secs() * 9.0).sin();
    for (lamp, mut t) in &mut lamps {
        let Ok(beam) = beams.get(lamp.beam) else { continue };
        let dir = (beam.rotation * Vec3::Y).truncate();
        // Brightest when a beam points down the screen, towards the viewer.
        let facing = dir.dot(Vec2::NEG_Y).abs().powf(8.0);
        let size = (22.0 + 70.0 * facing) * pulse;
        t.scale = Vec3::new(size, size, 1.0);
    }
}

fn sweep_beams(time: Res<Time>, mut q: Query<(&LighthouseBeam, &mut Transform)>) {
    for (beam, mut t) in &mut q {
        t.rotate_z(beam.speed * time.delta_secs());
    }
}

fn follow_ground(ground: Res<GroundScroll>, b: Res<Backdrop>, mut q: Query<&mut Transform, With<FollowGround>>) {
    if b.kind() != TerrainKind::Desert {
        return;
    }
    for mut t in &mut q {
        t.translation.z = dune_height(t.translation.x, t.translation.y + ground.distance) - 1.0;
    }
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

pub fn footprint_radius(r: f32) -> f32 {
    r * 1.55 + 20.0
}

/// Finds an x position at height `y` where a circle of `radius` doesn't
/// overlap any `existing` footprint. Prefers `preferred` when given.
pub fn free_x(existing: &[(Vec2, f32)], y: f32, radius: f32, preferred: Option<f32>, rng: &mut Rng) -> Option<f32> {
    let lo = -HALF_W - 60.0;
    let hi = HALF_W + 60.0;
    let fits = |x: f32| {
        existing
            .iter()
            .all(|(c, r)| c.distance(Vec2::new(x, y)) > r + radius)
    };
    let mut candidates: Vec<f32> = (0..24).map(|i| lo + (hi - lo) * i as f32 / 23.0).collect();
    match preferred {
        Some(p) => {
            candidates.push(p);
            candidates.sort_by(|a, b| (a - p).abs().total_cmp(&(b - p).abs()));
        }
        None => {
            // Shuffle so decorative islands don't line up.
            for i in (1..candidates.len()).rev() {
                let j = rng.int(0, i as i32) as usize;
                candidates.swap(i, j);
            }
        }
    }
    candidates.into_iter().find(|&x| fits(x))
}

// ---------------------------------------------------------------------------
// Islands, mesas and ice floes
// ---------------------------------------------------------------------------

/// A procedurally shaped landmass. Heights are in world units above sea level.
pub struct Island {
    pub radius: f32,
    kind: TerrainKind,
    stretch: Vec2,
    phases: [f32; 3],
    seed: u32,
    peak: f32,
    /// Icebergs: flat-topped tabular bergs (which can carry ground targets)
    /// rather than jagged peaks.
    tabular: bool,
}

impl Island {
    pub fn new(rng: &mut Rng, radius: f32, kind: TerrainKind, decorative: bool) -> Self {
        let peak = match kind {
            TerrainKind::Tropical => rng.range(18.0, 40.0),
            TerrainKind::Desert => rng.range(34.0, 44.0),
            TerrainKind::Arctic => 0.0,
        };
        let tabular = kind == TerrainKind::Arctic && (!decorative || rng.chance(0.35));
        let peak = match kind {
            TerrainKind::Arctic if tabular => rng.range(12.0, 17.0),
            TerrainKind::Arctic => rng.range(26.0, 48.0),
            _ => peak,
        };
        Self {
            radius,
            kind,
            stretch: Vec2::new(rng.range(0.85, 1.3), rng.range(0.8, 1.15)),
            phases: [rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU)],
            seed: rng.next_u64() as u32,
            peak,
            tabular,
        }
    }

    pub fn footprint(&self) -> f32 {
        footprint_radius(self.radius * self.stretch.max_element())
    }

    /// Normalised distance from the centre: 0 at the middle, 1 at the coast.
    fn t(&self, p: Vec2) -> f32 {
        let q = p / self.stretch;
        let a = q.to_angle();
        let edge = 1.0
            + 0.16 * (3.0 * a + self.phases[0]).sin()
            + 0.09 * (5.0 * a + self.phases[1]).sin()
            + 0.05 * (9.0 * a + self.phases[2]).sin();
        q.length() / (self.radius * edge)
    }

    /// 0..1: how rocky the coast is in the direction of `p`.
    fn rocky(&self, p: Vec2) -> f32 {
        let a = (p / self.stretch).to_angle();
        let d = Vec2::new(a.cos(), a.sin()) * 1.7 + Vec2::splat(self.seed as f32 % 53.0);
        smoothstep(0.56, 0.7, fbm(d, self.seed + 21, 3))
    }

    /// A point on the coastline at angle `a`, scaled by `k` (1 = waterline).
    fn coast_point(&self, a: f32, k: f32) -> Vec2 {
        let edge = 1.0
            + 0.16 * (3.0 * a + self.phases[0]).sin()
            + 0.09 * (5.0 * a + self.phases[1]).sin()
            + 0.05 * (9.0 * a + self.phases[2]).sin();
        Vec2::from_angle(a) * self.radius * edge * k * self.stretch
    }

    /// Surf hugging the waterline: a soft white band, brightest at the shore.
    fn foam_mesh(&self) -> Option<Mesh> {
        if self.kind == TerrainKind::Desert {
            return None;
        }
        let rings = [(0.985, 0.0), (1.0, 0.75), (1.025, 0.4), (1.07, 0.0)];
        let segs = 120;
        let mut m = Model::new();
        let mut ids = Vec::new();
        for j in 0..=segs {
            let a = j as f32 / segs as f32 * TAU;
            // Break the surf up a little so it isn't a perfect outline.
            let wobble = 0.6 + 0.8 * fbm(Vec2::new(a.cos(), a.sin()) * 3.0, self.seed + 31, 2);
            for (k, alpha) in rings {
                let p = self.coast_point(a, k);
                ids.push(m.vertex(p.extend(0.35), Vec3::Z, [1.0, 1.0, 1.0, (alpha * wobble).min(0.9)]));
            }
        }
        let n = rings.len() as u32;
        for j in 0..segs as u32 {
            for r in 0..n - 1 {
                let a = ids[(j * n + r) as usize];
                let b = ids[(j * n + r + 1) as usize];
                let c = ids[((j + 1) * n + r + 1) as usize];
                let d = ids[((j + 1) * n + r) as usize];
                m.triangle(a, c, b);
                m.triangle(a, d, c);
            }
        }
        Some(m.build())
    }

    /// A thin band of breaking surf along the coast; scaled outwards and
    /// animated by `roll_surf` so it rolls onto the beach.
    fn surf_mesh(&self) -> Option<Mesh> {
        if self.kind == TerrainKind::Desert {
            return None;
        }
        let rings = [(0.99, 0.0), (1.0, 0.9), (1.012, 0.6), (1.03, 0.0)];
        let segs = 120;
        let mut m = Model::new();
        let mut ids = Vec::new();
        for j in 0..=segs {
            let a = j as f32 / segs as f32 * TAU;
            let broken = 0.45 + 0.75 * fbm(Vec2::new(a.cos(), a.sin()) * 5.0, self.seed + 41, 2);
            for (k, alpha) in rings {
                let p = self.coast_point(a, k);
                ids.push(m.vertex(p.extend(0.0), Vec3::Z, [1.0, 1.0, 1.0, (alpha * broken).min(1.0)]));
            }
        }
        let n = rings.len() as u32;
        for j in 0..segs as u32 {
            for r in 0..n - 1 {
                let a = ids[(j * n + r) as usize];
                let b = ids[(j * n + r + 1) as usize];
                let c = ids[((j + 1) * n + r + 1) as usize];
                let d = ids[((j + 1) * n + r) as usize];
                m.triangle(a, c, b);
                m.triangle(a, d, c);
            }
        }
        Some(m.build())
    }

    /// Height of the surface at a point relative to the island centre.
    pub fn height(&self, p: Vec2) -> f32 {
        let t = self.t(p);
        let noise = || fbm(p * 0.02 + Vec2::splat(self.seed as f32 % 97.0), self.seed, 4);
        match self.kind {
            TerrainKind::Tropical => {
                if t < 1.0 {
                    let u = 1.0 - t;
                    let beach = 2.2 * smoothstep(0.0, 0.12, u);
                    // Rocky stretches of coast rise as low cliffs instead of beach.
                    let cliff = self.rocky(p) * 9.0 * smoothstep(0.0, 0.05, u);
                    let hills = self.peak * smoothstep(0.05, 0.85, u).powf(1.3) * (0.5 + 0.7 * noise());
                    beach.max(cliff) + hills + 3.0 * fbm(p * 0.06, self.seed + 3, 3) * smoothstep(0.1, 0.4, u)
                } else {
                    -22.0 * smoothstep(1.0, 1.45, t) - 0.4
                }
            }
            TerrainKind::Desert => {
                let cliff = smoothstep(1.0, 0.86, t);
                let top = self.peak + 5.0 * noise();
                let talus = 8.0 * smoothstep(1.12, 0.98, t);
                (cliff * top).max(talus) - 6.0 * (1.0 - smoothstep(1.12, 1.0, t))
            }
            TerrainKind::Arctic => {
                if self.tabular {
                    if t < 0.86 {
                        self.peak
                    } else if t < 1.0 {
                        self.peak * (1.0 - t) / 0.14
                    } else {
                        -9.0 * smoothstep(1.0, 1.18, t)
                    }
                } else if t < 1.0 {
                    self.peak * (1.0 - t).powf(0.9)
                } else {
                    -9.0 * smoothstep(1.0, 1.18, t)
                }
            }
        }
    }

    fn color(&self, p: Vec2, h: f32, slope: f32, rng_n: f32) -> Color {
        match self.kind {
            TerrainKind::Tropical => {
                let rocky = self.rocky(p);
                let rock = mix(rgb(0.52, 0.49, 0.44), rgb(0.4, 0.38, 0.35), rng_n);
                // Where vegetation starts wanders up and down the beach.
                let beach_top = 1.6 + 1.8 * fbm(p * 0.05, self.seed + 13, 2);
                if h < -0.6 {
                    // Bright sandy shallows that fade to the colour of the deep
                    // sea floor, so the reef's outer edge disappears.
                    let k = (-h / 21.0).clamp(0.0, 1.0).powf(0.7);
                    let shallows = mix(rgb(0.78, 0.92, 0.82), rgb(0.35, 0.74, 0.72), (k * 3.0).min(1.0));
                    mix(shallows, rgb(0.012, 0.06, 0.12), k)
                } else if slope > 1.35 || (rocky > 0.5 && h < 11.0 && slope > 0.35) {
                    rock
                } else if h < 0.7 {
                    // Wet sand at the waterline.
                    mix(rgb(0.76, 0.68, 0.52), rgb(0.7, 0.62, 0.47), rng_n)
                } else if h < beach_top {
                    mix(rgb(0.96, 0.9, 0.7), rgb(0.89, 0.81, 0.6), rng_n)
                } else if h < beach_top + 1.5 {
                    // Dune grass between the beach and the forest.
                    mix(rgb(0.66, 0.68, 0.38), rgb(0.5, 0.6, 0.3), rng_n)
                } else if h > self.peak * 0.95 {
                    rock
                } else {
                    let lush = fbm(p * 0.035, self.seed + 9, 3);
                    mix(rgb(0.42, 0.64, 0.28), rgb(0.24, 0.46, 0.18), lush)
                }
            }
            TerrainKind::Desert => {
                if slope > 0.8 {
                    // Banded sandstone strata on the cliffs.
                    let band = ((h / 5.5) + fbm(p * 0.05, self.seed, 2) * 1.5).floor() as i32;
                    match band.rem_euclid(3) {
                        0 => rgb(0.66, 0.4, 0.25),
                        1 => rgb(0.76, 0.5, 0.32),
                        _ => rgb(0.58, 0.34, 0.22),
                    }
                } else if h > 12.0 {
                    mix(rgb(0.78, 0.6, 0.42), rgb(0.68, 0.5, 0.34), rng_n)
                } else {
                    mix(rgb(0.82, 0.63, 0.42), rgb(0.7, 0.5, 0.33), rng_n)
                }
            }
            TerrainKind::Arctic => {
                if h < -0.4 {
                    let k = (-h / 9.6).clamp(0.0, 1.0).powf(0.7);
                    mix(rgb(0.55, 0.75, 0.85), rgb(0.006, 0.025, 0.05), k)
                } else if slope > 1.0 {
                    mix(rgb(0.42, 0.44, 0.48), rgb(0.33, 0.35, 0.38), rng_n)
                } else {
                    mix(rgb(0.95, 0.97, 1.0), rgb(0.8, 0.88, 0.96), rng_n * 0.7)
                }
            }
        }
    }

    fn mesh(&self, rng: &mut Rng) -> Mesh {
        if self.kind == TerrainKind::Arctic {
            return self.iceberg_mesh(rng);
        }
        let mut m = Model::new();
        let extent = self.radius * self.stretch.max_element() * match self.kind {
            TerrainKind::Tropical => 1.5,
            TerrainKind::Desert => 1.16,
            TerrainKind::Arctic => 1.3,
        };
        let cells = ((extent * 2.0) / 7.0).clamp(24.0, 60.0) as i32;
        let step = extent * 2.0 / cells as f32;
        let floor = match self.kind {
            TerrainKind::Tropical => -21.8,
            TerrainKind::Desert => -5.9,
            TerrainKind::Arctic => -9.8,
        };
        let n = (cells + 1) as usize;
        let mut heights = vec![0.0f32; n * n];
        let mut ids = vec![0u32; n * n];
        for j in 0..n {
            for i in 0..n {
                let p = Vec2::new(-extent + i as f32 * step, -extent + j as f32 * step);
                let h = self.height(p);
                let dx = self.height(p + Vec2::X * step * 0.5) - self.height(p - Vec2::X * step * 0.5);
                let dy = self.height(p + Vec2::Y * step * 0.5) - self.height(p - Vec2::Y * step * 0.5);
                let normal = Vec3::new(-dx / step, -dy / step, 1.0);
                let slope = Vec2::new(dx, dy).length() / step;
                let c = self.color(p, h, slope, fbm(p * 0.09, self.seed + 5, 2));
                heights[j * n + i] = h;
                ids[j * n + i] = m.vertex(p.extend(h), normal, lin(c));
            }
        }
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                let k = [j * n + i, j * n + i + 1, (j + 1) * n + i + 1, (j + 1) * n + i];
                if k.iter().all(|&k| heights[k] <= floor) {
                    continue;
                }
                m.triangle(ids[k[0]], ids[k[1]], ids[k[2]]);
                m.triangle(ids[k[0]], ids[k[2]], ids[k[3]]);
            }
        }
        self.decorate(&mut m, rng);
        m.build()
    }

    /// A faceted iceberg: hard-edged layers narrowing to a jagged peak (or a
    /// flat table), snow on top, glacial blue faces and a pale mass below
    /// the waterline.
    fn iceberg_mesh(&self, rng: &mut Rng) -> Mesh {
        let mut m = Model::new();
        let n = 13;
        let angles: Vec<f32> = (0..n).map(|i| (i as f32 + rng.range(-0.3, 0.3)) / n as f32 * TAU).collect();
        let mut ring = |k: f32, z: f32, jitter: f32, wobble: f32| -> Vec<Vec3> {
            let centre = Vec2::new(rng.range(-wobble, wobble), rng.range(-wobble, wobble)) * self.radius;
            angles
                .iter()
                .map(|&a| (self.coast_point(a, k * (1.0 + rng.range(-jitter, jitter))) + centre).extend(z + rng.range(-1.0, 1.0) * jitter * 20.0))
                .collect()
        };
        let h = self.peak;
        let layers = if self.tabular {
            vec![ring(1.17, -9.0, 0.04, 0.0), ring(1.0, 0.0, 0.0, 0.0), ring(0.93, h * 0.55, 0.03, 0.0), ring(0.86, h, 0.0, 0.0)]
        } else {
            vec![
                ring(1.18, -9.0, 0.04, 0.0),
                ring(1.0, 0.0, 0.0, 0.0),
                ring(0.74, h * 0.38, 0.12, 0.05),
                ring(0.46, h * 0.72, 0.2, 0.1),
                ring(0.2, h * 0.92, 0.3, 0.12),
            ]
        };
        let facet = |m: &mut Model, a: Vec3, b: Vec3, c: Vec3, tone: f32| {
            let normal = (b - a).cross(c - a).normalize_or(Vec3::Z);
            let mid = (a.z + b.z + c.z) / 3.0;
            let color = if mid < -0.5 {
                mix(rgb(0.45, 0.76, 0.84), rgb(0.12, 0.3, 0.38), (-mid / 9.0).clamp(0.0, 1.0))
            } else if normal.z > 0.8 {
                rgb(0.95, 0.97, 1.0)
            } else if normal.z > 0.45 {
                mix(rgb(0.84, 0.93, 0.99), rgb(0.74, 0.88, 0.97), tone)
            } else {
                mix(rgb(0.56, 0.8, 0.93), rgb(0.42, 0.68, 0.86), tone)
            };
            m.tri(a, b, c, color);
        };
        for l in 0..layers.len() - 1 {
            for i in 0..n {
                let (a, b) = (layers[l][i], layers[l][(i + 1) % n]);
                let (c, d) = (layers[l + 1][(i + 1) % n], layers[l + 1][i]);
                let tone = ((i * 5 + l * 3) % 7) as f32 / 6.0;
                facet(&mut m, a, b, c, tone);
                facet(&mut m, a, c, d, 1.0 - tone);
            }
        }
        // Cap: a flat table, or a jagged summit.
        let top = layers.last().cloned().unwrap_or_default();
        let centroid = top.iter().copied().sum::<Vec3>() / top.len().max(1) as f32;
        let apex = if self.tabular {
            centroid.truncate().extend(h + 0.6)
        } else {
            (centroid.truncate() + rng.in_disc(self.radius * 0.08)).extend(h * 1.05)
        };
        for i in 0..n {
            facet(&mut m, apex, top[i], top[(i + 1) % n], 0.5);
        }
        // Snowy flats sometimes carry a little colony of seals.
        if self.tabular {
            for _ in 0..rng.int(0, 4) {
                let p = centroid.truncate() + rng.in_disc(self.radius * 0.45);
                m.ellipsoid(p.extend(h + 1.0), Vec3::new(2.0, 4.2, 1.6), rgb(0.24, 0.24, 0.27), true);
            }
        }
        m.build()
    }

    /// Trees, pines and boulders, merged into the island mesh.
    fn decorate(&self, m: &mut Model, rng: &mut Rng) {
        let attempts = (self.radius * self.radius / 260.0) as usize;
        for _ in 0..attempts {
            let p = Vec2::from_angle(rng.range(0.0, TAU)) * rng.f32().sqrt() * self.radius * 0.85 * self.stretch;
            let h = self.height(p);
            let slope = (self.height(p + Vec2::X * 3.0) - self.height(p - Vec2::X * 3.0)).abs() / 6.0;
            match self.kind {
                TerrainKind::Tropical => {
                    if h < 3.0 || slope > 0.8 || fbm(p * 0.03, self.seed + 9, 3) < 0.42 {
                        continue;
                    }
                    let r = rng.range(5.0, 9.5);
                    let green = mix(rgb(0.16, 0.38, 0.14), rgb(0.26, 0.5, 0.2), rng.f32());
                    m.ellipsoid(p.extend(h + r * 0.4), Vec3::new(r, r, r * 0.85), green, true);
                }
                TerrainKind::Arctic => {
                    if h < 3.5 || slope > 0.7 || rng.chance(0.55) {
                        continue;
                    }
                    let r = rng.range(4.0, 6.5);
                    let base = p.extend(h);
                    m.cone(base, base + Vec3::Z * r * 2.4, r, 6, rgb(0.12, 0.3, 0.26));
                    m.cone(base + Vec3::Z * r * 1.5, base + Vec3::Z * r * 2.6, r * 0.45, 6, rgb(0.92, 0.95, 1.0));
                }
                TerrainKind::Desert => {
                    if h < self.peak * 0.8 || rng.chance(0.7) {
                        continue;
                    }
                    // Dry scrub on the mesa tops.
                    let r = rng.range(2.5, 4.5);
                    let scrub = mix(rgb(0.4, 0.38, 0.22), rgb(0.5, 0.46, 0.28), rng.f32());
                    m.ellipsoid(p.extend(h + r * 0.3), Vec3::new(r, r, r * 0.7), scrub, true);
                }
            }
        }
        if self.kind == TerrainKind::Tropical {
            // Boulders strewn along the shore, mostly on the rocky stretches.
            let rock_colors = [rgb(0.52, 0.49, 0.44), rgb(0.44, 0.42, 0.39), rgb(0.58, 0.54, 0.47)];
            for _ in 0..(self.radius / 14.0) as usize {
                let a = rng.range(0.0, TAU);
                let centre = self.coast_point(a, rng.range(0.95, 1.05));
                if self.rocky(centre) < 0.3 && rng.chance(0.6) {
                    continue;
                }
                for _ in 0..rng.int(2, 5) {
                    let p = centre + rng.in_disc(10.0);
                    let r = rng.range(3.0, 7.5);
                    let size = Vec3::new(r * rng.range(0.8, 1.3), r * rng.range(0.8, 1.2), r * rng.range(0.6, 1.0));
                    let color = rock_colors[rng.int(0, 2) as usize];
                    let h = self.height(p).max(-2.0);
                    let mut jitter = Rng::new(rng.next_u64());
                    m.rock(p.extend(h), size, color, move || jitter.range(0.72, 1.15));
                }
            }
            // A couple of sea stacks standing in the shallows.
            for _ in 0..rng.int(0, 2) {
                let p = self.coast_point(rng.range(0.0, TAU), rng.range(1.07, 1.18));
                let r = rng.range(6.0, 10.0);
                let size = Vec3::new(r, r * rng.range(0.8, 1.1), rng.range(12.0, 22.0));
                let h = self.height(p);
                let mut jitter = Rng::new(rng.next_u64());
                m.rock(p.extend(h), size, rock_colors[1], move || jitter.range(0.75, 1.15));
            }
            // Rocky outcrops on the hillsides.
            for _ in 0..(self.radius / 30.0) as usize {
                let p = Vec2::from_angle(rng.range(0.0, TAU)) * rng.f32().sqrt() * self.radius * 0.6 * self.stretch;
                let h = self.height(p);
                if h < 4.0 {
                    continue;
                }
                let r = rng.range(4.0, 8.0);
                let mut jitter = Rng::new(rng.next_u64());
                m.rock(p.extend(h - 1.0), Vec3::new(r * 1.2, r, r * 0.7), rock_colors[2], move || jitter.range(0.75, 1.15));
            }
        }
        if self.kind == TerrainKind::Desert {
            // Fallen boulders around the foot of the cliffs.
            for _ in 0..(self.radius / 9.0) as usize {
                let a = rng.range(0.0, TAU);
                let p = Vec2::from_angle(a) * self.radius * rng.range(1.0, 1.12) * self.stretch;
                let r = rng.range(3.0, 7.0);
                boulder(m, p.extend(self.height(p).max(0.0)), r, rng);
            }
        }
    }
}

fn boulder(m: &mut Model, at: Vec3, r: f32, rng: &mut Rng) {
    let c = mix(rgb(0.5, 0.38, 0.28), rgb(0.62, 0.48, 0.34), rng.f32());
    m.ellipsoid(at, Vec3::new(r * rng.range(0.9, 1.3), r * rng.range(0.8, 1.1), r * 0.6), c, true);
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    let t = t.clamp(0.0, 1.0);
    Color::linear_rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn spawn_island_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    art: &Art,
    island: &Island,
    rng: &mut Rng,
    pos: Vec2,
    decorative: bool,
) -> Entity {
    let base_z = if island.kind == TerrainKind::Desert { 0.0 } else { z::WATER };
    let foam = island.foam_mesh().map(|m| meshes.add(m));
    let mut entity = commands.spawn((
            Mesh3d(meshes.add(island.mesh(rng))),
            MeshMaterial3d(art.mat_terrain.clone()),
            Transform::from_translation(pos.extend(base_z)),
            Footprint {
                radius: island.footprint(),
                decorative,
            },
            Scrolls,
            Offscreen::new(island.footprint() + 40.0),
            BackdropItem,
        ));
    if let Some(foam) = foam {
        entity.with_child((
            Mesh3d(foam),
            MeshMaterial3d(art.mat_foam.clone()),
            NotShadowCaster,
        ));
    }
    if let Some(surf) = island.surf_mesh() {
        let surf = meshes.add(surf);
        // Two sets of waves, out of step, so there's always one coming in.
        for phase in [0.0, 0.5] {
            entity.with_child((
                Mesh3d(surf.clone()),
                MeshMaterial3d(art.mat_surf[0].clone()),
                Transform::from_xyz(0.0, 0.0, 0.3),
                SurfWave {
                    t: phase + rng.range(0.0, 0.2),
                    period: rng.range(2.6, 3.4),
                },
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
    let id = entity.id();
    // Now and then a decorative island gets a lighthouse on its shore.
    if decorative && island.kind == TerrainKind::Tropical && rng.chance(0.3) {
        let a = (0..12)
            .map(|_| rng.range(0.0, TAU))
            .max_by(|a, b| island.rocky(island.coast_point(*a, 0.9)).total_cmp(&island.rocky(island.coast_point(*b, 0.9))))
            .unwrap_or(0.0);
        let at = island.coast_point(a, 0.9);
        let h = island.height(at).max(0.5);
        let tower = commands
            .spawn((
                Mesh3d(art.lighthouse.paint.clone()),
                MeshMaterial3d(art.mat_paint.clone()),
                Transform::from_translation(at.extend(h)).with_rotation(Quat::from_rotation_z(a)),
                ChildOf(id),
            ))
            .id();
        crate::common::attach_model_parts(commands, tower, &art.lighthouse, art);
        let lantern = at.extend(h + 39.0);
        let beam = commands
            .spawn((
                Mesh3d(art.beam.clone()),
                MeshMaterial3d(art.mat_beam.clone()),
                Transform::from_translation(lantern).with_rotation(Quat::from_rotation_z(rng.range(0.0, TAU))),
                LighthouseBeam { speed: rng.range(0.9, 1.4) },
                NotShadowCaster,
                NotShadowReceiver,
                ChildOf(id),
            ))
            .id();
        commands.spawn((
            Mesh3d(art.quad.clone()),
            MeshMaterial3d(art.mat_lamp.clone()),
            Transform::from_translation(lantern + Vec3::Z * 4.0).with_scale(Vec3::new(30.0, 30.0, 1.0)),
            LighthouseLamp { beam },
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(id),
        ));
        // A real warm light that washes over the island and the water.
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.85, 0.6),
                intensity: 1.2e8,
                range: 240.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(lantern),
            ChildOf(id),
        ));
    }
    id
}

/// Spawns a scripted island for ground targets. It's nudged sideways if it
/// would overlap another island, and decorative islands still hidden above
/// the screen are cleared out of the way. Returns the island and its centre.
pub fn spawn_feature_island(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    art: &Art,
    kind: TerrainKind,
    rng: &mut Rng,
    x: f32,
    radius: f32,
    existing: &[(Entity, Vec2, Footprint)],
) -> (Island, Vec2) {
    let island = Island::new(rng, radius, kind, false);
    let y = HALF_H + island.footprint() * 0.75 + 30.0;
    let fp = island.footprint();
    // Hidden decorative islands in the way are simply removed.
    let mut keep: Vec<(Vec2, f32)> = Vec::new();
    for (e, c, f) in existing {
        let hidden = c.y - f.radius > HALF_H;
        let overlaps = c.distance(Vec2::new(x, y)) < f.radius + fp;
        if f.decorative && hidden && overlaps {
            commands.entity(*e).try_despawn();
        } else {
            keep.push((*c, f.radius));
        }
    }
    let x = free_x(&keep, y, fp, Some(x), rng)
        .map(|fx| fx.clamp(-HALF_W + radius * 0.6, HALF_W - radius * 0.6))
        .unwrap_or(x);
    let centre = Vec2::new(x, y);
    spawn_island_entity(commands, meshes, art, &island, rng, centre, false);
    (island, centre)
}

// ---------------------------------------------------------------------------
// Desert dunes
// ---------------------------------------------------------------------------

/// Height of the dune field at screen `x` and world `y` (screen y plus the
/// distance scrolled), so chunks join seamlessly.
pub fn dune_height(x: f32, wy: f32) -> f32 {
    let p = Vec2::new(x, wy);
    let warp = fbm(p * 0.0022, 41, 3) * 260.0;
    let coord = (wy + warp + 0.3 * x) / 175.0;
    let phase = coord - coord.floor();
    // Short steep slip face (facing away from the sun, so it falls into
    // shadow), then a long gentle windward slope.
    let profile = if phase < 0.24 { phase / 0.24 } else { (1.0 - phase) / 0.76 };
    let profile = profile * profile * (3.0 - 2.0 * profile);
    let amp = 16.0 + 20.0 * fbm(p * 0.003, 43, 3);
    profile * amp + 6.0 * fbm(p * 0.011, 47, 3)
}

fn fill_dunes(commands: &mut Commands, meshes: &mut Assets<Mesh>, art: &Art, b: &mut Backdrop, distance: f32) {
    while (b.next_chunk as f32) * CHUNK_H - distance < HALF_H + CHUNK_H {
        let k = b.next_chunk;
        b.next_chunk += 1;
        let world_y0 = k as f32 * CHUNK_H;
        // Chunk indices start negative (below the screen), so offset first.
        let mut rng = Rng::new((k as i64 + 1_000_000) as u64);
        if let Ok(mesh) = dune_chunk(world_y0, &mut rng).build().with_generated_tangents() {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(art.mat_dunes.clone()),
                Transform::from_xyz(0.0, world_y0 - distance, 0.0),
                Scrolls,
                Offscreen::new(CHUNK_H + 30.0),
                BackdropItem,
            ));
        }
    }
}

fn dune_chunk(world_y0: f32, rng: &mut Rng) -> Model {
    let mut m = Model::new();
    // Steps that divide the chunk exactly, so neighbouring chunks meet.
    let cells_x = (CHUNK_W / CHUNK_STEP).round() as usize;
    let cells_y = (CHUNK_H / CHUNK_STEP).round() as usize;
    let (sx, sy) = (CHUNK_W / cells_x as f32, CHUNK_H / cells_y as f32);
    let (nx, ny) = (cells_x + 1, cells_y + 1);
    let mut ids = Vec::with_capacity(nx * ny);
    let e = CHUNK_STEP * 0.5;
    for j in 0..ny {
        for i in 0..nx {
            let x = -CHUNK_W / 2.0 + i as f32 * sx;
            let ly = j as f32 * sy;
            let wy = world_y0 + ly;
            let h = dune_height(x, wy);
            let dx = dune_height(x + e, wy) - dune_height(x - e, wy);
            let dy = dune_height(x, wy + e) - dune_height(x, wy - e);
            let normal = Vec3::new(-dx / (2.0 * e), -dy / (2.0 * e), 1.0);
            let p = Vec2::new(x, wy);
            let tone = fbm(p * 0.008, 51, 3);
            let mut c = mix(rgb(0.88, 0.68, 0.45), rgb(0.78, 0.55, 0.34), tone);
            // Wind-scoured hardpan in some troughs.
            let flats = fbm(p * 0.004, 53, 3);
            if h < 4.0 && flats > 0.62 {
                c = mix(c, rgb(0.6, 0.45, 0.33), ((flats - 0.62) * 6.0).min(0.8));
            }
            ids.push(m.vertex_uv(Vec3::new(x, ly, h), normal, lin(c), Vec2::new(x, wy) / 150.0));
        }
    }
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let a = ids[j * nx + i];
            let b = ids[j * nx + i + 1];
            let c = ids[(j + 1) * nx + i + 1];
            let d = ids[(j + 1) * nx + i];
            m.triangle(a, b, c);
            m.triangle(a, c, d);
        }
    }
    // Scattered boulders and rocky outcrops.
    let clusters = rng.int(1, 3);
    for _ in 0..clusters {
        let c = Vec2::new(rng.range(-CHUNK_W / 2.0 + 40.0, CHUNK_W / 2.0 - 40.0), rng.range(20.0, CHUNK_H - 20.0));
        for _ in 0..rng.int(3, 8) {
            let p = c + rng.in_disc(40.0);
            let h = dune_height(p.x, world_y0 + p.y);
            let r = rng.range(3.0, 9.0);
            boulder(&mut m, Vec3::new(p.x, p.y, h - r * 0.15), r, rng);
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Clouds
// ---------------------------------------------------------------------------

fn spawn_cloud(commands: &mut Commands, art: &Art, rng: &mut Rng, p: Vec2, low: bool) {
    let i = rng.int(0, art.mat_cloud.len() as i32 - 1) as usize;
    let (alt, w, speed, mat) = if low {
        (z::CLOUD_LOW, rng.range(260.0, 460.0), rng.range(95.0, 120.0), art.mat_cloud[i].clone())
    } else {
        (z::CLOUD_HIGH, rng.range(300.0, 560.0), rng.range(160.0, 210.0), art.mat_cloud_high[i].clone())
    };
    let size = Vec2::new(w, w * 0.69);
    let shadow_offset = SHADOW_SLANT * (alt - 20.0);
    commands.spawn((
        Mesh3d(art.quad.clone()),
        MeshMaterial3d(mat),
        Transform::from_translation(p.extend(alt + rng.f32())).with_scale(size.extend(1.0)),
        Velocity(Vec2::new(rng.range(-6.0, 6.0), -speed)),
        Offscreen::new(360.0),
        NotShadowCaster,
        NotShadowReceiver,
        BackdropItem,
        children![(
            Mesh3d(art.quad.clone()),
            MeshMaterial3d(art.mat_cloud_shadow[i].clone()),
            Transform::from_translation(Vec3::new(shadow_offset.x / size.x, shadow_offset.y / size.y, z::CLOUD_SHADOW - alt)),
            NotShadowCaster,
            NotShadowReceiver,
        )],
    ));
}
