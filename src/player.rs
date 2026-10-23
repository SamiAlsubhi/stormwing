//! The player's aircraft: movement, cannon, seeker missiles and bombs.

use crate::art::{player_set, Art};
use crate::audio::{PlaySfx, Sfx};
use crate::combat::{BombDetonated, PlayerShot};
use crate::common::*;
use crate::effects;
use crate::lights::{self, lumens_for, MuzzleLight};
use crate::enemy::Enemy;
use crate::rng::Rng;
use crate::save::{Profile, Upgrade};
use crate::state::{AppState, Outcome, Run};
use crate::touch::{self, TouchInput};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

pub const PLAYER_SPEED: f32 = 540.0;
pub const PLAYER_HITBOX: f32 = 11.0;
pub const MAX_POWER: u8 = 3;
const INTRO_TIME: f32 = 1.3;

#[derive(Component)]
pub struct Player {
    pub hp: f32,
    pub max_hp: f32,
    /// Weapon power-ups collected during this stage.
    pub power: u8,
    pub bombs: u8,
    pub invuln: f32,
    pub shield: f32,
    pub vel: Vec2,
    pub cannon: u8,
    pub rate: u8,
    pub missiles: u8,
    pub magnet: u8,
    fire_timer: f32,
    missile_timer: f32,
    bomb_cooldown: f32,
    intro: f32,
    blink: f32,
}

impl Player {
    pub fn magnet_radius(&self) -> f32 {
        110.0 + 70.0 * self.magnet as f32
    }
    pub fn controllable(&self) -> bool {
        self.intro <= 0.0
    }
}

#[derive(Component)]
pub struct EngineFlame;

#[derive(Component)]
pub struct ShieldBubble;

#[derive(Component)]
pub struct Missile {
    pub damage: f32,
    pub target: Option<Entity>,
    pub vel: Vec2,
    pub age: f32,
    trail: f32,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), spawn_player)
            .add_systems(
                Update,
                (player_movement, player_weapons, player_bomb)
                    .chain()
                    .in_set(GameSet::Input),
            )
            .add_systems(Update, steer_missiles.in_set(GameSet::Move))
            .add_systems(Update, player_visuals.in_set(GameSet::Resolve));
    }
}

fn spawn_player(mut commands: Commands, art: Res<Art>, mut meshes: ResMut<Assets<Mesh>>, profile: Res<Profile>) {
    let hull = profile.level(Upgrade::Hull) as f32;
    let max_hp = 6.0 + 2.0 * hull;
    // The jet shows its hangar upgrades (extra guns, missile rails, armour...).
    let model = player_set(&mut meshes, profile.upgrades);
    let player = commands.spawn((
        Player {
            hp: max_hp,
            max_hp,
            power: 0,
            bombs: 2,
            invuln: INTRO_TIME + 1.0,
            shield: 0.0,
            vel: Vec2::ZERO,
            cannon: profile.level(Upgrade::Cannon),
            rate: profile.level(Upgrade::FireRate),
            missiles: profile.level(Upgrade::Missiles),
            magnet: profile.level(Upgrade::Magnet),
            fire_timer: 0.0,
            missile_timer: 1.0,
            bomb_cooldown: 0.0,
            intro: INTRO_TIME,
            blink: 0.0,
        },
        Mesh3d(model.paint.clone()),
        MeshMaterial3d(art.mat_paint.clone()),
        Transform::from_xyz(0.0, -HALF_H - 120.0, z::PLAYER),
        Visibility::default(),
        DespawnOnExit(AppState::Playing),
        children![
            (
                EngineFlame,
                Mesh3d(art.quad.clone()),
                MeshMaterial3d(art.mat_engine.clone()),
                Transform::from_xyz(4.6, -55.0, 0.0).with_scale(Vec3::new(9.0, 18.0, 1.0)),
                NotShadowCaster,
                NotShadowReceiver,
            ),
            (
                EngineFlame,
                Mesh3d(art.quad.clone()),
                MeshMaterial3d(art.mat_engine.clone()),
                Transform::from_xyz(-4.6, -55.0, 0.0).with_scale(Vec3::new(9.0, 18.0, 1.0)),
                NotShadowCaster,
                NotShadowReceiver,
            ),
            // The gun flash, lighting up the nose and wings with each volley.
            (
                MuzzleLight {
                    kick: 0.0,
                    peak: lumens_for(18000.0, 40.0),
                },
                lights::point(Color::srgb(1.0, 0.8, 0.45), 0.0, 170.0),
                Transform::from_xyz(0.0, 62.0, 16.0),
            ),
            (
                ShieldBubble,
                Mesh3d(art.quad.clone()),
                MeshMaterial3d(art.mat_shield.clone()),
                Transform::from_xyz(0.0, 0.0, 12.0).with_scale(Vec3::new(124.0, 124.0, 1.0)),
                Visibility::Hidden,
                NotShadowCaster,
                NotShadowReceiver,
            ),
        ],
    )).id();
    attach_model_parts(&mut commands, player, &model, &art);
}

fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    touches: Res<Touches>,
    mut touch_input: ResMut<TouchInput>,
    run: Res<Run>,
    mut q: Query<(&mut Player, &mut Transform)>,
) {
    let Ok((mut player, mut t)) = q.single_mut() else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    if player.intro > 0.0 {
        player.intro -= dt;
        let k = 1.0 - (player.intro / INTRO_TIME).clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - k).powi(3);
        t.translation.y = -HALF_H - 120.0 + ease * 300.0;
        player.vel = Vec2::Y * 200.0;
        return;
    }

    // After victory the pilot flies off the top of the screen.
    if run.outcome == Some(Outcome::Victory) {
        player.vel.y = (player.vel.y + 900.0 * dt).min(1400.0);
        player.vel.x *= 0.9;
        t.translation += (player.vel * dt).extend(0.0);
        return;
    }

    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    for pad in &pads {
        let stick = pad.left_stick();
        if stick.length() > 0.2 {
            dir += stick;
        }
        if pad.pressed(GamepadButton::DPadLeft) {
            dir.x -= 1.0;
        }
        if pad.pressed(GamepadButton::DPadRight) {
            dir.x += 1.0;
        }
        if pad.pressed(GamepadButton::DPadUp) {
            dir.y += 1.0;
        }
        if pad.pressed(GamepadButton::DPadDown) {
            dir.y -= 1.0;
        }
    }
    // Touch: the plane follows the steering finger's movement one-to-one.
    if let Ok((cam, cam_t)) = camera.single() {
        if let Some(delta) = touch::drag_delta(&touches, &mut touch_input, cam, cam_t) {
            let mut p = t.translation.truncate() + delta;
            p.x = p.x.clamp(-HALF_W + 34.0, HALF_W - 34.0);
            p.y = p.y.clamp(-HALF_H + 50.0, HALF_H - 70.0);
            let k = 1.0 - (-12.0 * dt).exp();
            player.vel = player.vel.lerp(delta / dt, k);
            t.translation.x = p.x;
            t.translation.y = p.y;
            return;
        }
    }

    let dir = dir.clamp_length_max(1.0);
    let focus = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let speed = if focus { PLAYER_SPEED * 0.5 } else { PLAYER_SPEED };
    let mut target = dir * speed;

    // Holding the left mouse button steers the plane towards the cursor.
    if mouse.pressed(MouseButton::Left) && dir == Vec2::ZERO {
        if let (Ok(window), Ok((cam, cam_t))) = (windows.single(), camera.single()) {
            if let Some(cursor) = window.cursor_position() {
                if let Some(world) = screen_to_world(cam, cam_t, cursor) {
                    // Aim slightly above the cursor so the plane isn't hidden under it.
                    let goal = world + Vec2::new(0.0, 40.0);
                    let delta = goal - t.translation.truncate();
                    target = (delta * 12.0).clamp_length_max(PLAYER_SPEED * 1.5);
                }
            }
        }
    }

    let k = 1.0 - (-18.0 * dt).exp();
    player.vel = player.vel.lerp(target, k);
    let mut p = t.translation.truncate() + player.vel * dt;
    p.x = p.x.clamp(-HALF_W + 34.0, HALF_W - 34.0);
    p.y = p.y.clamp(-HALF_H + 50.0, HALF_H - 70.0);
    t.translation.x = p.x;
    t.translation.y = p.y;
}

/// Bullet spread for a given total cannon power: `(x offset, angle in degrees)`.
fn cannon_pattern(power: u8) -> &'static [(f32, f32)] {
    match power {
        0 | 1 => &[(-6.0, 0.0), (6.0, 0.0)],
        2 => &[(-9.0, 0.0), (0.0, 0.0), (9.0, 0.0)],
        3 => &[(-6.0, 0.0), (6.0, 0.0), (-16.0, -4.0), (16.0, 4.0)],
        4 => &[(0.0, 0.0), (-9.0, 0.0), (9.0, 0.0), (-18.0, -6.0), (18.0, 6.0)],
        5 => &[(0.0, 0.0), (-9.0, 0.0), (9.0, 0.0), (-18.0, -5.0), (18.0, 5.0), (-30.0, -13.0), (30.0, 13.0)],
        6 | 7 => &[
            (-5.0, 0.0),
            (5.0, 0.0),
            (-14.0, -2.0),
            (14.0, 2.0),
            (-22.0, -7.0),
            (22.0, 7.0),
            (-32.0, -14.0),
            (32.0, 14.0),
        ],
        _ => &[
            (0.0, 0.0),
            (-8.0, 0.0),
            (8.0, 0.0),
            (-16.0, -2.5),
            (16.0, 2.5),
            (-24.0, -7.0),
            (24.0, 7.0),
            (-34.0, -13.0),
            (34.0, 13.0),
            (-38.0, -22.0),
            (38.0, 22.0),
        ],
    }
}

