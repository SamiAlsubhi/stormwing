//! Projectiles, hit detection, damage and destruction.

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::common::*;
use crate::effects;
use crate::voice::{Line, Say};
use crate::enemy::{Enemy, EnemyKind};
use crate::pickups;
use crate::player::{Missile, Player, PLAYER_HITBOX};
use crate::rng::Rng;
use crate::state::{AppState, Outcome, Run};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

#[derive(Component)]
pub struct PlayerShot {
    pub damage: f32,
    pub radius: f32,
}

#[derive(Component)]
pub struct EnemyShot {
    pub damage: f32,
    pub radius: f32,
}

/// Boss cores can't be damaged while their turrets are still standing.
#[derive(Component)]
pub struct Shielded;

#[derive(Message)]
pub struct BombDetonated {
    pub pos: Vec2,
}

/// Lets the dev/test harness make the player invulnerable.
#[derive(Resource, Default)]
pub struct GodMode(pub bool);

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BombDetonated>()
            .init_resource::<GodMode>()
            .add_systems(
                Update,
                (
                    player_shots_hit,
                    missiles_hit,
                    enemy_shots_hit,
                    enemy_contact,
                    detonate_bombs,
                )
                    .chain()
                    .in_set(GameSet::Combat),
            )
            .add_systems(Update, (destroy_enemies, destroy_player).in_set(GameSet::Resolve));
    }
}

pub fn spawn_enemy_shot(commands: &mut Commands, art: &Art, pos: Vec2, vel: Vec2, big: bool) {
    let (size, material) = if big {
        (28.0, art.mat_orb_big.clone())
    } else {
        (19.0, art.mat_orb.clone())
    };
    commands.spawn((
        Mesh3d(art.quad.clone()),
        MeshMaterial3d(material),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_translation(pos.extend(z::ENEMY_SHOT)).with_scale(Vec3::new(size, size, 1.0)),
        Velocity(vel),
        EnemyShot {
            damage: 1.0,
            radius: size * 0.3,
        },
        Offscreen::new(30.0),
        DespawnOnExit(AppState::Playing),
    ));
}

/// Applies a hit to an enemy. Returns false if the hit was deflected by a shield.
fn hurt_enemy(health: &mut Health, flash: &mut HitFlash, shielded: bool, damage: f32) -> bool {
    if shielded {
        return false;
    }
    health.hp -= damage;
    common_flash(flash);
    true
}

fn common_flash(f: &mut HitFlash) {
    crate::common::flash(f);
}

type EnemyTargets<'a> = (
    &'a Transform,
    &'a Hitbox,
    &'a mut Health,
    &'a mut HitFlash,
    Has<Shielded>,
);

fn player_shots_hit(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut sfx: MessageWriter<PlaySfx>,
    shots: Query<(Entity, &Transform, &PlayerShot)>,
    mut enemies: Query<EnemyTargets, (With<Enemy>, Without<PlayerShot>)>,
) {
    for (shot_e, st, shot) in &shots {
        let sp = st.translation.truncate();
        for (et, hitbox, mut health, mut flash, shielded) in &mut enemies {
            if health.hp <= 0.0 || et.translation.y > HALF_H + 10.0 {
                continue;
            }
            let ep = et.translation.truncate();
            let r = hitbox.0 + shot.radius;
            if sp.distance_squared(ep) > r * r {
                continue;
            }
            commands.entity(shot_e).try_despawn();
            let hit_pos = (sp + Vec2::Y * 6.0).extend(et.translation.z + 8.0);
            if hurt_enemy(&mut health, &mut flash, shielded, shot.damage) {
                effects::hit_sparks(&mut commands, &art, &mut rng, hit_pos, LinearRgba::rgb(3.5, 2.6, 1.0));
                sfx.write(PlaySfx::new(Sfx::Hit).pitch(rng.range(0.9, 1.2)));
            } else {
                effects::hit_sparks(&mut commands, &art, &mut rng, hit_pos, LinearRgba::rgb(0.8, 2.0, 3.5));
            }
            break;
        }
    }
}

