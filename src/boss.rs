//! Multi-part bosses: destroy the turrets to drop the core's shield, then
//! take down the core while it cycles through bullet patterns.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::combat::{spawn_enemy_shot, EnemyShot, Shielded};
use crate::common::*;
use crate::effects;
use crate::enemy::{fan, spawn_enemy, Barrel, Difficulty, Enemy, EnemyKind, Gun, Move, Pattern, SpawnCtx};
use crate::pickups;
use crate::player::Player;
use crate::rng::Rng;
use crate::state::{AppState, Outcome, Run};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

pub struct BossDef {
    pub name: &'static str,
    hp: f32,
    radius: f32,
    /// Turret mount points relative to the boss centre (screen orientation).
    parts: &'static [(f32, f32)],
    part_hp: f32,
    /// Rotation speed of the whole boss (the citadel slowly turns).
    spin: f32,
    /// Horizontal sway amplitude while fighting.
    sway: f32,
    arms: u32,
}

pub const BOSSES: [BossDef; 3] = [
    BossDef {
        name: "TIDEBREAKER",
        hp: 380.0,
        radius: 70.0,
        parts: &[(-150.0, 18.0), (150.0, 18.0), (-62.0, -40.0), (62.0, -40.0)],
        part_hp: 42.0,
        spin: 0.0,
        sway: 140.0,
        arms: 3,
    },
    BossDef {
        name: "DUNE COLOSSUS",
        hp: 520.0,
        radius: 80.0,
        parts: &[(-72.0, 95.0), (72.0, 95.0), (-72.0, -70.0), (72.0, -70.0), (0.0, -110.0)],
        part_hp: 50.0,
        spin: 0.0,
        sway: 80.0,
        arms: 4,
    },
    BossDef {
        name: "AURORA CITADEL",
        hp: 640.0,
        radius: 90.0,
        parts: &[
            (0.0, 120.0),
            (104.0, 60.0),
            (104.0, -60.0),
            (0.0, -120.0),
            (-104.0, -60.0),
            (-104.0, 60.0),
        ],
        part_hp: 46.0,
        spin: 0.22,
        sway: 100.0,
        arms: 5,
    },
];

#[derive(Component)]
pub struct Boss {
    pub def: usize,
    t: f32,
    attack: usize,
    attack_t: f32,
    fire_t: f32,
    spiral: f32,
    summon_t: f32,
    exposed: bool,
}

#[derive(Component)]
pub struct BossPart {
    pub boss: Entity,
    pub offset: Vec2,
}

#[derive(Component)]
struct BossShieldFx;

#[derive(Component)]
struct BossDying {
    t: f32,
    boom_t: f32,
}

/// Boss info for the HUD.
#[derive(Resource, Default)]
pub struct BossStatus {
    pub active: bool,
    pub name: &'static str,
    pub hp_frac: f32,
    pub shielded: bool,
}

const ENTER_TIME: f32 = 4.0;
const DEATH_TIME: f32 = 2.8;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossStatus>()
            .add_systems(OnEnter(AppState::Playing), |mut s: ResMut<BossStatus>| *s = BossStatus::default())
            .add_systems(Update, (move_boss, boss_attacks).chain().in_set(GameSet::Move))
            .add_systems(Update, (expose_core, boss_defeat, boss_dying).chain().in_set(GameSet::Resolve));
    }
}

