//! Stars (currency), power-ups and stranded survivors.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::common::*;
use crate::effects;
use crate::lights::{self, lumens_for, Blink};
use crate::player::{Player, MAX_POWER};
use crate::rng::Rng;
use crate::state::{AppState, Run};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerKind {
    Weapon,
    Shield,
    Repair,
    Bomb,
}

impl PowerKind {
    fn index(self) -> usize {
        match self {
            PowerKind::Weapon => 0,
            PowerKind::Shield => 1,
            PowerKind::Repair => 2,
            PowerKind::Bomb => 3,
        }
    }

    fn color(self) -> LinearRgba {
        match self {
            PowerKind::Weapon => LinearRgba::rgb(3.2, 1.6, 0.3),
            PowerKind::Shield => LinearRgba::rgb(0.5, 1.8, 3.4),
            PowerKind::Repair => LinearRgba::rgb(0.6, 3.0, 0.8),
            PowerKind::Bomb => LinearRgba::rgb(3.2, 0.5, 0.5),
        }
    }
}

#[derive(Clone, Copy)]
pub enum PickupKind {
    Star(u32),
    Power(PowerKind),
}

#[derive(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    vel: Vec2,
    age: f32,
    seed: f32,
    sparkle: f32,
}

/// The spinning star model inside a star pickup.
#[derive(Component)]
struct StarSpin {
    seed: f32,
}

/// Soft pulsing glow behind stars and power-ups.
#[derive(Component)]
struct Halo {
    base: f32,
    seed: f32,
}

#[derive(Component)]
pub struct Survivor {
    progress: f32,
}

#[derive(Component)]
struct RescueBarFill;

#[derive(Component)]
struct Beacon;

const RESCUE_TIME: f32 = 1.2;
const RESCUE_RADIUS: f32 = 80.0;
const BAR_W: f32 = 46.0;

pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (move_pickups, collect_pickups).chain().in_set(GameSet::Resolve))
            .add_systems(Update, rescue_survivors.in_set(GameSet::Resolve))
            .add_systems(Update, (spin_stars, pulse_halos));
    }
}

