//! Stars (currency), power-ups and stranded survivors.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::common::*;
use crate::effects;
use crate::player::{Player, MAX_POWER};
use crate::rng::Rng;
use crate::state::{AppState, Run};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerKind {
    Weapon,
    Shield,
    Repair,
    Bomb,
}

impl PowerKind {
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
            .add_systems(Update, rescue_survivors.in_set(GameSet::Resolve));
    }
}

pub fn drop_stars(commands: &mut Commands, art: &Art, rng: &mut Rng, pos: Vec2, count: u32) {
    let big = count / 5;
    let small = count % 5;
    for i in 0..(big + small) {
        let value = if i < big { 5 } else { 1 };
        let mesh = if value > 1 { art.big_star.clone() } else { art.star.clone() };
        let vel = rng.dir() * rng.range(60.0, 200.0) + Vec2::Y * 60.0;
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(art.mat_base.clone()),
            Transform::from_translation((pos + rng.in_disc(10.0)).extend(z::PICKUP + rng.f32() * 0.5)),
            Pickup {
                kind: PickupKind::Star(value),
                vel,
                age: 0.0,
                seed: rng.range(0.0, 10.0),
            },
            Offscreen::new(40.0),
            DespawnOnExit(AppState::Playing),
        ));
    }
}

pub fn drop_power(commands: &mut Commands, art: &Art, pos: Vec2, kind: PowerKind) {
    let icon = match kind {
        PowerKind::Weapon => art.icons[0].clone(),
        PowerKind::Shield => art.icons[1].clone(),
        PowerKind::Repair => art.icons[2].clone(),
        PowerKind::Bomb => art.icons[3].clone(),
    };
    commands.spawn((
        Sprite {
            image: art.tex_glow.clone(),
            custom_size: Some(Vec2::splat(70.0)),
            color: Color::LinearRgba(kind.color()),
            ..default()
        },
        Transform::from_translation(pos.extend(z::PICKUP + 0.6)),
        Pickup {
            kind: PickupKind::Power(kind),
            vel: Vec2::Y * 120.0,
            age: 0.0,
            seed: 0.0,
        },
        Offscreen::new(60.0),
        DespawnOnExit(AppState::Playing),
        children![
            (
                Sprite {
                    image: art.tex_disc.clone(),
                    custom_size: Some(Vec2::splat(38.0)),
                    color: Color::LinearRgba(kind.color() * 0.35).with_alpha(1.0),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.05),
            ),
            (
                Sprite {
                    image: art.tex_ring.clone(),
                    custom_size: Some(Vec2::splat(46.0)),
                    color: Color::LinearRgba(kind.color()),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.1),
            ),
            (
                Mesh2d(icon),
                MeshMaterial2d(art.mat_base.clone()),
                Transform::from_xyz(0.0, 0.0, 0.2),
            ),
        ],
    ));
}

pub fn spawn_survivor(commands: &mut Commands, art: &Art, pos: Vec2) {
    commands.spawn((
        Mesh2d(art.survivor.clone()),
        MeshMaterial2d(art.mat_base.clone()),
        Transform::from_translation(pos.extend(z::SURVIVOR)),
        Survivor { progress: 0.0 },
        Scrolls,
        Offscreen::new(30.0),
        DespawnOnExit(AppState::Playing),
        children![
            (
                Beacon,
                Sprite {
                    image: art.tex_ring.clone(),
                    custom_size: Some(Vec2::splat(60.0)),
                    color: Color::linear_rgba(2.5, 2.2, 0.6, 0.8),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, -0.1),
            ),
            (
                Sprite::from_color(Color::srgba(0.0, 0.0, 0.0, 0.6), Vec2::new(BAR_W + 4.0, 8.0)),
                Transform::from_xyz(0.0, 26.0, 0.2),
            ),
            (
                RescueBarFill,
                Sprite::from_color(Color::linear_rgb(0.6, 2.8, 0.8), Vec2::new(BAR_W, 4.0)),
                Transform::from_xyz(-BAR_W / 2.0, 26.0, 0.3).with_scale(Vec3::new(0.001, 1.0, 1.0)),
            ),
        ],
    ));
}