pub fn spawn_boss(commands: &mut Commands, ctx: &mut SpawnCtx, index: usize) {
    let def = &BOSSES[index];
    let art = ctx.art;
    let hp = def.hp * ctx.diff.hp;
    let start = Vec2::new(0.0, HALF_H + 300.0);
    let boss = commands
        .spawn((
            Mesh2d(art.bosses[index].clone()),
            MeshMaterial2d(art.mat_base.clone()),
            Transform::from_translation(start.extend(z::BOSS)).with_rotation(Quat::from_rotation_z(PI)),
            Enemy {
                kind: EnemyKind::Boss,
                score: 25_000 * (index as u32 + 1),
                stars: 60 + 20 * index as u32,
                drop: None,
                air: true,
                counted: false,
                targetable: false,
                size: 160.0,
                contact: false,
                survivor: false,
            },
            Health::new(hp),
            Hitbox(def.radius),
            HitFlash::default(),
            Shielded,
            Boss {
                def: index,
                t: 0.0,
                attack: 0,
                attack_t: 0.0,
                fire_t: 0.0,
                spiral: 0.0,
                summon_t: 6.0,
                exposed: false,
            },
            DespawnOnExit(AppState::Playing),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh2d(art.bosses[index].clone()),
                MeshMaterial2d(art.mat_shadow.clone()),
                Transform::from_scale(Vec3::splat(0.85)),
                Shadow {
                    offset: Vec2::new(50.0, -80.0),
                },
            ));
            c.spawn((
                BossShieldFx,
                Sprite {
                    image: art.tex_ring.clone(),
                    custom_size: Some(Vec2::splat(def.radius * 2.6)),
                    color: Color::linear_rgba(0.6, 1.6, 3.2, 0.8),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.5),
            ));
        })
        .id();

    for (i, &(x, y)) in def.parts.iter().enumerate() {
        let offset = Vec2::new(x, y);
        let pattern = match (index, i % 2) {
            (0, _) => Pattern::Aimed { count: 3, spread: 0.35 },
            (1, 0) => Pattern::Aimed { count: 1, spread: 0.0 },
            (1, _) => Pattern::Down { count: 5, spread: 1.2 },
            (_, 0) => Pattern::Aimed { count: 2, spread: 0.18 },
            (_, _) => Pattern::Ring { count: 8 },
        };
        let mut gun = Gun::new(pattern, 1.7 + i as f32 * 0.13, 235.0).muzzle(32.0);
        if index == 1 && i % 2 == 0 {
            gun = gun.burst(3, 0.14);
        }
        gun.timer = 1.0 + i as f32 * 0.35;
        commands.spawn((
            Mesh2d(art.boss_turret.clone()),
            MeshMaterial2d(art.mat_base.clone()),
            Transform::from_translation((start + offset).extend(z::BOSS + 1.0)),
            Enemy {
                kind: EnemyKind::BossTurret,
                score: 1500,
                stars: 8,
                drop: None,
                air: true,
                counted: false,
                targetable: true,
                size: 40.0,
                contact: false,
                survivor: false,
            },
            Health::new(def.part_hp * ctx.diff.hp),
            Hitbox(28.0),
            HitFlash::default(),
            gun,
            BossPart { boss, offset },
            DespawnOnExit(AppState::Playing),
            children![(
                Barrel,
                Mesh2d(art.boss_barrel.clone()),
                MeshMaterial2d(art.mat_base.clone()),
                Transform::from_xyz(0.0, 0.0, 0.2),
            )],
        ));
    }
}

fn move_boss(
    time: Res<Time>,
    mut bosses: Query<(Entity, &mut Boss, &mut Transform, Has<BossDying>), Without<BossPart>>,
    mut parts: Query<(&BossPart, &mut Transform), Without<Boss>>,
) {
    let dt = time.delta_secs();
    for (entity, mut boss, mut t, dying) in &mut bosses {
        let def = &BOSSES[boss.def];
        if !dying {
            boss.t += dt;
        }
        let bt = boss.t;
        let home = Vec2::new(0.0, HALF_H - 250.0);
        let pos = if bt < ENTER_TIME {
            let k = bt / ENTER_TIME;
            let ease = 1.0 - (1.0 - k).powi(3);
            Vec2::new(0.0, HALF_H + 300.0).lerp(home, ease)
        } else {
            let f = bt - ENTER_TIME;
            home + Vec2::new((f * 0.45).sin() * def.sway, (f * 0.8).sin() * 24.0)
        };
        let angle = def.spin * bt;
        if dying {
            // Shudder while breaking apart.
            t.translation.y -= 20.0 * dt;
        } else {
            t.translation.x = pos.x;
            t.translation.y = pos.y;
        }
        t.rotation = Quat::from_rotation_z(PI + angle);
        let centre = t.translation.truncate();
        for (part, mut pt) in &mut parts {
            if part.boss == entity {
                let p = centre + Vec2::from_angle(angle).rotate(part.offset);
                pt.translation.x = p.x;
                pt.translation.y = p.y;
            }
        }
    }
}