fn player_weapons(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    run: Res<Run>,
    mut sfx: MessageWriter<PlaySfx>,
    mut q: Query<(&mut Player, &Transform)>,
    mut muzzles: Query<(&mut MuzzleLight, &mut PointLight)>,
) {
    let Ok((mut player, t)) = q.single_mut() else { return };
    if !player.controllable() || run.outcome.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let pos = t.translation.truncate();

    player.fire_timer -= dt;
    if player.fire_timer <= 0.0 {
        let interval = (0.118 - 0.013 * player.rate as f32).max(0.06);
        player.fire_timer += interval;
        if player.fire_timer < 0.0 {
            player.fire_timer = 0.0;
        }
        let total = player.cannon + player.power;
        let damage = 1.0 + 0.22 * player.cannon as f32 + 0.12 * player.power as f32;
        let width = 8.0 + total.min(8) as f32 * 0.6;
        let (color, material) = if player.power >= 2 {
            (LinearRgba::rgb(1.6, 3.0, 3.6), art.mat_tracer_power.clone())
        } else {
            (LinearRgba::rgb(3.6, 2.9, 1.1), art.mat_tracer.clone())
        };
        for &(dx, deg) in cannon_pattern(total) {
            let dir = Vec2::from_angle(std::f32::consts::FRAC_PI_2 - deg.to_radians());
            commands.spawn((
                Mesh3d(art.quad.clone()),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::from_translation((pos + Vec2::new(dx, 36.0)).extend(z::PLAYER_SHOT))
                    .with_rotation(Quat::from_rotation_z(dir.to_angle() - std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(width, 34.0, 1.0)),
                Velocity(dir * 1450.0),
                PlayerShot { damage, radius: 8.0 },
                Offscreen::new(40.0),
                DespawnOnExit(AppState::Playing),
            ));
        }
        effects::puff(
            &mut commands,
            &art,
            (pos + Vec2::new(0.0, 50.0)).extend(z::PLAYER + 8.0),
            Vec2::Y * 60.0,
            0.06,
            (26.0, 12.0),
            color,
        );
        let tint = color.with_alpha(1.0) * (1.0 / color.red.max(color.green).max(color.blue));
        for (mut muzzle, mut light) in &mut muzzles {
            muzzle.kick = 1.0;
            light.color = Color::LinearRgba(tint);
        }
        sfx.write(PlaySfx::new(Sfx::Shoot));
    }

    if player.missiles > 0 {
        player.missile_timer -= dt;
        if player.missile_timer <= 0.0 {
            player.missile_timer = 1.35 - 0.3 * (player.missiles as f32 - 1.0);
            let damage = 5.0 + 3.0 * player.missiles as f32;
            for side in [-1.0f32, 1.0] {
                let vel = Vec2::new(side * 260.0, 120.0);
                let missile = commands.spawn((
                    Mesh3d(art.missile.paint.clone()),
                    MeshMaterial3d(art.mat_paint.clone()),
                    Transform::from_translation((pos + Vec2::new(side * 30.0, -8.0)).extend(z::PLAYER - 6.0)),
                    Missile {
                        damage,
                        target: None,
                        vel,
                        age: 0.0,
                        trail: 0.0,
                    },
                    Offscreen::new(80.0),
                    DespawnOnExit(AppState::Playing),
                )).id();
                attach_model_parts(&mut commands, missile, &art.missile, &art);
            }
            sfx.write(PlaySfx::new(Sfx::Missile));
        }
    }
}

fn player_bomb(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    touch_input: Res<TouchInput>,
    pads: Query<&Gamepad>,
    time: Res<Time>,
    run: Res<Run>,
    mut bombs: MessageWriter<BombDetonated>,
    mut q: Query<(&mut Player, &Transform)>,
) {
    let Ok((mut player, t)) = q.single_mut() else { return };
    player.bomb_cooldown -= time.delta_secs();
    if !player.controllable() || run.outcome.is_some() {
        return;
    }
    let pressed = keys.any_just_pressed([KeyCode::Space, KeyCode::KeyX, KeyCode::KeyB])
        || mouse.just_pressed(MouseButton::Right)
        || touch::second_finger_tap(&touches, &touch_input)
        || pads.iter().any(|p| {
            p.just_pressed(GamepadButton::South) || p.just_pressed(GamepadButton::East)
        });
    if pressed && player.bombs > 0 && player.bomb_cooldown <= 0.0 {
        player.bombs -= 1;
        player.bomb_cooldown = 1.0;
        player.invuln = player.invuln.max(1.6);
        bombs.write(BombDetonated {
            pos: t.translation.truncate(),
        });
    }
}

fn steer_missiles(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    mut missiles: Query<(Entity, &mut Missile, &mut Transform), Without<Enemy>>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Missile>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (entity, mut m, mut t) in &mut missiles {
        m.age += dt;
        if m.age > 4.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let pos = t.translation.truncate();
        let target_alive = m
            .target
            .and_then(|e| enemies.get(e).ok())
            .map(|(_, et, _)| et.translation.truncate());
        let target_pos = match target_alive {
            Some(p) => Some(p),
            None => {
                // Pick the closest visible enemy, preferring ones ahead of us.
                let best = enemies
                    .iter()
                    .filter(|(_, et, e)| e.targetable && et.translation.y < HALF_H - 10.0 && et.translation.y > -HALF_H)
                    .min_by(|a, b| {
                        let da = score_target(pos, a.1.translation.truncate());
                        let db = score_target(pos, b.1.translation.truncate());
                        da.total_cmp(&db)
                    });
                m.target = best.map(|b| b.0);
                best.map(|b| b.1.translation.truncate())
            }
        };

        let speed = (m.vel.length() + 1500.0 * dt).min(950.0);
        let desired = match target_pos {
            Some(tp) if m.age > 0.12 => (tp - pos).normalize_or(Vec2::Y),
            _ => (m.vel.normalize_or(Vec2::Y) + Vec2::Y * dt * 4.0).normalize(),
        };
        let current = m.vel.normalize_or(Vec2::Y);
        let turn = (7.0 * dt).min(1.0);
        let dir = current.lerp(desired, turn).normalize_or(desired);
        m.vel = dir * speed;
        t.translation += (m.vel * dt).extend(0.0);
        t.rotation = Quat::from_rotation_z(dir.to_angle() - std::f32::consts::FRAC_PI_2);

        m.trail -= dt;
        if m.trail <= 0.0 {
            m.trail = 0.025;
            effects::smoke_puff(&mut commands, &art, (pos - dir * 12.0).extend(t.translation.z - 1.0), -dir * 20.0, 0.45, (10.0, 26.0), 0.5);
        }
    }
}

fn score_target(from: Vec2, to: Vec2) -> f32 {
    let d = to - from;
    d.length() + if d.y < 0.0 { 400.0 } else { 0.0 }
}

fn player_visuals(
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut players: Query<(&mut Player, &mut Transform, &mut Visibility, &Children)>,
    art: Res<Art>,
    mut flames: Query<&mut Transform, (With<EngineFlame>, Without<Player>, Without<ShieldBubble>)>,
    mut bubbles: Query<
        (&mut Transform, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility),
        (With<ShieldBubble>, Without<Player>, Without<EngineFlame>),
    >,
) {
    let dt = time.delta_secs();
    let Ok((mut player, mut t, mut vis, children)) = players.single_mut() else { return };
    player.invuln = (player.invuln - dt).max(0.0);
    player.shield = (player.shield - dt).max(0.0);

    // Roll into turns: the right wing dips when moving right.
    let bank = (player.vel.x / PLAYER_SPEED).clamp(-1.0, 1.0);
    t.rotation = Quat::from_rotation_y(bank * 0.75);

    // Blink while invulnerable after a hit (but not during the intro or shield).
    if player.invuln > 0.0 && player.controllable() && player.shield <= 0.0 {
        player.blink += dt;
        *vis = if (player.blink * 14.0) as i32 % 2 == 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    } else {
        *vis = Visibility::Inherited;
    }

    let thrust = 1.0 + (player.vel.y / PLAYER_SPEED).clamp(-0.5, 1.0) * 0.5;
    for child in children.iter() {
        if let Ok(mut ft) = flames.get_mut(child) {
            let flicker = rng.range(0.85, 1.15);
            ft.scale = Vec3::new(9.0, 18.0 * thrust * flicker, 1.0);
            ft.translation.y = -51.0 - 9.0 * thrust * flicker;
        }
        if let Ok((mut bt, mut mat, mut bubble_vis)) = bubbles.get_mut(child) {
            let on = player.shield > 0.0;
            let fading = player.shield < 1.5 && (player.shield * 10.0) as i32 % 2 == 0;
            let want_vis = if on { Visibility::Inherited } else { Visibility::Hidden };
            if *bubble_vis != want_vis {
                *bubble_vis = want_vis;
            }
            let want = if fading { &art.mat_shield_dim } else { &art.mat_shield };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
            let pulse = 124.0 * (1.0 + 0.05 * (time.elapsed_secs() * 8.0).sin());
            bt.scale = Vec3::new(pulse, pulse, 1.0);
        }
    }
}
