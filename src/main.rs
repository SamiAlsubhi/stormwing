//! Stormwing: a vertical-scrolling arcade shoot-'em-up built with Bevy.
//!
//! Fly through three stages, collect stars, rescue survivors, earn medals and
//! spend your stars on hangar upgrades between missions. All art and audio is
//! generated procedurally at startup.

mod art;
mod audio;
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
mod model;
mod noise;
mod pickups;
mod player;
mod rng;
mod save;
mod state;
mod terrain;
mod textures;
mod touch;
mod voice;

use bevy::camera::ScalingMode;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster, NotShadowReceiver};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use common::{MainCamera, ARENA, HALF_H, HALF_W};

fn main() {
    let mut app = App::new();
    let default_plugins = DefaultPlugins.set(WindowPlugin {
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
        });
    // On the web the game plays sound through its own WebAudio engine, which
    // can be unlocked from a touch (see audio.rs), instead of Bevy's.
    #[cfg(target_arch = "wasm32")]
    let default_plugins = default_plugins.disable::<bevy::audio::AudioPlugin>();
    app.add_plugins(default_plugins)
        .add_plugins((
            common::CommonPlugin,
            state::StatePlugin,
            save::SavePlugin,
            art::ArtPlugin,
            audio::AudioFxPlugin,
            effects::EffectsPlugin,
            terrain::TerrainPlugin,
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
            voice::VoicePlugin,
        ))
        .insert_resource(DirectionalLightShadowMap {
            size: if cfg!(target_arch = "wasm32") { 1024 } else { 2048 },
        })
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.72, 0.8, 1.0),
            brightness: 420.0,
            ..default()
        })
        .add_systems(Startup, spawn_camera);
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, signal_ready);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(dev::DevPlugin);
    app.run();
}

/// Lets the web page hide its loading screen once frames are being drawn.
#[cfg(target_arch = "wasm32")]
fn signal_ready(mut frames: Local<u32>) {
    *frames += 1;
    if *frames == 6 {
        if let Some(window) = web_sys::window() {
            if let Ok(f) = js_sys::Reflect::get(&window, &"stormwingReady".into()) {
                if let Ok(f) = wasm_bindgen::JsCast::dyn_into::<js_sys::Function>(f) {
                    let _ = f.call0(&window);
                }
            }
        }
    }
}

fn spawn_camera(mut commands: Commands, art: Res<art::Art>) {
    // A top-down orthographic 3D camera that always shows the whole arena.
    commands.spawn((
        Camera3d::default(),
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: ARENA.x,
                min_height: ARENA.y,
            },
            near: 0.1,
            far: 2000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.16,
            ..Bloom::NATURAL
        },
        DebandDither::Enabled,
    ));

    // The sun, high in the upper left, so shadows fall down and to the right.
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            color: Color::srgb(1.0, 0.97, 0.9),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-230.0, 300.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.1,
            maximum_distance: 1600.0,
            first_cascade_far_bound: 1600.0,
            overlap_proportion: 0.0,
        }
        .build(),
    ));

    // Mask everything outside the play field when the window's aspect ratio
    // doesn't match, so enemies waiting off-screen stay hidden.
    let big = 4000.0;
    for (pos, size) in [
        (Vec2::new(-HALF_W - big / 2.0, 0.0), Vec2::new(big, big * 2.0)),
        (Vec2::new(HALF_W + big / 2.0, 0.0), Vec2::new(big, big * 2.0)),
        (Vec2::new(0.0, HALF_H + big / 2.0), Vec2::new(big * 2.0, big)),
        (Vec2::new(0.0, -HALF_H - big / 2.0), Vec2::new(big * 2.0, big)),
    ] {
        commands.spawn((
            Mesh3d(art.quad.clone()),
            MeshMaterial3d(art.mat_mask.clone()),
            Transform::from_translation(pos.extend(common::z::MASK)).with_scale(size.extend(1.0)),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}