fn fx_quad(art: &Art, material: &Handle<StandardMaterial>, z: f32, size: f32) -> impl Bundle {
    (
        Mesh3d(art.quad.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(0.0, 0.0, z).with_scale(Vec3::new(size, size, 1.0)),
        NotShadowCaster,
        NotShadowReceiver,
    )
}

pub fn drop_stars(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2, count: u32) {
    let big = count / 5;
    let small = count % 5;
    for i in 0..(big + small) {
        let value = if i < big { 5 } else { 1 };
        let (mesh, halo) = if value > 1 { (art.big_star.clone(), 70.0) } else { (art.star.clone(), 46.0) };
        let vel = rng.dir() * rng.range(60.0, 200.0) + Vec2::Y * 60.0;
        let seed = rng.range(0.0, 10.0);
        commands.spawn((
            Transform::from_translation((pos + rng.in_disc(10.0)).extend(z::PICKUP + rng.f32() * 3.0)),
            Visibility::default(),
            Pickup {
                kind: PickupKind::Star(value),
                vel,
                age: 0.0,
                seed,
                sparkle: rng.range(0.0, 0.6),
            },
            Offscreen::new(40.0),
            DespawnOnExit(AppState::Playing),
            children![
                (fx_quad(art, &art.mat_star_halo, -4.0, halo), Halo { base: halo, seed }),
                (
                    StarSpin { seed },
                    Mesh3d(mesh),
                    MeshMaterial3d(art.mat_star.clone()),
                    Transform::default(),
                ),
            ],
        ));
    }
}

pub fn drop_power(commands: &mut Commands, art: &Art, pos: Vec2, kind: PowerKind) {
    let k = kind.index();
    commands.spawn((
        Transform::from_translation(pos.extend(z::PICKUP + 4.0)),
        Visibility::default(),
        Pickup {
            kind: PickupKind::Power(kind),
            vel: Vec2::Y * 120.0,
            age: 0.0,
            seed: 0.0,
            sparkle: 1.0,
        },
        Offscreen::new(60.0),
        DespawnOnExit(AppState::Playing),
        children![
            (fx_quad(art, &art.mat_power_glow[k], -2.0, 76.0), Halo { base: 76.0, seed: 0.0 }),
            fx_quad(art, &art.mat_power_disc[k], 0.0, 40.0),
            fx_quad(art, &art.mat_power_ring[k], 0.5, 48.0),
            (
                Mesh3d(art.icons[k].clone()),
                MeshMaterial3d(art.mat_icon.clone()),
                Transform::from_xyz(0.0, 0.0, 1.0),
                NotShadowCaster,
            ),
        ],
    ));
}

pub fn spawn_survivor(commands: &mut Commands, art: &Art, pos: Vec3) {
    let survivor = commands
        .spawn((
            Mesh3d(art.survivor.paint.clone()),
            MeshMaterial3d(art.mat_paint.clone()),
            Transform::from_translation(pos),
            Survivor { progress: 0.0 },
            Scrolls,
            Offscreen::new(30.0),
            DespawnOnExit(AppState::Playing),
            children![
                (Beacon, fx_quad(art, &art.mat_beacon, 3.0, 60.0)),
                (
                    Blink {
                        base: lumens_for(16000.0, 30.0),
                        rate: 1.4,
                    },
                    lights::point(Color::srgb(1.0, 0.85, 0.4), 0.0, 140.0),
                    Transform::from_xyz(0.0, 0.0, 30.0),
                ),
                (
                    Mesh3d(art.quad.clone()),
                    MeshMaterial3d(art.mat_bar_bg.clone()),
                    Transform::from_xyz(0.0, 28.0, 30.0).with_scale(Vec3::new(BAR_W + 4.0, 8.0, 1.0)),
                    NotShadowCaster,
                    NotShadowReceiver,
                ),
                (
                    RescueBarFill,
                    Mesh3d(art.quad.clone()),
                    MeshMaterial3d(art.mat_bar_fill.clone()),
                    Transform::from_xyz(-BAR_W / 2.0, 28.0, 31.0).with_scale(Vec3::new(0.001, 4.0, 1.0)),
                    NotShadowCaster,
                    NotShadowReceiver,
                ),
            ],
        ))
        .id();
    let _ = survivor;
}

fn move_pickups(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    scroll: Res<ScrollSpeed>,
    player: Query<(&Player, &Transform), Without<Pickup>>,
    mut q: Query<(&mut Pickup, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let player = player.single().ok();
    for (mut p, mut t) in &mut q {
        p.age += dt;
        let pos = t.translation.truncate();
        let is_star = matches!(p.kind, PickupKind::Star(_));
        let fall = if is_star { -scroll.0 - 30.0 } else { -60.0 };
        p.vel = p.vel.lerp(Vec2::new(0.0, fall), 1.0 - (-2.2 * dt).exp());

        if let Some((pl, pt)) = player {
            let to = pt.translation.truncate() - pos;
            let radius = if is_star { pl.magnet_radius() } else { 90.0 };
            if to.length() < radius && p.age > 0.25 && pl.hp > 0.0 {
                let pull = to.normalize_or_zero() * 900.0;
                p.vel = p.vel.lerp(pull, 1.0 - (-9.0 * dt).exp());
            }
        }
        t.translation += (p.vel * dt).extend(0.0);

        if is_star {
            // Occasional twinkle.
            p.sparkle -= dt;
            if p.sparkle <= 0.0 {
                p.sparkle = 0.5 + (p.seed * 7.3).fract() * 0.6;
                let offset = Vec2::from_angle(p.age * 3.0 + p.seed) * 9.0;
                effects::puff(
                    &mut commands,
                    &art,
                    (t.translation.truncate() + offset).extend(t.translation.z + 6.0),
                    Vec2::ZERO,
                    0.25,
                    (4.0, 16.0),
                    LinearRgba::rgb(3.0, 2.6, 1.4),
                );
            }
        } else {
            t.translation.x += (p.age * 2.0).sin() * 30.0 * dt;
            let pulse = 1.0 + 0.08 * (p.age * 6.0).sin();
            t.scale = Vec3::splat(pulse);
        }
    }
}

/// Stars spin and flip like coins, catching the sunlight.
fn spin_stars(time: Res<Time>, mut q: Query<(&StarSpin, &mut Transform)>) {
    let s = time.elapsed_secs();
    for (spin, mut t) in &mut q {
        t.rotation = Quat::from_rotation_y((s + spin.seed) * 4.0) * Quat::from_rotation_z((s + spin.seed) * 1.2);
    }
}

fn pulse_halos(time: Res<Time>, mut q: Query<(&Halo, &mut Transform)>) {
    let s = time.elapsed_secs();
    for (halo, mut t) in &mut q {
        let k = halo.base * (1.0 + 0.16 * ((s + halo.seed) * 5.0).sin());
        t.scale = Vec3::new(k, k, 1.0);
    }
}

fn collect_pickups(
    mut commands: Commands,
    art: Res<Art>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut sfx: MessageWriter<PlaySfx>,
    mut voice: MessageWriter<crate::voice::Say>,
    mut player: Query<(&mut Player, &Transform), Without<Pickup>>,
    q: Query<(Entity, &Pickup, &Transform)>,
) {
    let Ok((mut player, pt)) = player.single_mut() else { return };
    if player.hp <= 0.0 {
        return;
    }
    let pp = pt.translation.truncate();
    for (e, pickup, t) in &q {
        let pos = t.translation.truncate();
        if pos.distance_squared(pp) > 36.0 * 36.0 {
            continue;
        }
        commands.entity(e).try_despawn();
        let fx_pos = pos.extend(z::FX);
        match pickup.kind {
            PickupKind::Star(v) => {
                run.stars += v;
                run.score += 10 * v;
                sfx.write(PlaySfx::new(Sfx::Star).pitch(rng.range(0.95, 1.12)));
                effects::puff(&mut commands, &art, fx_pos, Vec2::ZERO, 0.2, (30.0, 6.0), LinearRgba::rgb(3.0, 2.2, 0.5));
            }
            PickupKind::Power(kind) => {
                use crate::voice::Line;
                let (label, line) = match kind {
                    PowerKind::Weapon => {
                        if player.power < MAX_POWER {
                            player.power += 1;
                            if player.power == MAX_POWER {
                                ("MAX POWER", Line::MaxPower)
                            } else {
                                ("POWER UP", Line::PowerUp)
                            }
                        } else {
                            run.score += 2000;
                            ("+2000", Line::MaxPower)
                        }
                    }
                    PowerKind::Shield => {
                        player.shield = 9.0;
                        ("SHIELD", Line::Shield)
                    }
                    PowerKind::Repair => {
                        player.hp = (player.hp + 3.0).min(player.max_hp);
                        ("REPAIR", Line::Repair)
                    }
                    PowerKind::Bomb => {
                        player.bombs = (player.bombs + 1).min(5);
                        ("+1 BOMB", Line::BombReady)
                    }
                };
                effects::floating_text(&mut commands, fx_pos + Vec3::Y * 30.0, label, Color::LinearRgba(kind.color()), 26.0);
                effects::shockwave(&mut commands, &art, fx_pos, 20.0, 180.0, 0.35, kind.color());
                sfx.write(PlaySfx::new(if kind == PowerKind::Shield { Sfx::Shield } else { Sfx::PowerUp }));
                voice.write(crate::voice::Say(line));
            }
        }
    }
}

fn rescue_survivors(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    mut run: ResMut<Run>,
    mut sfx: MessageWriter<PlaySfx>,
    mut voice: MessageWriter<crate::voice::Say>,
    player: Query<(&Player, &Transform), Without<Survivor>>,
    mut survivors: Query<(Entity, &mut Survivor, &Transform, &Children)>,
    mut fills: Query<&mut Transform, (With<RescueBarFill>, Without<Survivor>, Without<Beacon>, Without<Player>)>,
    mut beacons: Query<&mut Transform, (With<Beacon>, Without<Survivor>, Without<RescueBarFill>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let player = player.single().ok().filter(|(p, _)| p.hp > 0.0);
    for (e, mut s, t, children) in &mut survivors {
        let pos = t.translation.truncate();
        let near = player
            .map(|(_, pt)| pt.translation.truncate().distance(pos) < RESCUE_RADIUS)
            .unwrap_or(false);
        if near {
            s.progress += dt;
        } else {
            s.progress = (s.progress - dt * 0.3).max(0.0);
        }
        let k = (s.progress / RESCUE_TIME).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut ft) = fills.get_mut(child) {
                ft.scale.x = (BAR_W * k).max(0.001);
                ft.translation.x = -BAR_W / 2.0 + BAR_W * k / 2.0;
            }
            if let Ok(mut bt) = beacons.get_mut(child) {
                let w = (time.elapsed_secs() * 2.0).fract();
                let size = 60.0 * (0.6 + w * 0.9);
                bt.scale = Vec3::new(size, size, 1.0);
            }
        }
        if s.progress >= RESCUE_TIME {
            run.rescued += 1;
            run.score += 1000;
            run.stars += 10;
            let fx = t.translation + Vec3::new(0.0, 30.0, 20.0);
            effects::floating_text(&mut commands, fx, "RESCUED!", Color::srgb(0.6, 1.0, 0.6), 26.0);
            effects::shockwave(&mut commands, &art, fx, 20.0, 160.0, 0.4, LinearRgba::rgb(0.6, 3.0, 0.8));
            sfx.write(PlaySfx::new(Sfx::Rescue));
            voice.write(crate::voice::Say(crate::voice::Line::Rescued));
            commands.entity(e).try_despawn();
        }
    }
}
