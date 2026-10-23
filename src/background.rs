//! Scrolling scenery: sea or desert floor, procedurally shaped islands, surf,
//! roads and drifting clouds. Each stage has its own colour theme.

use crate::art::Art;
use crate::common::*;
use crate::rng::Rng;
use crate::shapes::Shape;
use crate::state::{AppState, Run};
use bevy::prelude::*;
use std::f32::consts::TAU;

pub struct Theme {
    pub name: &'static str,
    pub base: Color,
    pub streak: Color,
    pub shallows: Color,
    pub edge: Color,
    pub land: Color,
    pub land_dark: Color,
    pub detail: Color,
    pub detail_light: Color,
    /// Desert stages get roads instead of surf.
    pub roads: bool,
}

pub const THEMES: [Theme; 3] = [
    Theme {
        name: "Coral Coast",
        base: Color::srgb(0.05, 0.32, 0.5),
        streak: Color::srgba(0.75, 0.92, 1.0, 0.16),
        shallows: Color::srgba(0.35, 0.8, 0.85, 0.45),
        edge: Color::srgb(0.93, 0.85, 0.6),
        land: Color::srgb(0.33, 0.62, 0.3),
        land_dark: Color::srgb(0.24, 0.49, 0.24),
        detail: Color::srgb(0.12, 0.36, 0.16),
        detail_light: Color::srgb(0.22, 0.5, 0.24),
        roads: false,
    },
    Theme {
        name: "Ember Dunes",
        base: Color::srgb(0.76, 0.58, 0.37),
        streak: Color::srgba(0.92, 0.76, 0.54, 0.35),
        shallows: Color::srgba(0.6, 0.42, 0.25, 0.35),
        edge: Color::srgb(0.56, 0.37, 0.23),
        land: Color::srgb(0.68, 0.46, 0.29),
        land_dark: Color::srgb(0.6, 0.39, 0.24),
        detail: Color::srgb(0.42, 0.27, 0.17),
        detail_light: Color::srgb(0.5, 0.33, 0.2),
        roads: true,
    },
    Theme {
        name: "Frostbite Reach",
        base: Color::srgb(0.09, 0.17, 0.27),
        streak: Color::srgba(0.8, 0.9, 1.0, 0.14),
        shallows: Color::srgba(0.45, 0.65, 0.8, 0.4),
        edge: Color::srgb(0.72, 0.85, 0.94),
        land: Color::srgb(0.92, 0.96, 1.0),
        land_dark: Color::srgb(0.78, 0.86, 0.94),
        detail: Color::srgb(0.18, 0.33, 0.33),
        detail_light: Color::srgb(0.3, 0.46, 0.45),
        roads: false,
    },
];

#[derive(Resource)]
pub struct Backdrop {
    pub theme: usize,
    island_timer: f32,
    streak_timer: f32,
    cloud_timer: f32,
    road_timer: f32,
    rng: Rng,
}

/// Everything that belongs to the scrolling scenery.
#[derive(Component)]
pub struct BackdropItem;

pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Backdrop {
            theme: 0,
            island_timer: 0.0,
            streak_timer: 0.0,
            cloud_timer: 0.0,
            road_timer: 0.0,
            rng: Rng::new(77),
        })
        .add_systems(Startup, |mut c: Commands, mut m: ResMut<Assets<Mesh>>, art: Res<Art>, mut b: ResMut<Backdrop>, q: Query<Entity, With<BackdropItem>>| {
            reset(&mut c, &mut m, &art, &mut b, &q, 0);
        })
        .add_systems(
            OnEnter(AppState::Playing),
            |mut c: Commands, mut m: ResMut<Assets<Mesh>>, art: Res<Art>, mut b: ResMut<Backdrop>, q: Query<Entity, With<BackdropItem>>, run: Res<Run>| {
                reset(&mut c, &mut m, &art, &mut b, &q, run.stage);
            },
        )
        .add_systems(
            OnExit(AppState::Playing),
            |mut c: Commands, mut m: ResMut<Assets<Mesh>>, art: Res<Art>, mut b: ResMut<Backdrop>, q: Query<Entity, With<BackdropItem>>, run: Res<Run>| {
                reset(&mut c, &mut m, &art, &mut b, &q, run.stage);
            },
        )
        .add_systems(Update, spawn_scenery);
    }
}

