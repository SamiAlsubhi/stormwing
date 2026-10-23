//! Stormwing: a vertical-scrolling arcade shoot-'em-up built with Bevy.
//!
//! Fly through three stages, collect stars, rescue survivors, earn medals and
//! spend your stars on hangar upgrades between missions. All art and audio is
//! generated procedurally at startup.

mod art;
mod audio;
mod background;
mod boss;
mod combat;
mod common;
#[cfg(not(target_arch = "wasm32"))]
mod dev;
mod effects;
mod enemy;
mod hud;
mod level;
mod menu;
mod pickups;
mod player;
mod rng;
mod save;
mod shapes;
mod state;
mod textures;
mod touch;

use bevy::camera::ScalingMode;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use common::{MainCamera, ARENA, HALF_H, HALF_W};

fn main() {
    let mut app = App::new();
    app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Stormwing".into(),
                resolution: (600u32, 900u32).into(),
                // Web: draw into the page's canvas, fill its container and
                // keep the browser from scrolling or zooming on touch.
                canvas: Some("#stormwing".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            common::CommonPlugin,
            state::StatePlugin,
            save::SavePlugin,
            art::ArtPlugin,
            audio::AudioFxPlugin,
            effects::EffectsPlugin,
            background::BackgroundPlugin,
            player::PlayerPlugin,
        ))
        .add_plugins((
            enemy::EnemyPlugin,
            boss::BossPlugin,
            combat::CombatPlugin,
            pickups::PickupPlugin,
            level::LevelPlugin,
            hud::HudPlugin,
            menu::MenuPlugin,
            touch::TouchPlugin,
        ))
        .add_systems(Startup, spawn_camera);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(dev::DevPlugin);
    app.run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: ARENA.x,
                min_height: ARENA.y,
            },
            ..OrthographicProjection::default_2d()
        }),
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.2,
            ..Bloom::NATURAL
        },
        DebandDither::Enabled,
    ));

    // Mask everything outside the play field when the window's aspect ratio
    // doesn't match, so enemies waiting off-screen stay hidden.
    let mask = Color::srgb(0.01, 0.02, 0.04);
    let big = 4000.0;
    for (pos, size) in [
        (Vec2::new(-HALF_W - big / 2.0, 0.0), Vec2::new(big, big * 2.0)),
        (Vec2::new(HALF_W + big / 2.0, 0.0), Vec2::new(big, big * 2.0)),
        (Vec2::new(0.0, HALF_H + big / 2.0), Vec2::new(big * 2.0, big)),
        (Vec2::new(0.0, -HALF_H - big / 2.0), Vec2::new(big * 2.0, big)),
    ] {
        commands.spawn((
            Sprite::from_color(mask, size),
            Transform::from_translation(pos.extend(common::z::MASK)),
        ));
    }
}
