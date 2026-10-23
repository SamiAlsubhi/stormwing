//! Enemy definitions, movement patterns and guns.

use crate::art::{Art, ModelSet};
use crate::audio::{PlaySfx, Sfx};
use crate::combat::spawn_enemy_shot;
use crate::common::*;
use crate::pickups::PowerKind;
use crate::player::Player;
use crate::rng::Rng;
use crate::state::{AppState, Run};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
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
    /// Direction the nose points (radians, 0 = up the screen).
    heading: f32,
    /// Bank angle, leaning into turns.
    roll: f32,
}

impl Mover {
    pub fn new(kind: Move, face: bool, heading: f32) -> Self {
        Self {
            kind,
            t: 0.0,
            face,
            vel: Vec2::NEG_Y * 100.0,
            heading,
            roll: 0.0,
        }
    }
}

/// Heading of an object from its rotation, ignoring any roll.
pub fn heading_of(rotation: Quat) -> f32 {
    (rotation * Vec3::Y).truncate().to_angle() - FRAC_PI_2
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
            .add_systems(Update, (steer_boats, move_enemies, boat_wakes).chain().in_set(GameSet::Move))
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
            let mut turn_rate = 0.0;
            if dir.length_squared() > 1e-4 {
                let want = dir.to_angle() - FRAC_PI_2;
                let diff = (want - m.heading + PI).rem_euclid(TAU) - PI;
                let step = diff * (10.0 * dt).min(1.0);
                m.heading += step;
                turn_rate = step / dt;
            }
            // Bank into turns (turning right = negative rate = right wing down).
            let want_roll = (-turn_rate * 0.35).clamp(-0.9, 0.9);
            m.roll += (want_roll - m.roll) * (6.0 * dt).min(1.0);
            t.rotation = Quat::from_rotation_z(m.heading) * Quat::from_rotation_y(m.roll);
        }
    }
}

/// Boats look ahead and veer around islands, icebergs and their shallows.
fn steer_boats(
    time: Res<Time>,
    islands: Query<(&Transform, &crate::terrain::Footprint), Without<Mover>>,
    mut boats: Query<(&Enemy, &Transform, &mut Mover)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    const CRUISE: Vec2 = Vec2::new(0.0, 22.0);
    for (enemy, t, mut mover) in &mut boats {
        if enemy.kind != EnemyKind::Boat {
            continue;
        }
        let Move::Ground(vel) = mover.kind else { continue };
        let pos = t.translation.truncate();
        let mut desired = CRUISE;
        for (it, fp) in &islands {
            let centre = it.translation.truncate();
            let rel = pos - centre;
            let dist = rel.length();
            let keep_out = fp.radius * 0.8 + 30.0;
            // Only land that's ahead of (or level with) the boat matters.
            if dist > keep_out + 120.0 || centre.y < pos.y - keep_out * 0.5 {
                continue;
            }
            let side = if rel.x >= 0.0 { 1.0 } else { -1.0 };
            let urgency = (1.0 - (dist - keep_out).max(0.0) / 120.0).clamp(0.0, 1.0);
            desired.x += side * 70.0 * urgency;
            if dist < keep_out {
                desired += rel.normalize_or(Vec2::X) * 50.0;
            }
        }
        let desired = desired.clamp_length_max(60.0);
        mover.kind = Move::Ground(vel.lerp(desired, (2.0 * dt).min(1.0)));
    }
}

