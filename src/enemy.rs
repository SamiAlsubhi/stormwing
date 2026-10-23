//! Enemy definitions, movement patterns and guns.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::combat::spawn_enemy_shot;
use crate::common::*;
use crate::pickups::PowerKind;
use crate::player::Player;
use crate::rng::Rng;
use crate::state::{AppState, Run};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Dart,
    Swooper,
    Chaser,
    Gunship,
    Bomber,
    Turret,
    Tank,
    Boat,
    Hut,
    Depot,
    Boss,
    BossTurret,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub score: u32,
    pub stars: u32,
    pub drop: Option<PowerKind>,
    pub air: bool,
    /// Counts towards the stage's "destroy X%" medals.
    pub counted: bool,
    /// Seeker missiles may lock on.
    pub targetable: bool,
    /// Rough radius used to size the explosion.
    pub size: f32,
    /// Collides with (and damages) the player.
    pub contact: bool,
    /// Releases a survivor to rescue when destroyed.
    pub survivor: bool,
}

/// Difficulty multipliers for the current stage.
#[derive(Resource, Clone, Copy)]
pub struct Difficulty {
    pub hp: f32,
    pub bullet_speed: f32,
    pub fire_rate: f32,
}

impl Difficulty {
    pub fn for_stage(stage: usize) -> Self {
        match stage {
            0 => Self { hp: 1.0, bullet_speed: 1.0, fire_rate: 1.0 },
            1 => Self { hp: 1.4, bullet_speed: 1.12, fire_rate: 1.18 },
            _ => Self { hp: 1.85, bullet_speed: 1.22, fire_rate: 1.32 },
        }
    }
}

#[derive(Clone, Copy)]
pub enum Move {
    Linear(Vec2),
    Sine { x0: f32, amp: f32, freq: f32, vy: f32, phase: f32 },
    /// Follows a cubic Bézier, then keeps flying in the final direction.
    Curve { p: [Vec2; 4], dur: f32, exit_speed: f32 },
    /// Flies to a point, loiters while firing, then leaves.
    Hover { target: Vec2, hold: f32, leave: Vec2 },
    /// Homes in on the player until it overshoots.
    Chase { speed: f32, turn: f32 },
    /// Moves along the ground (on top of the ground scroll).
    Ground(Vec2),
}

#[derive(Component)]
pub struct Mover {
    pub kind: Move,
    pub t: f32,
    pub face: bool,
    vel: Vec2,
}

impl Mover {
    pub fn new(kind: Move, face: bool) -> Self {
        Self { kind, t: 0.0, face, vel: Vec2::NEG_Y * 100.0 }
    }
}

#[derive(Clone, Copy)]
pub enum Pattern {
    Aimed { count: u32, spread: f32 },
    Down { count: u32, spread: f32 },
    Ring { count: u32 },
}

#[derive(Component)]
pub struct Gun {
    pub pattern: Pattern,
    pub interval: f32,
    pub timer: f32,
    pub speed: f32,
    /// Distance from the centre to the muzzle.
    pub muzzle: f32,
    pub big: bool,
    /// Shots per volley, fired `burst_gap` seconds apart.
    pub burst: u32,
    pub burst_gap: f32,
    burst_left: u32,
    burst_timer: f32,
    spin: f32,
}

impl Gun {
    pub fn new(pattern: Pattern, interval: f32, speed: f32) -> Self {
        Self {
            pattern,
            interval,
            timer: interval,
            speed,
            muzzle: 20.0,
            big: false,
            burst: 1,
            burst_gap: 0.12,
            burst_left: 0,
            burst_timer: 0.0,
            spin: 0.0,
        }
    }
    pub fn muzzle(mut self, m: f32) -> Self {
        self.muzzle = m;
        self
    }
    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }
    pub fn burst(mut self, n: u32, gap: f32) -> Self {
        self.burst = n;
        self.burst_gap = gap;
        self
    }
}