fn reset(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    art: &Art,
    backdrop: &mut Backdrop,
    existing: &Query<Entity, With<BackdropItem>>,
    theme: usize,
) {
    for e in existing {
        commands.entity(e).try_despawn();
    }
    backdrop.theme = theme.min(THEMES.len() - 1);
    let theme = &THEMES[backdrop.theme];
    commands.insert_resource(ClearColor(theme.base));

    // Pre-populate the screen so a stage never starts on an empty sea.
    let mut y = -HALF_H - 100.0;
    while y < HALF_H + 200.0 {
        let x = backdrop.rng.range(-HALF_W, HALF_W);
        let r = backdrop.rng.range(50.0, 130.0);
        spawn_island(commands, meshes, art, theme, &mut backdrop.rng, Vec2::new(x, y), r);
        y += backdrop.rng.range(260.0, 420.0);
    }
    for _ in 0..40 {
        let p = Vec2::new(
            backdrop.rng.range(-HALF_W - 50.0, HALF_W + 50.0),
            backdrop.rng.range(-HALF_H, HALF_H + 100.0),
        );
        spawn_streak(commands, art, theme, &mut backdrop.rng, p);
    }
    for i in 0..3 {
        let p = Vec2::new(backdrop.rng.range(-HALF_W, HALF_W), -300.0 + i as f32 * 400.0);
        spawn_cloud(commands, art, &mut backdrop.rng, p);
    }
    if theme.roads {
        spawn_road(commands, meshes, art, theme, backdrop.rng.range(-200.0, 200.0), 0.0, 1300.0);
    }
}

fn spawn_scenery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    art: Res<Art>,
    time: Res<Time>,
    scroll: Res<ScrollSpeed>,
    mut b: ResMut<Backdrop>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let theme = &THEMES[b.theme];
    let speed_k = scroll.0 / 70.0;

    b.island_timer -= dt * speed_k;
    if b.island_timer <= 0.0 {
        b.island_timer = b.rng.range(3.5, 6.5);
        let r = b.rng.range(45.0, 140.0);
        let x = b.rng.range(-HALF_W - 40.0, HALF_W + 40.0);
        spawn_island(&mut commands, &mut meshes, &art, theme, &mut b.rng, Vec2::new(x, HALF_H + r + 60.0), r);
    }

    b.streak_timer -= dt * speed_k;
    while b.streak_timer <= 0.0 {
        b.streak_timer += b.rng.range(0.08, 0.2);
        let p = Vec2::new(b.rng.range(-HALF_W - 50.0, HALF_W + 50.0), HALF_H + 40.0);
        spawn_streak(&mut commands, &art, theme, &mut b.rng, p);
    }

    b.cloud_timer -= dt;
    if b.cloud_timer <= 0.0 {
        b.cloud_timer = b.rng.range(2.5, 5.5);
        let p = Vec2::new(b.rng.range(-HALF_W, HALF_W), HALF_H + 250.0);
        spawn_cloud(&mut commands, &art, &mut b.rng, p);
    }

    if theme.roads {
        b.road_timer -= dt * speed_k;
        if b.road_timer <= 0.0 {
            b.road_timer = b.rng.range(14.0, 22.0);
            let x = b.rng.range(-250.0, 250.0);
            spawn_road(&mut commands, &mut meshes, &art, theme, x, HALF_H + 700.0, 1300.0);
        }
    }
}

fn spawn_streak(commands: &mut Commands, art: &Art, theme: &Theme, rng: &mut Rng, p: Vec2) {
    let len = rng.range(30.0, 90.0);
    let mut color = theme.streak;
    color.set_alpha(theme.streak.alpha() * rng.range(0.5, 1.0));
    commands.spawn((
        Sprite {
            image: art.tex_streak.clone(),
            custom_size: Some(Vec2::new(5.0, len)),
            color,
            ..default()
        },
        Transform::from_translation(p.extend(z::WATER_FX))
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2 + rng.range(-0.15, 0.15))),
        Scrolls,
        Offscreen::new(120.0),
        BackdropItem,
    ));
}