fn missiles_hit(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut sfx: MessageWriter<PlaySfx>,
    missiles: Query<(Entity, &Transform, &Missile)>,
    mut enemies: Query<EnemyTargets, (With<Enemy>, Without<Missile>)>,
) {
    for (me, mt, missile) in &missiles {
        let mp = mt.translation.truncate();
        for (et, hitbox, mut health, mut flash, shielded) in &mut enemies {
            if health.hp <= 0.0 || et.translation.y > HALF_H + 10.0 {
                continue;
            }
            let r = hitbox.0 + 8.0;
            if mp.distance_squared(et.translation.truncate()) > r * r {
                continue;
            }
            commands.entity(me).try_despawn();
            hurt_enemy(&mut health, &mut flash, shielded, missile.damage);
            effects::explosion(&mut commands, &art, &mut rng, mp.extend(et.translation.z), 10.0, false);
            sfx.write(PlaySfx::new(Sfx::Hit).pitch(0.7));
            break;
        }
    }
}

/// Damages the player unless they're protected. Returns true if the hit landed
/// (including when the shield absorbed it).
#[allow(clippy::too_many_arguments)]
pub fn hurt_player(
    player: &mut Player,
    amount: f32,
    run: &mut Run,
    shake: &mut Shake,
    sfx: &mut MessageWriter<PlaySfx>,
    voice: &mut MessageWriter<Say>,
    commands: &mut Commands,
    god: bool,
) -> bool {
    if player.invuln > 0.0 || !player.controllable() || run.outcome.is_some() {
        return false;
    }
    if player.shield > 0.0 {
        sfx.write(PlaySfx::new(Sfx::Shield).volume(0.6));
        return true;
    }
    let was_ok = player.hp > player.max_hp * 0.34;
    if !god {
        player.hp -= amount;
    }
    if was_ok && player.hp > 0.0 && player.hp <= player.max_hp * 0.34 {
        voice.write(Say(Line::HullCritical));
    }
    player.invuln = 1.4;
    run.damaged = true;
    shake.add(0.5);
    sfx.write(PlaySfx::new(Sfx::Hurt));
    effects::screen_flash(commands, Color::srgb(1.0, 0.15, 0.1), 0.28, 0.3);
    true
}

fn enemy_shots_hit(
    mut commands: Commands,
    art: Res<Art>,
    mut run: ResMut<Run>,
    mut shake: ResMut<Shake>,
    god: Res<GodMode>,
    mut sfx: MessageWriter<PlaySfx>,
    mut voice: MessageWriter<Say>,
    shots: Query<(Entity, &Transform, &EnemyShot)>,
    mut player: Query<(&mut Player, &Transform)>,
) {
    let Ok((mut player, pt)) = player.single_mut() else { return };
    let pp = pt.translation.truncate();
    let shield_r = if player.shield > 0.0 { 48.0 } else { 0.0 };
    for (e, t, shot) in &shots {
        let sp = t.translation.truncate();
        let r = PLAYER_HITBOX.max(shield_r) + shot.radius;
        if sp.distance_squared(pp) > r * r {
            continue;
        }
        // While invulnerable, bullets pass through harmlessly (except the shield eats them).
        if player.invuln > 0.0 && player.shield <= 0.0 {
            continue;
        }
        commands.entity(e).try_despawn();
        effects::puff(&mut commands, &art, sp.extend(z::ENEMY_SHOT), Vec2::ZERO, 0.15, (30.0, 8.0), LinearRgba::rgb(3.0, 1.0, 2.0));
        hurt_player(&mut player, shot.damage, &mut run, &mut shake, &mut sfx, &mut voice, &mut commands, god.0);
    }
}

fn enemy_contact(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut shake: ResMut<Shake>,
    god: Res<GodMode>,
    mut sfx: MessageWriter<PlaySfx>,
    mut voice: MessageWriter<Say>,
    mut player: Query<(&mut Player, &Transform), Without<Enemy>>,
    mut enemies: Query<(&Enemy, &Transform, &Hitbox, &mut Health, &mut HitFlash)>,
) {
    let Ok((mut player, pt)) = player.single_mut() else { return };
    let pp = pt.translation.truncate();
    for (enemy, et, hitbox, mut health, mut flash) in &mut enemies {
        if !enemy.contact || health.hp <= 0.0 {
            continue;
        }
        let r = hitbox.0 * 0.75 + PLAYER_HITBOX + 8.0;
        if et.translation.truncate().distance_squared(pp) > r * r {
            continue;
        }
        if hurt_player(&mut player, 2.0, &mut run, &mut shake, &mut sfx, &mut voice, &mut commands, god.0) {
            health.hp -= 15.0;
            common_flash(&mut flash);
        }
    }
}