/// A child sprite/mesh that rotates to aim at the player (turret barrels).
#[derive(Component)]
pub struct Barrel;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Difficulty::for_stage(0))
            .add_systems(OnEnter(AppState::Playing), |run: Res<Run>, mut c: Commands| {
                c.insert_resource(Difficulty::for_stage(run.stage));
            })
            .add_systems(Update, move_enemies.in_set(GameSet::Move))
            .add_systems(Update, (aim_barrels, fire_guns).chain().in_set(GameSet::Combat));
    }
}

fn bezier(p: &[Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    p[0] * u * u * u + p[1] * 3.0 * u * u * t + p[2] * 3.0 * u * t * t + p[3] * t * t * t
}

fn move_enemies(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Mover>)>,
    mut q: Query<(&mut Mover, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let target = player.single().map(|t| t.translation.truncate()).ok();
    for (mut m, mut t) in &mut q {
        m.t += dt;
        let time = m.t;
        let pos = t.translation.truncate();
        let mut new = pos;
        let mut facing: Option<Vec2> = None;
        match m.kind {
            Move::Linear(v) => new = pos + v * dt,
            Move::Sine { x0, amp, freq, vy, phase } => {
                new.x = x0 + amp * (freq * time + phase).sin();
                new.y = pos.y + vy * dt;
            }
            Move::Curve { p, dur, exit_speed } => {
                if time < dur {
                    new = bezier(&p, time / dur);
                } else {
                    let dir = (p[3] - p[2]).normalize_or(Vec2::NEG_Y);
                    new = pos + dir * exit_speed * dt;
                }
            }
            Move::Hover { target: goal, hold, leave } => {
                let approach = 2.2;
                if time < approach {
                    new = pos.lerp(goal, 1.0 - (-2.8 * dt).exp());
                } else if time < approach + hold {
                    let sway = (time - approach) * 0.9;
                    let wanted = goal + Vec2::new(sway.sin() * 40.0, (sway * 1.7).sin() * 12.0);
                    new = pos.lerp(wanted, 1.0 - (-3.0 * dt).exp());
                } else {
                    let k = ((time - approach - hold) * 0.8).min(1.0);
                    new = pos + leave * k * dt;
                }
                facing = Some(Vec2::NEG_Y);
            }
            Move::Chase { speed, turn } => {
                let mut vel = m.vel;
                if let Some(tp) = target {
                    // Stop homing once we've dived past the player.
                    if pos.y > tp.y + 60.0 {
                        let want = (tp - pos).normalize_or(Vec2::NEG_Y);
                        let cur = vel.normalize_or(Vec2::NEG_Y);
                        vel = cur.lerp(want, (turn * dt).min(1.0)).normalize_or(want) * vel.length();
                    }
                }
                let s = (vel.length() + speed * 0.8 * dt).min(speed);
                vel = vel.normalize_or(Vec2::NEG_Y) * s;
                m.vel = vel;
                new = pos + vel * dt;
            }
            Move::Ground(v) => {
                new = pos + v * dt;
                if v.length_squared() > 1.0 {
                    facing = Some(v);
                }
            }
        }
        t.translation.x = new.x;
        t.translation.y = new.y;
        if m.face {
            let dir = facing.unwrap_or(new - pos);
            if dir.length_squared() > 1e-4 {
                let want = dir.to_angle() - FRAC_PI_2;
                let (_, _, current) = t.rotation.to_euler(EulerRot::XYZ);
                let diff = (want - current + PI).rem_euclid(TAU) - PI;
                let turn = (10.0 * dt).min(1.0);
                t.rotation = Quat::from_rotation_z(current + diff * turn);
            }
        }
    }
}

fn aim_barrels(
    player: Query<&Transform, With<Player>>,
    parents: Query<(&Transform, &Children), (With<Enemy>, Without<Barrel>)>,
    mut barrels: Query<&mut Transform, (With<Barrel>, Without<Enemy>, Without<Player>)>,
) {
    let Ok(pt) = player.single() else { return };
    let target = pt.translation.truncate();
    for (t, children) in &parents {
        let pos = t.translation.truncate();
        let world_angle = (target - pos).to_angle() - FRAC_PI_2;
        let (_, _, parent_angle) = t.rotation.to_euler(EulerRot::XYZ);
        for child in children.iter() {
            if let Ok(mut bt) = barrels.get_mut(child) {
                bt.rotation = Quat::from_rotation_z(world_angle - parent_angle);
            }
        }
    }
}

fn fire_guns(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    diff: Res<Difficulty>,
    run: Res<Run>,
    mut sfx: MessageWriter<PlaySfx>,
    player: Query<&Transform, With<Player>>,
    mut guns: Query<(&mut Gun, &Transform)>,
) {
    if run.outcome.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let target = player.single().map(|t| t.translation.truncate()).ok();
    let mut fired = false;
    for (mut gun, t) in &mut guns {
        let pos = t.translation.truncate();
        let on_screen = pos.y < HALF_H - 20.0 && pos.y > -HALF_H + 80.0 && pos.x.abs() < HALF_W;
        if !on_screen {
            continue;
        }
        gun.spin += dt;
        if gun.burst_left == 0 {
            gun.timer -= dt * diff.fire_rate;
            if gun.timer > 0.0 {
                continue;
            }
            gun.timer += gun.interval;
            gun.burst_left = gun.burst;
            gun.burst_timer = 0.0;
        }
        gun.burst_timer -= dt;
        if gun.burst_timer > 0.0 {
            continue;
        }
        gun.burst_timer = gun.burst_gap;
        gun.burst_left -= 1;

        let aim = target
            .map(|tp| (tp - pos).normalize_or(Vec2::NEG_Y))
            .unwrap_or(Vec2::NEG_Y);
        // Don't shoot from right on top of the player; it feels unfair.
        if let Some(tp) = target {
            if tp.distance(pos) < 90.0 {
                continue;
            }
        }
        let speed = gun.speed * diff.bullet_speed;
        let dirs: Vec<Vec2> = match gun.pattern {
            Pattern::Aimed { count, spread } => fan(aim, count, spread),
            Pattern::Down { count, spread } => fan(Vec2::NEG_Y, count, spread),
            Pattern::Ring { count } => (0..count)
                .map(|i| Vec2::from_angle(gun.spin * 0.7 + i as f32 / count as f32 * TAU))
                .collect(),
        };
        let muzzle_dir = match gun.pattern {
            Pattern::Ring { .. } => Vec2::ZERO,
            Pattern::Down { .. } => Vec2::NEG_Y,
            _ => aim,
        };
        let origin = pos + muzzle_dir * gun.muzzle;
        for d in dirs {
            spawn_enemy_shot(&mut commands, &art, origin, d * speed, gun.big);
        }
        fired = true;
    }
    if fired {
        sfx.write(PlaySfx::new(Sfx::EnemyShoot));
    }
}

/// `count` directions spread evenly across `spread` radians around `dir`.
pub fn fan(dir: Vec2, count: u32, spread: f32) -> Vec<Vec2> {
    if count <= 1 {
        return vec![dir];
    }
    let base = dir.to_angle();
    (0..count)
        .map(|i| {
            let k = i as f32 / (count - 1) as f32 - 0.5;
            Vec2::from_angle(base + k * spread)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

pub struct SpawnCtx<'a> {
    pub art: &'a Art,
    pub diff: Difficulty,
    pub run: &'a mut Run,
    pub rng: &'a mut Rng,
}

/// Spawns one enemy of `kind`. Returns its entity.
pub fn spawn_enemy(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    kind: EnemyKind,
    pos: Vec2,
    mv: Move,
    drop: Option<PowerKind>,
) -> Entity {
    let art = ctx.art;
    let d = ctx.diff;
    struct Spec {
        mesh: Handle<Mesh>,
        hp: f32,
        score: u32,
        stars: u32,
        hitbox: f32,
        size: f32,
        air: bool,
        gun: Option<Gun>,
        barrel: Option<(Handle<Mesh>, f32)>,
        engines: Vec<(Vec2, Vec2)>,
        shadow: f32,
        survivor: bool,
    }
    let spec = match kind {
        EnemyKind::Dart => Spec {
            mesh: art.dart.clone(),
            hp: 3.0,
            score: 100,
            stars: 1,
            hitbox: 20.0,
            size: 22.0,
            air: true,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 2.4, 250.0)),
            barrel: None,
            engines: vec![(Vec2::new(0.0, -26.0), Vec2::new(10.0, 20.0))],
            shadow: 0.7,
            survivor: false,
        },
        EnemyKind::Swooper => Spec {
            mesh: art.swooper.clone(),
            hp: 4.0,
            score: 150,
            stars: 2,
            hitbox: 24.0,
            size: 24.0,
            air: true,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 1.9, 270.0)),
            barrel: None,
            engines: vec![(Vec2::new(0.0, -21.0), Vec2::new(9.0, 18.0))],
            shadow: 0.7,
            survivor: false,
        },
        EnemyKind::Chaser => Spec {
            mesh: art.chaser.clone(),
            hp: 2.0,
            score: 120,
            stars: 1,
            hitbox: 17.0,
            size: 18.0,
            air: true,
            gun: None,
            barrel: None,
            engines: vec![],
            shadow: 0.7,
            survivor: false,
        },
        EnemyKind::Gunship => Spec {
            mesh: art.gunship.clone(),
            hp: 34.0,
            score: 800,
            stars: 8,
            hitbox: 44.0,
            size: 45.0,
            air: true,
            gun: Some(
                Gun::new(Pattern::Aimed { count: 5, spread: 0.7 }, 1.9, 230.0)
                    .muzzle(30.0)
                    .burst(2, 0.25),
            ),
            barrel: None,
            engines: vec![
                (Vec2::new(30.0, -26.0), Vec2::new(12.0, 22.0)),
                (Vec2::new(-30.0, -26.0), Vec2::new(12.0, 22.0)),
            ],
            shadow: 0.6,
            survivor: false,
        },
        EnemyKind::Bomber => Spec {
            mesh: art.bomber.clone(),
            hp: 80.0,
            score: 1500,
            stars: 16,
            hitbox: 58.0,
            size: 70.0,
            air: true,
            gun: Some(Gun::new(Pattern::Ring { count: 14 }, 2.4, 190.0).big()),
            barrel: None,
            engines: vec![
                (Vec2::new(32.0, -14.0), Vec2::new(12.0, 22.0)),
                (Vec2::new(-32.0, -14.0), Vec2::new(12.0, 22.0)),
                (Vec2::new(63.0, -17.0), Vec2::new(10.0, 18.0)),
                (Vec2::new(-63.0, -17.0), Vec2::new(10.0, 18.0)),
            ],
            shadow: 0.55,
            survivor: false,
        },
        EnemyKind::Turret => Spec {
            mesh: art.turret_base.clone(),
            hp: 12.0,
            score: 300,
            stars: 3,
            hitbox: 22.0,
            size: 26.0,
            air: false,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 1.7, 240.0).muzzle(28.0)),
            barrel: Some((art.turret_barrel.clone(), 0.2)),
            engines: vec![],
            shadow: 0.0,
            survivor: false,
        },
        EnemyKind::Tank => Spec {
            mesh: art.tank_body.clone(),
            hp: 14.0,
            score: 350,
            stars: 3,
            hitbox: 24.0,
            size: 26.0,
            air: false,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 2.1, 230.0).muzzle(28.0)),
            barrel: Some((art.tank_turret.clone(), 0.2)),
            engines: vec![],
            shadow: 0.0,
            survivor: false,
        },
        EnemyKind::Boat => Spec {
            mesh: art.boat.clone(),
            hp: 26.0,
            score: 500,
            stars: 5,
            hitbox: 30.0,
            size: 34.0,
            air: false,
            gun: Some(
                Gun::new(Pattern::Aimed { count: 3, spread: 0.4 }, 2.2, 230.0).muzzle(26.0),
            ),
            barrel: Some((art.turret_barrel.clone(), 0.2)),
            engines: vec![],
            shadow: 0.0,
            survivor: false,
        },
        EnemyKind::Hut => Spec {
            mesh: art.hut.clone(),
            hp: 10.0,
            score: 200,
            stars: 2,
            hitbox: 24.0,
            size: 26.0,
            air: false,
            gun: None,
            barrel: None,
            engines: vec![],
            shadow: 0.0,
            survivor: true,
        },
        EnemyKind::Depot => Spec {
            mesh: art.depot.clone(),
            hp: 8.0,
            score: 250,
            stars: 4,
            hitbox: 30.0,
            size: 48.0,
            air: false,
            gun: None,
            barrel: None,
            engines: vec![],
            shadow: 0.0,
            survivor: false,
        },
        EnemyKind::Boss | EnemyKind::BossTurret => unreachable!("bosses are spawned by the boss module"),
    };

    let face = spec.air || matches!(mv, Move::Ground(_));
    let initial_rot = match mv {
        Move::Linear(v) | Move::Ground(v) if v.length_squared() > 0.0 => v.to_angle() - FRAC_PI_2,
        Move::Curve { p, .. } => (p[1] - p[0]).to_angle() - FRAC_PI_2,
        Move::Ground(_) => 0.0,
        _ => PI,
    };
    let z = if spec.air { z::ENEMY + ctx.rng.f32() } else { z::GROUND_UNIT + ctx.rng.f32() * 0.5 };
    let mut gun = spec.gun;
    if let Some(g) = gun.as_mut() {
        g.timer = g.interval * ctx.rng.range(0.35, 1.0);
    }

    ctx.run.spawned += 1;
    if spec.survivor {
        ctx.run.survivors += 1;
    }

    let mut ec = commands.spawn((
        Mesh2d(spec.mesh.clone()),
        MeshMaterial2d(art.mat_base.clone()),
        Transform::from_translation(pos.extend(z)).with_rotation(Quat::from_rotation_z(initial_rot)),
        Enemy {
            kind,
            score: spec.score,
            stars: spec.stars,
            drop,
            air: spec.air,
            counted: true,
            targetable: true,
            size: spec.size,
            contact: spec.air,
            survivor: spec.survivor,
        },
        Health::new(spec.hp * d.hp),
        Hitbox(spec.hitbox),
        HitFlash::default(),
        Mover::new(mv, face),
        Offscreen::new(if spec.air { 260.0 } else { 120.0 }),
        DespawnOnExit(AppState::Playing),
    ));
    if let Some(g) = gun {
        ec.insert(g);
    }
    if !spec.air {
        ec.insert(Scrolls);
    }
    let shadow_mesh = spec.mesh.clone();
    ec.with_children(|c| {
        if spec.air {
            c.spawn((
                Mesh2d(shadow_mesh),
                MeshMaterial2d(art.mat_shadow.clone()),
                Transform::from_scale(Vec3::splat(spec.shadow)),
                Shadow {
                    offset: Vec2::new(26.0, -40.0) * (1.6 - spec.shadow),
                },
            ));
        }
        if let Some((mesh, dz)) = &spec.barrel {
            c.spawn((
                Barrel,
                Mesh2d(mesh.clone()),
                MeshMaterial2d(art.mat_base.clone()),
                Transform::from_xyz(0.0, 0.0, *dz),
            ));
        }
        for (offset, size) in &spec.engines {
            c.spawn((
                Sprite {
                    image: art.tex_glow.clone(),
                    custom_size: Some(*size),
                    color: Color::linear_rgb(3.2, 1.4, 0.45),
                    ..default()
                },
                Transform::from_translation(offset.extend(-0.05)),
            ));
        }
    });
    ec.id()
}