fn boss_attacks(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    diff: Res<Difficulty>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut sfx: MessageWriter<PlaySfx>,
    player: Query<&Transform, (With<Player>, Without<Boss>)>,
    mut bosses: Query<(&mut Boss, &Transform, &Health, Has<Shielded>), Without<BossDying>>,
) {
    if run.outcome.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let target = player.single().map(|t| t.translation.truncate()).ok();
    for (mut boss, t, health, shielded) in &mut bosses {
        if boss.t < ENTER_TIME {
            continue;
        }
        let def = &BOSSES[boss.def];
        let pos = t.translation.truncate();
        let aim = target.map(|tp| (tp - pos).normalize_or(Vec2::NEG_Y)).unwrap_or(Vec2::NEG_Y);
        let enraged = health.frac() < 0.5;
        let speed = 210.0 * diff.bullet_speed;
        let mut fired = false;
        boss.fire_t -= dt;
        boss.spiral += dt * if enraged { 2.6 } else { 1.9 };

        if shielded {
            // While turrets stand, the core lobs slow rings.
            if boss.fire_t <= 0.0 {
                boss.fire_t = 3.0;
                let n = 12 + 2 * boss.def as u32;
                let off = rng.range(0.0, TAU);
                for i in 0..n {
                    let d = Vec2::from_angle(off + i as f32 / n as f32 * TAU);
                    spawn_enemy_shot(&mut commands, &art, pos + d * 40.0, d * speed * 0.75, true);
                }
                fired = true;
            }
        } else {
            boss.attack_t -= dt;
            if boss.attack_t <= 0.0 {
                boss.attack = (boss.attack + 1) % 3;
                boss.attack_t = if enraged { 3.2 } else { 4.2 };
                boss.fire_t = 0.6;
            }
            if boss.fire_t <= 0.0 {
                match boss.attack {
                    // Rotating spiral arms.
                    0 => {
                        boss.fire_t = if enraged { 0.075 } else { 0.1 };
                        let arms = def.arms;
                        for i in 0..arms {
                            let a = boss.spiral + i as f32 / arms as f32 * TAU;
                            let d = Vec2::from_angle(a);
                            spawn_enemy_shot(&mut commands, &art, pos + d * 50.0, d * speed * 0.9, false);
                            if enraged {
                                let d2 = Vec2::from_angle(-a * 1.0 + 0.3);
                                spawn_enemy_shot(&mut commands, &art, pos + d2 * 50.0, d2 * speed * 0.7, false);
                            }
                        }
                    }
                    // Aimed fans.
                    1 => {
                        boss.fire_t = if enraged { 0.45 } else { 0.6 };
                        let n = if enraged { 9 } else { 7 };
                        for d in fan(aim, n, 1.1) {
                            spawn_enemy_shot(&mut commands, &art, pos + d * 50.0, d * speed * 1.15, false);
                        }
                    }
                    // Dense rings.
                    _ => {
                        boss.fire_t = if enraged { 0.6 } else { 0.85 };
                        let n = 20 + 4 * boss.def as u32;
                        let off = boss.spiral * 0.5;
                        for i in 0..n {
                            let d = Vec2::from_angle(off + i as f32 / n as f32 * TAU);
                            spawn_enemy_shot(&mut commands, &art, pos + d * 50.0, d * speed * 0.8, true);
                        }
                    }
                }
                fired = true;
            }

            // When enraged, the boss calls in escorts.
            if enraged {
                boss.summon_t -= dt;
                if boss.summon_t <= 0.0 {
                    boss.summon_t = 7.0;
                    let mut ctx = SpawnCtx {
                        art: &art,
                        diff: *diff,
                        run: &mut run,
                        rng: &mut rng,
                    };
                    for side in [-1.0f32, 1.0] {
                        let p0 = Vec2::new(side * (HALF_W + 60.0), HALF_H - 80.0);
                        let mv = Move::Curve {
                            p: [p0, Vec2::new(side * 150.0, 150.0), Vec2::new(-side * 80.0, -100.0), Vec2::new(-side * 250.0, -420.0)],
                            dur: 3.2,
                            exit_speed: 300.0,
                        };
                        let e = spawn_enemy(&mut commands, &mut ctx, EnemyKind::Dart, p0, mv, None);
                        commands.entity(e).insert(Escort);
                    }
                    // Escorts don't count towards the stage's kill medal.
                    ctx.run.spawned -= 2;
                }
            }
        }
        if fired {
            sfx.write(PlaySfx::new(Sfx::EnemyShoot).pitch(0.8));
        }
    }
}

#[derive(Component)]
struct Escort;