fn detonate_bombs(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<Shake>,
    mut sfx: MessageWriter<PlaySfx>,
    mut voice: MessageWriter<Say>,
    mut bombs: MessageReader<BombDetonated>,
    shots: Query<(Entity, &Transform), With<EnemyShot>>,
    mut enemies: Query<(&Transform, &mut Health, &mut HitFlash, Has<Shielded>), With<Enemy>>,
) {
    for bomb in bombs.read() {
        for (e, t) in &shots {
            effects::puff(
                &mut commands,
                &art,
                t.translation,
                Vec2::ZERO,
                0.3,
                (22.0, 40.0),
                LinearRgba::new(1.5, 2.5, 3.5, 0.8),
            );
            commands.entity(e).try_despawn();
        }
        for (t, mut health, mut flash, shielded) in &mut enemies {
            let p = t.translation;
            if p.y > HALF_H + 20.0 || p.y < -HALF_H - 20.0 || p.x.abs() > HALF_W + 20.0 {
                continue;
            }
            hurt_enemy(&mut health, &mut flash, shielded, 45.0);
        }
        for i in 0..3 {
            effects::shockwave(
                &mut commands,
                &art,
                bomb.pos.extend(z::FX),
                40.0,
                1400.0 + i as f32 * 300.0,
                0.6 + i as f32 * 0.15,
                LinearRgba::new(2.0, 2.6, 3.5, 0.9),
            );
        }
        for _ in 0..12 {
            let p = bomb.pos + rng.in_disc(260.0);
            effects::explosion(&mut commands, &art, &mut rng, p.extend(z::ENEMY), 22.0, false);
        }
        effects::screen_flash(&mut commands, Color::WHITE, 0.75, 0.6);
        shake.add(0.8);
        sfx.write(PlaySfx::new(Sfx::Bomb));
        voice.write(Say(Line::BombAway));
    }
}

fn destroy_enemies(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
    mut shake: ResMut<Shake>,
    mut sfx: MessageWriter<PlaySfx>,
    q: Query<(Entity, &Enemy, &Health, &Transform)>,
) {
    for (e, enemy, health, t) in &q {
        if health.hp > 0.0 || enemy.kind == EnemyKind::Boss {
            continue;
        }
        let pos = t.translation.truncate();
        effects::explosion(&mut commands, &art, &mut rng, t.translation, enemy.size, !enemy.air);
        run.score += enemy.score;
        if enemy.counted {
            run.kills += 1;
        }
        if enemy.score >= 300 {
            effects::floating_text(&mut commands, t.translation, enemy.score.to_string(), Color::srgb(1.0, 0.95, 0.7), 22.0);
        }
        pickups::drop_stars(&mut commands, &art, &mut rng, pos, enemy.stars);
        if let Some(kind) = enemy.drop {
            pickups::drop_power(&mut commands, &art, pos, kind);
        }
        if enemy.survivor {
            pickups::spawn_survivor(&mut commands, &art, t.translation);
        }
        let big = enemy.size >= 40.0;
        sfx.write(PlaySfx::new(if big { Sfx::ExplodeBig } else { Sfx::ExplodeSmall }).pitch(rng.range(0.85, 1.15)));
        shake.add((enemy.size / 120.0).min(0.6));
        commands.entity(e).try_despawn();
    }
}

fn destroy_player(
    mut commands: Commands,
    art: Res<Art>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
    mut shake: ResMut<Shake>,
    mut sfx: MessageWriter<PlaySfx>,
    mut voice: MessageWriter<Say>,
    q: Query<(Entity, &Player, &Transform)>,
) {
    let Ok((e, player, t)) = q.single() else { return };
    if player.hp > 0.0 {
        return;
    }
    effects::explosion(&mut commands, &art, &mut rng, t.translation, 60.0, false);
    for _ in 0..4 {
        let p = t.translation + rng.in_disc(40.0).extend(0.0);
        effects::explosion(&mut commands, &art, &mut rng, p, 25.0, false);
    }
    effects::screen_flash(&mut commands, Color::srgb(1.0, 0.6, 0.3), 0.5, 0.5);
    shake.add(1.0);
    sfx.write(PlaySfx::new(Sfx::ExplodeBig));
    voice.write(Say(Line::MissionFailed));
    run.end(Outcome::Defeat, 2.8);
    commands.entity(e).try_despawn();
}