fn move_pickups(
    time: Res<Time>,
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
            // Spin like a coin.
            t.rotate_z(1.5 * dt);
            t.scale.x = ((p.age + p.seed) * 5.0).cos().abs().max(0.25);
        } else {
            t.translation.x += (p.age * 2.0).sin() * 30.0 * dt;
            let pulse = 1.0 + 0.08 * (p.age * 6.0).sin();
            t.scale = Vec3::splat(pulse);
        }
    }
}

fn collect_pickups(
    mut commands: Commands,
    art: Res<Art>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut sfx: MessageWriter<PlaySfx>,
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
        match pickup.kind {
            PickupKind::Star(v) => {
                run.stars += v;
                run.score += 10 * v;
                sfx.write(PlaySfx::new(Sfx::Star).pitch(rng.range(0.95, 1.12)));
                effects::puff(&mut commands, &art, pos, Vec2::ZERO, 0.2, (30.0, 6.0), LinearRgba::rgb(3.0, 2.2, 0.5));
            }
            PickupKind::Power(kind) => {
                let label = match kind {
                    PowerKind::Weapon => {
                        if player.power < MAX_POWER {
                            player.power += 1;
                            if player.power == MAX_POWER { "MAX POWER" } else { "POWER UP" }
                        } else {
                            run.score += 2000;
                            "+2000"
                        }
                    }
                    PowerKind::Shield => {
                        player.shield = 9.0;
                        "SHIELD"
                    }
                    PowerKind::Repair => {
                        player.hp = (player.hp + 3.0).min(player.max_hp);
                        "REPAIR"
                    }
                    PowerKind::Bomb => {
                        player.bombs = (player.bombs + 1).min(5);
                        "+1 BOMB"
                    }
                };
                effects::floating_text(&mut commands, pos + Vec2::Y * 30.0, label, Color::LinearRgba(kind.color()), 26.0);
                effects::shockwave(&mut commands, &art, pos, 20.0, 180.0, 0.35, kind.color());
                sfx.write(PlaySfx::new(if kind == PowerKind::Shield { Sfx::Shield } else { Sfx::PowerUp }));
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
    player: Query<(&Player, &Transform), Without<Survivor>>,
    mut survivors: Query<(Entity, &mut Survivor, &Transform, &Children)>,
    mut fills: Query<
        &mut Transform,
        (With<RescueBarFill>, Without<Survivor>, Without<Beacon>, Without<Player>),
    >,
    mut beacons: Query<
        (&mut Sprite, &mut Transform),
        (With<Beacon>, Without<Survivor>, Without<RescueBarFill>, Without<Player>),
    >,
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
                ft.scale.x = k.max(0.001);
                ft.translation.x = -BAR_W / 2.0 + BAR_W * k / 2.0;
            }
            if let Ok((mut sprite, mut bt)) = beacons.get_mut(child) {
                let w = (time.elapsed_secs() * 2.0).fract();
                bt.scale = Vec3::splat(0.6 + w * 0.9);
                // Quantised so the sprite only changes a few times per pulse.
                let alpha = (0.9 * (1.0 - w) * 8.0).round() / 8.0;
                if sprite.color.alpha() != alpha {
                    sprite.color.set_alpha(alpha);
                }
            }
        }
        if s.progress >= RESCUE_TIME {
            run.rescued += 1;
            run.score += 1000;
            run.stars += 10;
            effects::floating_text(&mut commands, pos + Vec2::Y * 30.0, "RESCUED!", Color::srgb(0.6, 1.0, 0.6), 26.0);
            effects::shockwave(&mut commands, &art, pos, 20.0, 160.0, 0.4, LinearRgba::rgb(0.6, 3.0, 0.8));
            sfx.write(PlaySfx::new(Sfx::Rescue));
            commands.entity(e).try_despawn();
        }
    }
}