fn expose_core(
    mut commands: Commands,
    art: Res<Art>,
    mut sfx: MessageWriter<PlaySfx>,
    mut status: ResMut<BossStatus>,
    mut bosses: Query<(Entity, &mut Boss, &Health, &Transform, &Children, Has<Shielded>)>,
    parts: Query<&BossPart>,
    shield_fx: Query<(), With<BossShieldFx>>,
    mut escorts: Query<&mut Enemy, With<Escort>>,
) {
    for mut e in &mut escorts {
        e.counted = false;
    }
    let mut any = false;
    for (entity, mut boss, health, t, children, shielded) in &mut bosses {
        any = true;
        let def = &BOSSES[boss.def];
        status.active = true;
        status.name = def.name;
        status.hp_frac = health.frac();
        status.shielded = shielded;
        if !shielded || boss.t < ENTER_TIME {
            continue;
        }
        let remaining = parts.iter().filter(|p| p.boss == entity).count();
        if remaining == 0 && !boss.exposed {
            boss.exposed = true;
            boss.attack_t = 1.0;
            commands.entity(entity).remove::<Shielded>();
            for child in children.iter() {
                if shield_fx.get(child).is_ok() {
                    commands.entity(child).try_despawn();
                }
            }
            let pos = t.translation.truncate();
            effects::shockwave(&mut commands, &art, pos, 60.0, 500.0, 0.5, LinearRgba::rgb(0.8, 2.0, 3.5));
            effects::floating_text(&mut commands, pos + Vec2::new(0.0, -120.0), "CORE EXPOSED", Color::srgb(0.6, 0.9, 1.0), 30.0);
            sfx.write(PlaySfx::new(Sfx::Shield).pitch(0.6));
        }
    }
    if !any && status.active {
        status.active = false;
    }
}

fn boss_defeat(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut sfx: MessageWriter<PlaySfx>,
    bosses: Query<(Entity, &Health, &Transform), (With<Boss>, Without<BossDying>)>,
    shots: Query<(Entity, &Transform), With<EnemyShot>>,
) {
    for (e, health, t) in &bosses {
        if health.hp > 0.0 {
            continue;
        }
        commands.entity(e).insert(BossDying { t: 0.0, boom_t: 0.0 }).remove::<Hitbox>();
        // Clear the screen of bullets, turning them into stars.
        for (se, st) in &shots {
            if rng.chance(0.25) {
                pickups::drop_stars(&mut commands, &art, &mut rng, st.translation.truncate(), 1);
            }
            commands.entity(se).try_despawn();
        }
        effects::explosion(&mut commands, &art, &mut rng, t.translation.truncate(), 60.0, false);
        sfx.write(PlaySfx::new(Sfx::ExplodeBig));
    }
}

fn boss_dying(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
    mut shake: ResMut<Shake>,
    mut sfx: MessageWriter<PlaySfx>,
    mut bosses: Query<(Entity, &Boss, &Enemy, &mut BossDying, &Transform)>,
    parts: Query<(Entity, &BossPart)>,
) {
    let dt = time.delta_secs();
    for (e, boss, enemy, mut dying, t) in &mut bosses {
        dying.t += dt;
        dying.boom_t -= dt;
        let pos = t.translation.truncate();
        let r = BOSSES[boss.def].radius * 2.0;
        if dying.boom_t <= 0.0 {
            dying.boom_t = 0.11;
            let p = pos + rng.in_disc(r);
            let size = rng.range(20.0, 45.0);
            effects::explosion(&mut commands, &art, &mut rng, p, size, false);
            sfx.write(PlaySfx::new(Sfx::ExplodeSmall).pitch(rng.range(0.7, 1.1)));
            shake.add(0.15);
        }
        if dying.t >= DEATH_TIME {
            for _ in 0..6 {
                let p = pos + rng.in_disc(r * 0.7);
                effects::explosion(&mut commands, &art, &mut rng, p, 70.0, false);
            }
            effects::explosion(&mut commands, &art, &mut rng, pos, 140.0, false);
            effects::shockwave(&mut commands, &art, pos, 80.0, 1600.0, 0.9, LinearRgba::new(3.0, 2.2, 1.4, 0.9));
            effects::screen_flash(&mut commands, &art, Color::WHITE, 0.85, 0.9);
            effects::floating_text(&mut commands, pos, enemy.score.to_string(), Color::srgb(1.0, 0.9, 0.5), 40.0);
            pickups::drop_stars(&mut commands, &art, &mut rng, pos, enemy.stars);
            shake.add(1.0);
            sfx.write(PlaySfx::new(Sfx::ExplodeBig).pitch(0.7));
            run.score += enemy.score;
            run.end(Outcome::Victory, 4.5);
            for (pe, part) in &parts {
                if part.boss == e {
                    commands.entity(pe).try_despawn();
                }
            }
            commands.entity(e).try_despawn();
        }
    }
}