/// Boats churn up a white wake behind them.
fn boat_wakes(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    mut timer: Local<f32>,
    boats: Query<(&Enemy, &Transform)>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.12;
    for (enemy, t) in &boats {
        if enemy.kind != EnemyKind::Boat {
            continue;
        }
        let back = t.rotation * Vec3::NEG_Y;
        let side = t.rotation * Vec3::X;
        let stern = t.translation + back * 44.0;
        for s in [-1.0f32, 1.0] {
            crate::effects::foam(
                &mut commands,
                &art,
                Vec3::new(stern.x, stern.y, 0.6),
                (side * s * 18.0 + back * 10.0).truncate(),
                1.6,
                (10.0, 30.0),
                0.45,
            );
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
        let parent_angle = heading_of(t.rotation);
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

/// Spawns one enemy of `kind`. Ground units stand at `ground_z`; aircraft
/// fly at their usual altitude. Returns the entity.
pub fn spawn_enemy(
    commands: &mut Commands,
    ctx: &mut SpawnCtx,
    kind: EnemyKind,
    pos: Vec2,
    ground_z: f32,
    mv: Move,
    drop: Option<PowerKind>,
) -> Entity {
    let art = ctx.art;
    let d = ctx.diff;
    struct Spec {
        model: ModelSet,
        hp: f32,
        score: u32,
        stars: u32,
        hitbox: f32,
        size: f32,
        air: bool,
        gun: Option<Gun>,
        barrel: Option<(ModelSet, Vec3)>,
        engines: Vec<(Vec2, Vec2)>,
        survivor: bool,
    }
    let spec = match kind {
        EnemyKind::Dart => Spec {
            model: art.dart.clone(),
            hp: 3.0,
            score: 100,
            stars: 1,
            hitbox: 20.0,
            size: 22.0,
            air: true,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 2.4, 250.0)),
            barrel: None,
            engines: vec![(Vec2::new(0.0, -26.0), Vec2::new(10.0, 20.0))],
            survivor: false,
        },
        EnemyKind::Swooper => Spec {
            model: art.swooper.clone(),
            hp: 4.0,
            score: 150,
            stars: 2,
            hitbox: 24.0,
            size: 24.0,
            air: true,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 1.9, 270.0)),
            barrel: None,
            engines: vec![(Vec2::new(0.0, -21.0), Vec2::new(9.0, 18.0))],
            survivor: false,
        },
        EnemyKind::Chaser => Spec {
            model: art.chaser.clone(),
            hp: 2.0,
            score: 120,
            stars: 1,
            hitbox: 17.0,
            size: 18.0,
            air: true,
            gun: None,
            barrel: None,
            engines: vec![],
            survivor: false,
        },
        EnemyKind::Gunship => Spec {
            model: art.gunship.clone(),
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
            survivor: false,
        },
        EnemyKind::Bomber => Spec {
            model: art.bomber.clone(),
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
            survivor: false,
        },
        EnemyKind::Turret => Spec {
            model: art.turret_base.clone(),
            hp: 12.0,
            score: 300,
            stars: 3,
            hitbox: 22.0,
            size: 26.0,
            air: false,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 1.7, 240.0).muzzle(28.0)),
            barrel: Some((art.turret_barrel.clone(), Vec3::ZERO)),
            engines: vec![],
            survivor: false,
        },
        EnemyKind::Tank => Spec {
            model: art.tank_body.clone(),
            hp: 14.0,
            score: 350,
            stars: 3,
            hitbox: 24.0,
            size: 26.0,
            air: false,
            gun: Some(Gun::new(Pattern::Aimed { count: 1, spread: 0.0 }, 2.1, 230.0).muzzle(28.0)),
            barrel: Some((art.tank_turret.clone(), Vec3::new(0.0, 2.0, 0.0))),
            engines: vec![],
            survivor: false,
        },
        EnemyKind::Boat => Spec {
            model: art.boat.clone(),
            hp: 26.0,
            score: 500,
            stars: 5,
            hitbox: 30.0,
            size: 34.0,
            air: false,
            gun: Some(
                Gun::new(Pattern::Aimed { count: 3, spread: 0.4 }, 2.2, 230.0).muzzle(26.0),
            ),
            barrel: Some((art.turret_barrel.clone(), Vec3::new(0.0, 26.0, 1.0))),
            engines: vec![],
            survivor: false,
        },
        EnemyKind::Hut => Spec {
            model: art.hut.clone(),
            hp: 10.0,
            score: 200,
            stars: 2,
            hitbox: 24.0,
            size: 26.0,
            air: false,
            gun: None,
            barrel: None,
            engines: vec![],
            survivor: true,
        },
        EnemyKind::Depot => Spec {
            model: art.depot.clone(),
            hp: 8.0,
            score: 250,
            stars: 4,
            hitbox: 30.0,
            size: 48.0,
            air: false,
            gun: None,
            barrel: None,
            engines: vec![],
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
    let z = if spec.air { z::ENEMY + ctx.rng.range(-6.0, 6.0) } else { ground_z };
    let mut gun = spec.gun;
    if let Some(g) = gun.as_mut() {
        g.timer = g.interval * ctx.rng.range(0.35, 1.0);
    }

    ctx.run.spawned += 1;
    if spec.survivor {
        ctx.run.survivors += 1;
    }

    let mut ec = commands.spawn((
        Mesh3d(spec.model.paint.clone()),
        MeshMaterial3d(art.mat_paint.clone()),
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
        Mover::new(mv, face, initial_rot),
        Offscreen::new(if spec.air { 260.0 } else { 120.0 }),
        DespawnOnExit(AppState::Playing),
    ));
    if let Some(g) = gun {
        ec.insert(g);
    }
    if !spec.air {
        ec.insert(Scrolls);
    }
    let entity = ec.id();
    let mut barrel = None;
    ec.with_children(|c| {
        if let Some((set, offset)) = &spec.barrel {
            barrel = Some(
                c.spawn((
                    Barrel,
                    Mesh3d(set.paint.clone()),
                    MeshMaterial3d(art.mat_paint.clone()),
                    Transform::from_translation(*offset),
                ))
                .id(),
            );
        }
        for (offset, size) in &spec.engines {
            c.spawn((
                Mesh3d(art.quad.clone()),
                MeshMaterial3d(art.mat_engine.clone()),
                Transform::from_translation(offset.extend(0.0)).with_scale(size.extend(1.0)),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    });
    attach_model_parts(commands, entity, &spec.model, art);
    if let (Some(b), Some((set, _))) = (barrel, &spec.barrel) {
        attach_model_parts(commands, b, set, art);
    }
    entity
}