fn spawn_cloud(commands: &mut Commands, art: &Art, rng: &mut Rng, p: Vec2) {
    let tex = art.tex_clouds[rng.int(0, art.tex_clouds.len() as i32 - 1) as usize].clone();
    let w = rng.range(280.0, 520.0);
    commands.spawn((
        Sprite {
            image: tex,
            custom_size: Some(Vec2::new(w, w * 0.66)),
            color: Color::srgba(1.0, 1.0, 1.0, rng.range(0.3, 0.45)),
            flip_x: rng.chance(0.5),
            ..default()
        },
        Transform::from_translation(p.extend(z::CLOUD + rng.f32())),
        Velocity(Vec2::new(rng.range(-8.0, 8.0), -rng.range(150.0, 210.0))),
        Offscreen::new(320.0),
        BackdropItem,
    ));
}

fn spawn_road(commands: &mut Commands, meshes: &mut Assets<Mesh>, art: &Art, theme: &Theme, x: f32, y: f32, len: f32) {
    let mut s = Shape::new();
    s.rect(Vec2::ZERO, Vec2::new(64.0, len), theme.edge.with_alpha(0.6));
    s.rect(Vec2::ZERO, Vec2::new(48.0, len), theme.detail);
    let mut yy = -len / 2.0;
    while yy < len / 2.0 {
        s.rect(Vec2::new(0.0, yy), Vec2::new(4.0, 26.0), Color::srgba(0.95, 0.85, 0.5, 0.7));
        yy += 60.0;
    }
    commands.spawn((
        Mesh2d(meshes.add(s.build())),
        MeshMaterial2d(art.mat_base.clone()),
        Transform::from_xyz(x, y, z::WATER_FX + 0.5),
        Scrolls,
        Offscreen::new(len / 2.0 + 50.0),
        BackdropItem,
    ));
}

/// Builds an organic island outline: shallows, beach, grass, hills and trees.
pub fn island_shape(theme: &Theme, rng: &mut Rng, radius: f32) -> Shape {
    let n = 36;
    let phases: [f32; 4] = [rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU)];
    let stretch = Vec2::new(rng.range(0.8, 1.3), rng.range(0.8, 1.2));
    let outline = |scale: f32, wobble: f32| -> Vec<Vec2> {
        (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * TAU;
                let r = 1.0
                    + 0.16 * (3.0 * a + phases[0]).sin()
                    + 0.09 * (5.0 * a + phases[1]).sin() * wobble
                    + 0.05 * (9.0 * a + phases[2]).sin() * wobble;
                Vec2::from_angle(a) * r * radius * scale * stretch
            })
            .collect()
    };

    let mut s = Shape::new();
    if !theme.roads {
        s.poly(&outline(1.22, 0.6), theme.shallows);
    }
    s.poly(&outline(1.0, 1.0), theme.edge);
    s.poly(&outline(0.84, 1.3), theme.land);
    let hill_c = Vec2::new(rng.range(-0.2, 0.2), rng.range(-0.2, 0.2)) * radius;
    let hill: Vec<Vec2> = outline(0.45, 1.6).into_iter().map(|p| p + hill_c).collect();
    s.poly(&hill, theme.land_dark);

    let trees = (radius / 7.0) as usize;
    for _ in 0..trees {
        let p = Vec2::from_angle(rng.range(0.0, TAU)) * rng.f32().sqrt() * radius * 0.62 * stretch;
        let r = rng.range(5.0, 11.0);
        if theme.roads {
            // Desert: rocky outcrops.
            s.regular(p + Vec2::new(2.0, -2.0), r, 5, rng.range(0.0, TAU), theme.detail.with_alpha(0.5));
            s.regular(p, r, 5, rng.range(0.0, TAU), theme.detail_light);
        } else {
            s.circle(p + Vec2::new(2.5, -2.5), r, Color::srgba(0.0, 0.0, 0.0, 0.18));
            s.circle(p, r, theme.detail);
            s.circle(p + Vec2::new(-r * 0.25, r * 0.25), r * 0.55, theme.detail_light);
        }
    }
    s
}

pub fn spawn_island(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    art: &Art,
    theme: &Theme,
    rng: &mut Rng,
    pos: Vec2,
    radius: f32,
) -> Entity {
    let shape = island_shape(theme, rng, radius);
    commands
        .spawn((
            Mesh2d(meshes.add(shape.build())),
            MeshMaterial2d(art.mat_base.clone()),
            Transform::from_translation(pos.extend(z::ISLAND + rng.f32() * 0.5)),
            Scrolls,
            Offscreen::new(radius * 1.4 + 40.0),
            BackdropItem,
        ))
        .id()
}
