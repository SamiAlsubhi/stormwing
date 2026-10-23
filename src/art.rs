//! All of the game's 3D models and materials, generated at startup.

use crate::model::{hdr, ngon, pts, rgb, shade, star_outline, Model};
use crate::textures;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

/// A model split by surface: painted body, glass, and self-lit parts.
#[derive(Clone)]
pub struct ModelSet {
    pub paint: Handle<Mesh>,
    pub glass: Option<Handle<Mesh>>,
    pub glow: Option<Handle<Mesh>>,
}

#[derive(Default)]
struct Parts {
    paint: Model,
    glass: Model,
    glow: Model,
}

#[derive(Resource)]
pub struct Art {
    pub mat_paint: Handle<StandardMaterial>,
    pub mat_flash: Handle<StandardMaterial>,
    pub mat_glass: Handle<StandardMaterial>,
    pub mat_glow: Handle<StandardMaterial>,
    pub mat_terrain: Handle<StandardMaterial>,
    pub mat_dunes: Handle<StandardMaterial>,
    pub mat_water: [Handle<StandardMaterial>; 3],
    pub mat_floor: [Handle<StandardMaterial>; 3],
    pub mat_star: Handle<StandardMaterial>,
    pub mat_icon: Handle<StandardMaterial>,

    // Self-lit effect materials.
    pub mat_engine: Handle<StandardMaterial>,
    pub mat_tracer: Handle<StandardMaterial>,
    pub mat_tracer_power: Handle<StandardMaterial>,
    pub mat_orb: Handle<StandardMaterial>,
    pub mat_orb_big: Handle<StandardMaterial>,
    pub mat_shield: Handle<StandardMaterial>,
    pub mat_shield_dim: Handle<StandardMaterial>,
    pub mat_boss_shield: Handle<StandardMaterial>,
    pub mat_beacon: Handle<StandardMaterial>,
    pub mat_bar_bg: Handle<StandardMaterial>,
    pub mat_bar_fill: Handle<StandardMaterial>,
    pub mat_star_halo: Handle<StandardMaterial>,
    pub mat_power_glow: [Handle<StandardMaterial>; 4],
    pub mat_power_disc: [Handle<StandardMaterial>; 4],
    pub mat_power_ring: [Handle<StandardMaterial>; 4],
    pub mat_cloud: Vec<Handle<StandardMaterial>>,
    pub mat_cloud_high: Vec<Handle<StandardMaterial>>,
    pub mat_cloud_shadow: Vec<Handle<StandardMaterial>>,
    pub mat_scorch: Handle<StandardMaterial>,
    /// Surf along coastlines (white, alpha from vertex colours).
    pub mat_foam: Handle<StandardMaterial>,
    /// Whitecaps drifting over the open sea.
    pub mat_whitecaps: [Handle<StandardMaterial>; 3],
    /// Surf rolling onto beaches, at increasing opacity.
    pub mat_surf: Vec<Handle<StandardMaterial>>,
    /// Flashing lighthouse lamp.
    pub mat_lamp: Handle<StandardMaterial>,
    pub mat_mask: Handle<StandardMaterial>,

    /// 1x1 quad facing the camera, scaled per use (particles, bullets, clouds).
    pub quad: Handle<Mesh>,

    pub dart: ModelSet,
    pub swooper: ModelSet,
    pub chaser: ModelSet,
    pub gunship: ModelSet,
    pub bomber: ModelSet,
    pub turret_base: ModelSet,
    pub turret_barrel: ModelSet,
    pub tank_body: ModelSet,
    pub tank_turret: ModelSet,
    pub boat: ModelSet,
    pub hut: ModelSet,
    pub depot: ModelSet,
    pub bosses: [ModelSet; 3],
    pub boss_turret: ModelSet,
    pub boss_barrel: ModelSet,
    pub missile: ModelSet,
    pub star: Handle<Mesh>,
    pub big_star: Handle<Mesh>,
    pub icons: [Handle<Mesh>; 4],
    pub survivor: ModelSet,
    pub lighthouse: ModelSet,
    /// Whale, in parts so it can swim: body, tail (pivots at the tail stock)
    /// and the two flippers.
    pub whale_body: Handle<Mesh>,
    pub whale_tail: Handle<Mesh>,
    pub whale_fins: [Handle<Mesh>; 2],
    /// A few shapes of drifting sea ice.
    pub ice_chunks: Vec<Handle<Mesh>>,
    /// Letters of the 3D title, each with its x offset.
    pub title_letters: Vec<(Handle<Mesh>, f32)>,
    /// Twin sweeping light beams for lighthouses.
    pub beam: Handle<Mesh>,
    pub mat_beam: Handle<StandardMaterial>,

    pub tex_glow: Handle<Image>,
    pub tex_smoke: Handle<Image>,
    pub tex_ring: Handle<Image>,
    pub tex_streak: Handle<Image>,
    pub tex_star_icon: Handle<Image>,
    pub tex_medal: Handle<Image>,
}

pub struct ArtPlugin;

impl Plugin for ArtPlugin {
    fn build(&self, _app: &mut App) {}

    // Built in `finish` (after every plugin has registered its asset types) so
    // the art exists before the first state transition, which runs before
    // the startup schedules.
    fn finish(&self, app: &mut App) {
        let world = app.world_mut();
        let art = world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                let mut images = world.resource_mut::<Assets<Image>>();
                build_art(&mut meshes, &mut materials, &mut images)
            })
        });
        world.insert_resource(art);
    }
}

/// Self-lit material for effects: unaffected by lighting, never shadowed.
pub fn fx_material(texture: Option<Handle<Image>>, color: Color, additive: bool) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        base_color_texture: texture,
        unlit: true,
        alpha_mode: if additive { AlphaMode::Add } else { AlphaMode::Blend },
        cull_mode: None,
        double_sided: true,
        fog_enabled: false,
        ..default()
    }
}

fn build_art(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>) -> Art {
    let tex_glow = images.add(textures::soft_glow());
    let tex_smoke = images.add(textures::smoke());
    let tex_orb = images.add(textures::orb());
    let tex_disc = images.add(textures::disc());
    let tex_ring = images.add(textures::ring());
    let tex_streak = images.add(textures::streak());
    let water_normal = images.add(textures::water_normal());
    let sand_normal = images.add(textures::sand_normal());
    let whitecaps = images.add(textures::whitecaps());
    let clouds: Vec<Handle<Image>> = (0..4).map(|i| images.add(textures::cloud(11 + i * 31))).collect();

    let mut set = |p: Parts| ModelSet {
        paint: meshes.add(p.paint.build()),
        glass: (p.glass.vertex_count() > 0).then(|| meshes.add(p.glass.build())),
        glow: (p.glow.vertex_count() > 0).then(|| meshes.add(p.glow.build())),
    };
    let dart = set(dart());
    let swooper = set(swooper());
    let chaser = set(chaser());
    let gunship = set(gunship());
    let bomber = set(bomber());
    let turret_base = set(turret_base());
    let turret_barrel = set(turret_barrel());
    let tank_body = set(tank_body());
    let tank_turret = set(tank_turret());
    let boat = set(boat());
    let hut = set(hut());
    let depot = set(depot());
    let bosses = [set(boss_wing()), set(boss_crawler()), set(boss_citadel())];
    let boss_turret = set(boss_turret());
    let boss_barrel = set(boss_barrel());
    let missile = set(missile());
    let survivor = set(survivor());
    let lighthouse = set(lighthouse());
    let (whale_body, whale_tail, whale_fin_r, whale_fin_l) = whale_parts();
    let whale_body = meshes.add(whale_body.build());
    let whale_tail = meshes.add(whale_tail.build());
    let whale_fins = [meshes.add(whale_fin_r.build()), meshes.add(whale_fin_l.build())];

    let mut mesh = |m: Model| meshes.add(m.build());
    let star = mesh(star_model(1.0));
    let big_star = mesh(star_model(1.6));
    let icons = [mesh(icon_weapon()), mesh(icon_shield()), mesh(icon_repair()), mesh(icon_bomb())];
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let beam = meshes.add(beam_model().build());
    let ice_chunks = (0..8).map(|i| meshes.add(ice_chunk(i).build())).collect();
    let title_letters = title_models()
        .into_iter()
        .map(|(m, x)| (meshes.add(m.build()), x))
        .collect();

    let mut fx = |tex: &Handle<Image>, color: Color, additive: bool| {
        materials.add(fx_material(Some(tex.clone()), color, additive))
    };
    let power_colors = [
        hdr(3.2, 1.6, 0.3),
        hdr(0.5, 1.8, 3.4),
        hdr(0.6, 3.0, 0.8),
        hdr(3.2, 0.5, 0.5),
    ];
    let mat_engine = fx(&tex_glow, hdr(3.6, 1.7, 0.6), true);
    let mat_tracer = fx(&tex_streak, hdr(3.6, 2.9, 1.1), true);
    let mat_tracer_power = fx(&tex_streak, hdr(1.6, 3.0, 3.6), true);
    let mat_orb = fx(&tex_orb, hdr(3.4, 0.5, 1.5), false);
    let mat_orb_big = fx(&tex_orb, hdr(3.6, 1.3, 0.25), false);
    let mat_shield = fx(&tex_ring, Color::linear_rgba(0.6, 1.8, 3.0, 0.9), true);
    let mat_shield_dim = fx(&tex_ring, Color::linear_rgba(0.6, 1.8, 3.0, 0.25), true);
    let mat_boss_shield = fx(&tex_ring, Color::linear_rgba(0.6, 1.6, 3.2, 0.8), true);
    let mat_beacon = fx(&tex_ring, Color::linear_rgba(2.5, 2.2, 0.6, 0.8), true);
    let mat_star_halo = fx(&tex_glow, Color::linear_rgba(2.6, 1.7, 0.35, 0.7), true);
    let mat_power_glow = power_colors.map(|c| fx(&tex_glow, c, true));
    let mat_power_disc = power_colors.map(|c| {
        let l = c.to_linear();
        fx(&tex_disc, Color::linear_rgba(l.red * 0.3, l.green * 0.3, l.blue * 0.3, 1.0), false)
    });
    let mat_power_ring = power_colors.map(|c| fx(&tex_ring, c, true));
    let mat_cloud = clouds.iter().map(|t| fx(t, Color::srgba(1.0, 1.0, 1.0, 0.85), false)).collect();
    let mat_cloud_high = clouds.iter().map(|t| fx(t, Color::srgba(1.0, 1.0, 1.0, 0.4), false)).collect();
    let mat_cloud_shadow = clouds.iter().map(|t| fx(t, Color::srgba(0.0, 0.02, 0.06, 0.32), false)).collect();
    let mat_scorch = fx(&tex_smoke, Color::srgba(0.04, 0.03, 0.02, 0.65), false);

    let solid = |color: Color| fx_material(None, color, false);
    // Lit exactly like the terrain, so reef shelves blend into it.
    let seabed = |color: Color| StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.93,
        reflectance: 0.2,
        ..default()
    };
    let water = |color: Color| StandardMaterial {
        base_color: color,
        normal_map_texture: Some(water_normal.clone()),
        perceptual_roughness: 0.17,
        reflectance: 0.45,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };

    Art {
        mat_paint: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.42,
            metallic: 0.3,
            reflectance: 0.5,
            ..default()
        }),
        mat_flash: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::rgb(2.2, 2.2, 2.4),
            perceptual_roughness: 0.42,
            metallic: 0.3,
            ..default()
        }),
        mat_glass: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.05,
            metallic: 0.0,
            reflectance: 1.0,
            ..default()
        }),
        mat_glow: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        mat_terrain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.93,
            reflectance: 0.2,
            ..default()
        }),
        mat_dunes: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            normal_map_texture: Some(sand_normal),
            perceptual_roughness: 0.96,
            reflectance: 0.15,
            ..default()
        }),
        mat_water: [
            materials.add(water(Color::srgba(0.02, 0.3, 0.44, 0.58))),
            materials.add(water(Color::srgba(0.02, 0.3, 0.44, 0.58))),
            materials.add(water(Color::srgba(0.03, 0.12, 0.2, 0.7))),
        ],
        mat_floor: [
            materials.add(seabed(Color::srgb(0.012, 0.06, 0.12))),
            materials.add(seabed(Color::srgb(0.012, 0.06, 0.12))),
            materials.add(seabed(Color::srgb(0.006, 0.025, 0.05))),
        ],
        mat_star: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.25),
            emissive: LinearRgba::rgb(2.6, 1.6, 0.25),
            perceptual_roughness: 0.25,
            metallic: 0.6,
            reflectance: 0.8,
            ..default()
        }),
        mat_icon: materials.add(solid(Color::WHITE)),

        mat_engine,
        mat_tracer,
        mat_tracer_power,
        mat_orb,
        mat_orb_big,
        mat_shield,
        mat_shield_dim,
        mat_boss_shield,
        mat_beacon,
        mat_bar_bg: materials.add(solid(Color::srgba(0.0, 0.0, 0.0, 0.6))),
        mat_bar_fill: materials.add(solid(hdr(0.6, 2.8, 0.8))),
        mat_star_halo,
        mat_power_glow,
        mat_power_disc,
        mat_power_ring,
        mat_cloud,
        mat_cloud_high,
        mat_cloud_shadow,
        mat_scorch,
        mat_foam: materials.add(solid(Color::WHITE)),
        mat_whitecaps: [
            materials.add(fx_material(Some(whitecaps.clone()), Color::srgba(1.0, 1.0, 1.0, 0.6), false)),
            materials.add(fx_material(Some(whitecaps.clone()), Color::srgba(1.0, 1.0, 1.0, 0.6), false)),
            materials.add(fx_material(Some(whitecaps.clone()), Color::srgba(0.88, 0.94, 1.0, 0.5), false)),
        ],
        mat_surf: (1..=10).map(|i| materials.add(solid(Color::srgba(1.0, 1.0, 1.0, i as f32 / 10.0)))).collect(),
        mat_lamp: materials.add(fx_material(Some(tex_glow.clone()), hdr(5.0, 4.2, 2.6), true)),
        mat_mask: materials.add(solid(Color::srgb(0.01, 0.02, 0.04))),

        quad,
        dart,
        swooper,
        chaser,
        gunship,
        bomber,
        turret_base,
        turret_barrel,
        tank_body,
        tank_turret,
        boat,
        hut,
        depot,
        bosses,
        boss_turret,
        boss_barrel,
        missile,
        star,
        big_star,
        icons,
        survivor,
        lighthouse,
        whale_body,
        whale_tail,
        whale_fins,
        ice_chunks,
        title_letters,
        beam,
        mat_beam: materials.add(fx_material(None, Color::WHITE, true)),

        tex_glow,
        tex_smoke,
        tex_ring,
        tex_streak,
        tex_star_icon: images.add(textures::star_icon()),
        tex_medal: images.add(textures::medal_icon()),
    }
}

// ---------------------------------------------------------------------------
// Aircraft. Nose along +Y, top towards +Z.
// ---------------------------------------------------------------------------

const GLASS_BLUE: (f32, f32, f32) = (0.1, 0.22, 0.36);
const GLASS_AMBER: (f32, f32, f32) = (0.4, 0.24, 0.06);

fn glass(c: (f32, f32, f32)) -> Color {
    rgb(c.0, c.1, c.2)
}

/// Where each hangar upgrade lives on the player's jet (model space), used
/// by the hangar's callouts. Order matches `save::UPGRADES`.
pub const UPGRADE_ANCHORS: [Vec3; 5] = [
    Vec3::new(0.0, 47.0, 0.0),   // Main Cannon: nose guns
    Vec3::new(9.0, 20.0, 4.0),   // Fire Rate: ammo feed behind the canopy
    Vec3::new(30.0, -7.0, 1.5),  // Hull Plating: wing armour
    Vec3::new(24.0, -3.0, -4.0), // Seeker Missiles: under-wing pylons
    Vec3::new(0.0, -6.0, 9.0),   // Star Magnet: dorsal collector ring
];

/// Builds the player's jet with its hangar upgrades visible on it.
pub fn player_set(meshes: &mut Assets<Mesh>, levels: [u8; 5]) -> ModelSet {
    let p = player_with(levels);
    ModelSet {
        paint: meshes.add(p.paint.build()),
        glass: Some(meshes.add(p.glass.build())),
        glow: Some(meshes.add(p.glow.build())),
    }
}

fn player_with(levels: [u8; 5]) -> Parts {
    let [cannon, rate, hull_lv, missiles, magnet] = levels;
    let mut p = player_base();
    let gun = rgb(0.18, 0.19, 0.22);
    let gold = rgb(0.95, 0.72, 0.25);
    // Main cannon: one extra barrel pair per two levels.
    let barrels = 1 + (cannon as usize + 1) / 2;
    for i in 0..barrels {
        let x = 3.0 + i as f32 * 2.6;
        p.paint.symmetric(|m| {
            m.cylinder(Vec3::new(x, 30.0, -2.6), Vec3::new(x, 44.0 + cannon as f32, -2.6), 0.9, 6, gun, true);
        });
    }
    // Fire rate: ammo feed drums, glowing when upgraded.
    for i in 0..rate as usize {
        p.paint.symmetric(|m| {
            m.cylinder(Vec3::new(8.5, 16.0 + i as f32 * 3.2, 3.6), Vec3::new(8.5, 18.0 + i as f32 * 3.2, 3.6), 2.2, 8, rgb(0.3, 0.32, 0.36), true);
        });
        p.glow.symmetric(|m| {
            m.ellipsoid(Vec3::new(8.5, 17.0 + i as f32 * 3.2, 5.9), Vec3::new(1.0, 0.6, 0.3), hdr(3.2, 1.6, 0.4), false);
        });
    }
    // Hull plating: armour panels on the wings, gold-trimmed at max level.
    for i in 0..hull_lv as usize {
        let y = 2.0 - i as f32 * 4.2;
        let color = if hull_lv >= 5 { gold } else { rgb(0.5, 0.54, 0.6) };
        p.paint.symmetric(|m| {
            m.wing(&pts(&[(14.0, y), (30.0, y - 9.0), (30.0, y - 12.0), (14.0, y - 3.0)]), 1.2, 1.2, color);
        });
    }
    // Seeker missiles: extra rails under the wings.
    for i in 0..missiles as usize {
        let x = 20.0 + i as f32 * 5.0;
        p.paint.symmetric(|m| {
            m.cylinder(Vec3::new(x, 0.0, -4.2), Vec3::new(x, -12.0, -4.2), 1.4, 8, rgb(0.92, 0.92, 0.95), true);
            m.cone(Vec3::new(x, 0.0, -4.2), Vec3::new(x, 4.0, -4.2), 1.4, 8, rgb(0.9, 0.25, 0.15));
            m.cuboid(Vec3::new(x, -5.0, -2.6), Vec3::new(1.0, 8.0, 1.6), gun);
        });
    }
    // Star magnet: a dorsal collector ring that grows with level.
    if magnet > 0 {
        let r = 3.0 + magnet as f32 * 1.4;
        p.paint.cylinder(Vec3::new(0.0, -6.0, 6.5), Vec3::new(0.0, -6.0, 8.0), 1.2, 6, gun, true);
        p.glow.frustum(Vec3::new(0.0, -6.0, 8.0), Vec3::new(0.0, -6.0, 9.2), r, r * 0.9, 16, hdr(2.6, 1.9, 0.5), true);
    }
    p
}

fn player_base() -> Parts {
    let hull = rgb(0.8, 0.83, 0.87);
    let belly = rgb(0.56, 0.6, 0.66);
    let dark = rgb(0.2, 0.22, 0.26);
    let accent = rgb(0.95, 0.45, 0.1);
    let mut p = Parts::default();
    p.paint.loft(
        &[
            (-44.0, 4.0, 3.5, 0.0),
            (-38.0, 8.5, 5.5, 0.0),
            (-24.0, 10.0, 6.5, 0.0),
            (0.0, 10.5, 7.0, 0.5),
            (16.0, 8.5, 6.5, 1.0),
            (28.0, 6.0, 5.0, 0.5),
            (38.0, 3.6, 3.6, 0.0),
            (45.0, 1.4, 1.6, -0.4),
            (49.0, 0.05, 0.05, -0.6),
        ],
        16,
        move |n, y| {
            if y > 39.0 {
                accent
            } else if n.z < -0.25 {
                belly
            } else {
                hull
            }
        },
    );
    p.paint.symmetric(|m| {
        m.wing(&pts(&[(8.0, 14.0), (44.0, -9.0), (45.0, -19.0), (8.0, -15.0)]), -0.5, 2.6, hull);
        m.wing(&pts(&[(33.0, -2.5), (44.0, -9.0), (45.0, -19.0), (35.5, -16.6)]), -0.5, 3.0, accent);
        m.wing(&pts(&[(7.0, 25.0), (17.0, 17.0), (17.0, 14.0), (7.0, 16.0)]), 1.5, 1.6, hull);
        m.wing(&pts(&[(6.0, -30.0), (21.0, -38.0), (22.0, -44.0), (6.0, -41.0)]), 0.0, 1.8, hull);
        m.fin(&pts(&[(-26.0, 3.0), (-40.0, 21.0), (-45.0, 21.0), (-42.0, 3.0)]), 6.0, 1.6, 0.3, belly);
        m.cuboid(Vec3::new(11.5, 3.0, -1.0), Vec3::new(4.0, 16.0, 6.0), dark);
        m.cylinder(Vec3::new(4.6, -40.0, 0.0), Vec3::new(4.6, -47.0, 0.0), 3.4, 12, dark, true);
    });
    p.glass.ellipsoid(Vec3::new(0.0, 20.0, 5.6), Vec3::new(4.3, 10.5, 4.6), glass(GLASS_BLUE), true);
    p.glow.symmetric(|m| {
        m.ellipsoid(Vec3::new(4.6, -47.3, 0.0), Vec3::new(2.8, 0.7, 2.8), hdr(4.0, 1.6, 0.5), false);
    });
    p.glow.ellipsoid(Vec3::new(42.0, -15.0, 0.6), Vec3::splat(1.6), hdr(0.3, 3.0, 0.5), false);
    p.glow.ellipsoid(Vec3::new(-42.0, -15.0, 0.6), Vec3::splat(1.6), hdr(3.2, 0.3, 0.25), false);
    p
}

fn dart() -> Parts {
    let body = rgb(0.36, 0.37, 0.43);
    let red = rgb(0.72, 0.15, 0.12);
    let mut p = Parts::default();
    p.paint.loft(
        &[(-24.0, 3.0, 2.6, 0.0), (-18.0, 5.5, 4.0, 0.0), (0.0, 6.0, 4.6, 0.0), (14.0, 4.5, 4.0, 0.3), (22.0, 2.0, 2.4, 0.0), (27.0, 0.05, 0.05, -0.3)],
        12,
        move |n, _| if n.z < -0.3 { shade(body, 0.7) } else { body },
    );
    p.paint.symmetric(|m| {
        m.wing(&pts(&[(3.0, 10.0), (23.0, -12.0), (23.0, -18.0), (4.0, -15.0)]), 0.0, 2.2, red);
        m.wing(&pts(&[(17.0, -8.0), (23.0, -12.0), (23.0, -18.0), (17.0, -16.5)]), 0.0, 2.5, rgb(0.92, 0.88, 0.82));
    });
    p.paint.fin(&pts(&[(-12.0, 3.0), (-22.0, 14.0), (-25.0, 14.0), (-23.0, 3.0)]), 0.0, 1.4, 0.0, shade(red, 0.8));
    p.glass.ellipsoid(Vec3::new(0.0, 7.0, 3.6), Vec3::new(2.6, 5.5, 2.6), glass(GLASS_AMBER), true);
    p.glow.ellipsoid(Vec3::new(0.0, -24.5, 0.0), Vec3::new(2.4, 0.6, 2.2), hdr(4.0, 1.6, 0.4), false);
    p
}

fn swooper() -> Parts {
    let wing = rgb(0.17, 0.48, 0.4);
    let body = rgb(0.3, 0.32, 0.37);
    let mut p = Parts::default();
    p.paint.loft(
        &[(-19.0, 3.0, 2.5, 0.0), (-12.0, 5.0, 4.0, 0.0), (4.0, 5.5, 4.6, 0.0), (14.0, 3.5, 3.4, 0.0), (19.0, 0.05, 0.05, 0.0)],
        12,
        move |n, _| if n.z < -0.3 { shade(body, 0.7) } else { body },
    );
    p.paint.symmetric(|m| {
        m.wing(
            &pts(&[(3.0, 12.0), (14.0, 9.0), (31.0, -1.0), (30.0, -15.0), (19.0, -8.0), (10.0, -17.0), (3.0, -11.0)]),
            0.0,
            2.4,
            wing,
        );
        m.wing(&pts(&[(20.0, 3.0), (31.0, -1.0), (30.0, -15.0), (24.0, -10.0)]), 0.0, 2.7, shade(wing, 0.6));
    });
    p.glass.ellipsoid(Vec3::new(0.0, 5.0, 3.4), Vec3::new(2.5, 4.5, 2.5), glass(GLASS_AMBER), true);
    p.glow.ellipsoid(Vec3::new(0.0, 12.0, 2.5), Vec3::splat(1.8), hdr(0.4, 3.0, 1.4), false);
    p.glow.ellipsoid(Vec3::new(0.0, -19.4, 0.0), Vec3::new(2.0, 0.5, 2.0), hdr(4.0, 1.6, 0.4), false);
    p
}

fn chaser() -> Parts {
    let mut p = Parts::default();
    for i in 0..4 {
        let d = Vec2::from_angle(FRAC_PI_4 + i as f32 * FRAC_PI_2).extend(0.0);
        p.paint.cone(d * 7.0, d * 21.0, 4.5, 8, rgb(0.26, 0.26, 0.3));
    }
    p.paint.ellipsoid(Vec3::ZERO, Vec3::new(10.5, 10.5, 7.0), rgb(0.9, 0.42, 0.1), false);
    p.glow.ellipsoid(Vec3::new(0.0, 0.0, 5.0), Vec3::new(5.0, 5.0, 3.2), hdr(4.5, 1.3, 0.2), false);
    p
}

fn gunship() -> Parts {
    let olive = rgb(0.47, 0.52, 0.34);
    let olive_dark = rgb(0.31, 0.36, 0.22);
    let pod = rgb(0.23, 0.25, 0.21);
    let mut p = Parts::default();
    p.paint.loft(
        &[(-42.0, 5.0, 4.0, 0.0), (-36.0, 9.0, 7.0, 0.0), (-10.0, 14.0, 9.0, 0.0), (10.0, 14.0, 9.0, 0.0), (28.0, 10.0, 8.0, 1.0), (38.0, 6.0, 5.0, 0.0), (44.0, 0.05, 0.05, 0.0)],
        14,
        move |n, _| if n.z < -0.3 { shade(olive, 0.65) } else { olive },
    );
    p.paint.symmetric(|m| {
        m.wing(&pts(&[(10.0, 20.0), (56.0, 1.0), (56.0, -13.0), (10.0, -17.0)]), 0.0, 3.2, olive_dark);
        m.wing(&pts(&[(43.0, -1.0), (50.0, -2.5), (50.0, -11.5), (43.0, -11.0)]), 0.0, 3.5, rgb(0.86, 0.74, 0.2));
        m.frustum(Vec3::new(30.0, 17.0, 0.0), Vec3::new(30.0, -22.0, 0.0), 6.0, 7.5, 12, pod, true);
        m.wing(&pts(&[(5.0, -30.0), (20.0, -44.0), (20.0, -49.0), (5.0, -41.0)]), 0.0, 2.0, olive_dark);
        m.fin(&pts(&[(-30.0, 4.0), (-42.0, 18.0), (-48.0, 18.0), (-45.0, 4.0)]), 6.0, 1.8, 0.3, olive);
    });
    p.paint.ellipsoid(Vec3::new(0.0, -8.0, 8.0), Vec3::new(7.0, 7.0, 5.0), pod, true);
    p.paint.symmetric(|m| {
        m.cylinder(Vec3::new(2.5, -8.0, 10.0), Vec3::new(2.5, 6.0, 10.0), 1.2, 6, rgb(0.15, 0.15, 0.15), true);
    });
    p.glass.ellipsoid(Vec3::new(0.0, 25.0, 6.8), Vec3::new(4.6, 8.5, 4.2), glass(GLASS_AMBER), true);
    p.glow.symmetric(|m| {
        m.ellipsoid(Vec3::new(30.0, -22.6, 0.0), Vec3::new(5.0, 0.8, 5.0), hdr(4.0, 1.5, 0.4), false);
    });
    p
}

fn bomber() -> Parts {
    let steel = rgb(0.42, 0.46, 0.54);
    let light = rgb(0.6, 0.63, 0.7);
    let dark = rgb(0.2, 0.22, 0.27);
    let mut p = Parts::default();
    p.paint.loft(
        &[(-60.0, 4.0, 4.0, 0.0), (-52.0, 9.0, 8.0, 0.0), (-30.0, 15.0, 11.0, 0.0), (20.0, 16.0, 12.0, 0.0), (44.0, 12.0, 10.0, 1.0), (56.0, 6.0, 6.0, 0.0), (61.0, 0.05, 0.05, 0.0)],
        16,
        move |n, _| if n.z < -0.3 { shade(light, 0.65) } else { light },
    );
    p.paint.symmetric(|m| {
        m.wing(&pts(&[(14.0, 26.0), (96.0, -2.0), (96.0, -16.0), (14.0, -14.0)]), 0.0, 4.0, steel);
        m.cylinder(Vec3::new(80.0, -8.0, 2.0), Vec3::new(80.0, -8.0, 2.6), 6.5, 16, rgb(0.8, 0.18, 0.14), true);
        m.cylinder(Vec3::new(80.0, -8.0, 2.6), Vec3::new(80.0, -8.0, 2.8), 2.6, 12, rgb(0.95, 0.95, 0.95), true);
        m.frustum(Vec3::new(32.0, 22.0, -2.0), Vec3::new(32.0, -12.0, -2.0), 6.0, 7.5, 12, dark, true);
        m.frustum(Vec3::new(63.0, 14.0, -2.0), Vec3::new(63.0, -15.0, -2.0), 5.0, 6.5, 12, dark, true);
        m.wing(&pts(&[(8.0, -40.0), (32.0, -55.0), (32.0, -64.0), (8.0, -58.0)]), 0.0, 2.4, steel);
        m.fin(&pts(&[(-46.0, 1.0), (-58.0, 22.0), (-64.0, 22.0), (-62.0, 1.0)]), 30.0, 2.0, 0.1, steel);
    });
    p.glass.ellipsoid(Vec3::new(0.0, 47.0, 6.5), Vec3::new(7.0, 9.0, 5.0), glass(GLASS_BLUE), true);
    p.glow.symmetric(|m| {
        m.ellipsoid(Vec3::new(32.0, -12.6, -2.0), Vec3::new(5.5, 0.8, 5.5), hdr(3.6, 1.4, 0.4), false);
        m.ellipsoid(Vec3::new(63.0, -15.6, -2.0), Vec3::new(4.8, 0.8, 4.8), hdr(3.6, 1.4, 0.4), false);
    });
    p
}

fn missile() -> Parts {
    let mut p = Parts::default();
    p.paint.cylinder(Vec3::new(0.0, -10.0, 0.0), Vec3::new(0.0, 7.0, 0.0), 2.2, 8, rgb(0.92, 0.92, 0.95), true);
    p.paint.cone(Vec3::new(0.0, 7.0, 0.0), Vec3::new(0.0, 13.0, 0.0), 2.2, 8, rgb(0.9, 0.25, 0.15));
    p.paint.symmetric(|m| {
        m.wing(&pts(&[(1.5, -5.0), (6.0, -10.0), (6.0, -12.0), (1.5, -10.0)]), 0.0, 0.8, rgb(0.5, 0.5, 0.55));
    });
    p.glow.ellipsoid(Vec3::new(0.0, -10.6, 0.0), Vec3::new(1.8, 0.6, 1.8), hdr(4.0, 2.0, 0.6), false);
    p
}

// ---------------------------------------------------------------------------
// Ground targets. Bases extend below z = 0 so they sit on uneven terrain.
// ---------------------------------------------------------------------------

fn turret_base() -> Parts {
    let concrete = rgb(0.55, 0.55, 0.52);
    let mut p = Parts::default();
    p.paint.extrude(&ngon(Vec2::ZERO, 22.0, 8, 0.39), -10.0, 5.0, concrete, shade(concrete, 0.8));
    p.paint.extrude(&ngon(Vec2::ZERO, 15.0, 20, 0.0), 5.0, 8.0, rgb(0.36, 0.37, 0.4), rgb(0.28, 0.29, 0.31));
    for i in 0..8 {
        let d = Vec2::from_angle(0.39 + i as f32 * TAU / 8.0) * 18.5;
        p.paint.cuboid(d.extend(5.5), Vec3::splat(2.2), rgb(0.3, 0.3, 0.32));
    }
    p
}

fn turret_barrel() -> Parts {
    let gun = rgb(0.4, 0.42, 0.45);
    let mut p = Parts::default();
    p.paint.ellipsoid(Vec3::new(0.0, 0.0, 8.0), Vec3::new(11.0, 11.0, 7.0), gun, true);
    p.paint.cylinder(Vec3::new(0.0, 4.0, 11.0), Vec3::new(0.0, 25.0, 11.0), 2.8, 8, rgb(0.18, 0.18, 0.2), true);
    p.paint.cylinder(Vec3::new(0.0, 23.0, 11.0), Vec3::new(0.0, 29.0, 11.0), 3.8, 8, rgb(0.12, 0.12, 0.14), true);
    p.glow.ellipsoid(Vec3::new(0.0, -5.0, 13.5), Vec3::splat(1.8), hdr(3.0, 0.5, 0.3), false);
    p
}

fn tank_body() -> Parts {
    let olive = rgb(0.43, 0.46, 0.3);
    let tread = rgb(0.16, 0.16, 0.15);
    let mut p = Parts::default();
    p.paint.symmetric(|m| {
        m.cuboid(Vec3::new(17.0, 0.0, 1.0), Vec3::new(10.0, 50.0, 10.0), tread);
        for i in 0..9 {
            let y = -22.0 + i as f32 * 5.5;
            m.cuboid(Vec3::new(17.0, y, 6.2), Vec3::new(10.4, 1.8, 0.6), rgb(0.3, 0.3, 0.28));
        }
    });
    p.paint.cuboid(Vec3::new(0.0, 0.0, 4.0), Vec3::new(26.0, 44.0, 10.0), olive);
    p.paint.cuboid(Vec3::new(0.0, -17.0, 9.5), Vec3::new(22.0, 8.0, 2.0), shade(olive, 0.75));
    p
}

fn tank_turret() -> Parts {
    let olive = rgb(0.52, 0.56, 0.38);
    let mut p = Parts::default();
    p.paint.ellipsoid(Vec3::new(0.0, 0.0, 9.0), Vec3::new(11.0, 12.0, 6.0), olive, true);
    p.paint.cylinder(Vec3::new(0.0, 6.0, 12.0), Vec3::new(0.0, 30.0, 12.0), 2.3, 8, rgb(0.2, 0.22, 0.15), true);
    p.paint.cylinder(Vec3::new(-4.0, -4.0, 14.0), Vec3::new(-4.0, -4.0, 16.0), 3.0, 8, shade(olive, 0.8), true);
    p
}

fn boat() -> Parts {
    let hull = rgb(0.45, 0.47, 0.51);
    let deck = rgb(0.6, 0.62, 0.66);
    let mut p = Parts::default();
    let outline = pts(&[(0.0, 56.0), (10.0, 36.0), (14.0, 10.0), (14.0, -34.0), (11.0, -44.0), (0.0, -46.0), (-11.0, -44.0), (-14.0, -34.0), (-14.0, 10.0), (-10.0, 36.0)]);
    p.paint.extrude(&outline, -4.0, 4.0, deck, hull);
    p.paint.cuboid(Vec3::new(0.0, -14.0, 9.0), Vec3::new(14.0, 22.0, 10.0), rgb(0.85, 0.86, 0.88));
    p.paint.cylinder(Vec3::new(0.0, -20.0, 14.0), Vec3::new(0.0, -20.0, 26.0), 1.0, 6, rgb(0.3, 0.3, 0.32), false);
    p.paint.cuboid(Vec3::new(0.0, -20.0, 26.0), Vec3::new(12.0, 2.0, 1.2), rgb(0.3, 0.3, 0.32));
    p.glow.cuboid(Vec3::new(0.0, -3.2, 11.0), Vec3::new(10.0, 0.6, 3.0), hdr(0.4, 1.2, 1.6));
    p
}

fn hut() -> Parts {
    let walls = rgb(0.82, 0.78, 0.68);
    let roof = rgb(0.2, 0.42, 0.78);
    let mut p = Parts::default();
    p.paint.cuboid(Vec3::new(0.0, 0.0, 2.0), Vec3::new(40.0, 40.0, 20.0), walls);
    p.paint.roof(Vec3::new(0.0, 0.0, 12.0), Vec2::new(46.0, 46.0), 13.0, roof);
    p.paint.cuboid(Vec3::new(0.0, 0.0, 25.2), Vec3::new(3.0, 46.0, 1.2), rgb(0.96, 0.82, 0.25));
    p.glow.ellipsoid(Vec3::new(14.0, -20.5, 8.0), Vec3::new(2.5, 0.6, 2.5), hdr(3.0, 2.4, 0.5), false);
    p
}

fn depot() -> Parts {
    let tank = rgb(0.62, 0.54, 0.4);
    let mut p = Parts::default();
    p.paint.cylinder(Vec3::new(-17.0, 0.0, 6.0), Vec3::new(17.0, 0.0, 6.0), 2.0, 8, rgb(0.3, 0.3, 0.3), false);
    p.paint.symmetric(|m| {
        m.cylinder(Vec3::new(17.0, 0.0, -4.0), Vec3::new(17.0, 0.0, 14.0), 15.0, 20, tank, false);
        m.cylinder(Vec3::new(17.0, 0.0, 8.0), Vec3::new(17.0, 0.0, 10.5), 15.3, 20, rgb(0.82, 0.25, 0.15), false);
        m.ellipsoid(Vec3::new(17.0, 0.0, 14.0), Vec3::new(15.0, 15.0, 5.0), shade(tank, 1.1), true);
    });
    p
}

fn survivor() -> Parts {
    let skin = rgb(0.95, 0.75, 0.55);
    let mut p = Parts::default();
    p.paint.cylinder(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), 14.0, 24, rgb(0.9, 0.9, 0.88), true);
    p.paint.cylinder(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 10.0), 3.0, 8, rgb(1.0, 0.82, 0.1), true);
    p.paint.ellipsoid(Vec3::new(0.0, 0.0, 12.5), Vec3::splat(2.8), skin, false);
    p.paint.symmetric(|m| {
        m.cylinder(Vec3::new(2.6, 0.0, 8.5), Vec3::new(7.0, 0.0, 15.0), 1.1, 6, skin, true);
    });
    p
}

fn lighthouse() -> Parts {
    let white = rgb(0.94, 0.94, 0.92);
    let red = rgb(0.8, 0.16, 0.12);
    let stone = rgb(0.5, 0.48, 0.44);
    let mut p = Parts::default();
    p.paint.frustum(Vec3::new(0.0, 0.0, -6.0), Vec3::new(0.0, 0.0, 4.0), 10.0, 9.0, 14, stone, true);
    p.paint.frustum(Vec3::new(0.0, 0.0, 4.0), Vec3::new(0.0, 0.0, 34.0), 6.5, 4.3, 14, white, true);
    for (z0, z1) in [(11.0, 16.0), (23.0, 28.0)] {
        let r0 = 6.5 - (z0 - 4.0) / 30.0 * 2.2 + 0.15;
        let r1 = 6.5 - (z1 - 4.0) / 30.0 * 2.2 + 0.15;
        p.paint.frustum(Vec3::new(0.0, 0.0, z0), Vec3::new(0.0, 0.0, z1), r0, r1, 14, red, false);
    }
    p.paint.cylinder(Vec3::new(0.0, 0.0, 34.0), Vec3::new(0.0, 0.0, 35.6), 6.4, 14, rgb(0.22, 0.22, 0.25), true);
    p.paint.cone(Vec3::new(0.0, 0.0, 41.0), Vec3::new(0.0, 0.0, 46.5), 4.8, 14, red);
    // Keeper's cottage.
    p.paint.cuboid(Vec3::new(14.0, 4.0, 1.0), Vec3::new(13.0, 10.0, 10.0), rgb(0.88, 0.85, 0.78));
    p.paint.roof(Vec3::new(14.0, 4.0, 6.0), Vec2::new(15.0, 12.0), 5.0, red);
    p.glass.cylinder(Vec3::new(0.0, 0.0, 35.6), Vec3::new(0.0, 0.0, 41.0), 3.7, 12, glass(GLASS_BLUE), false);
    p.glow.ellipsoid(Vec3::new(0.0, 0.0, 38.4), Vec3::splat(2.8), hdr(4.5, 3.8, 2.0), false);
    p
}

// ---------------------------------------------------------------------------
// Title lettering: chunky block capitals built from strokes, extruded with a
// dark backing plate so they read against the sea.
// ---------------------------------------------------------------------------

const LETTER_W: f32 = 44.0;
const LETTER_H: f32 = 64.0;
const STROKE: f32 = 13.0;

/// One stroke of a letter: centre, size and tilt (radians).
type Stroke = (Vec2, Vec2, f32);

fn letter_strokes(c: char) -> Vec<Stroke> {
    let (w, h, s) = (LETTER_W, LETTER_H, STROKE);
    let (hx, hy) = (w / 2.0 - s / 2.0, h / 2.0 - s / 2.0);
    let bar = |y: f32| (Vec2::new(0.0, y), Vec2::new(w, s), 0.0);
    let stem = |x: f32| (Vec2::new(x, 0.0), Vec2::new(s, h), 0.0);
    let half = |x: f32, top: bool| {
        let y = if top { h / 4.0 } else { -h / 4.0 };
        (Vec2::new(x, y), Vec2::new(s, h / 2.0 + s / 2.0), 0.0)
    };
    match c {
        'S' => vec![bar(hy), half(-hx, true), bar(0.0), half(hx, false), bar(-hy)],
        'T' => vec![bar(hy), (Vec2::new(0.0, -s / 2.0), Vec2::new(s, h - s), 0.0)],
        'O' => vec![bar(hy), bar(-hy), stem(-hx), stem(hx)],
        'R' => vec![
            stem(-hx),
            bar(hy),
            half(hx, true),
            bar(0.0),
            (Vec2::new(hx * 0.55, -hy * 0.55), Vec2::new(s, h * 0.58), 0.5),
        ],
        'M' => vec![
            stem(-hx),
            stem(hx),
            (Vec2::new(-hx * 0.48, hy * 0.35), Vec2::new(s * 0.95, h * 0.62), -0.55),
            (Vec2::new(hx * 0.48, hy * 0.35), Vec2::new(s * 0.95, h * 0.62), 0.55),
        ],
        'W' => vec![
            stem(-hx),
            stem(hx),
            (Vec2::new(-hx * 0.48, -hy * 0.35), Vec2::new(s * 0.95, h * 0.62), 0.55),
            (Vec2::new(hx * 0.48, -hy * 0.35), Vec2::new(s * 0.95, h * 0.62), -0.55),
        ],
        'I' => vec![
            (Vec2::ZERO, Vec2::new(s, h), 0.0),
            (Vec2::new(0.0, hy), Vec2::new(w * 0.7, s), 0.0),
            (Vec2::new(0.0, -hy), Vec2::new(w * 0.7, s), 0.0),
        ],
        'N' => vec![stem(-hx), stem(hx), (Vec2::ZERO, Vec2::new(s, h * 1.05), 0.6)],
        'G' => vec![
            bar(hy),
            stem(-hx),
            bar(-hy),
            half(hx, false),
            (Vec2::new(hx * 0.45, 0.0), Vec2::new(w * 0.5, s * 0.85), 0.0),
        ],
        _ => vec![],
    }
}

fn title_models() -> Vec<(Model, f32)> {
    let word = "STORMWING";
    let gap = 9.0;
    let total = word.len() as f32 * LETTER_W + (word.len() - 1) as f32 * gap;
    word.chars()
        .enumerate()
        .map(|(i, c)| {
            let mut m = Model::new();
            for (centre, size, tilt) in letter_strokes(c) {
                let rot = Quat::from_rotation_z(tilt);
                // Dark backing plate, then the metal face on top.
                m.box_tf(centre.extend(-3.0), (size + Vec2::splat(6.0)).extend(6.0), rot, rgb(0.05, 0.08, 0.14));
                m.box_tf(centre.extend(5.0), size.extend(10.0), rot, Color::WHITE);
            }
            let x = -total / 2.0 + LETTER_W / 2.0 + i as f32 * (LETTER_W + gap);
            (m, x)
        })
        .collect()
}

/// Mottled humpback skin: charcoal-blue with paler blotches; white belly.
fn whale_skin(n: Vec3, p: Vec3) -> Color {
    let back = rgb(0.14, 0.17, 0.21);
    let belly = rgb(0.82, 0.84, 0.86);
    if n.z < -0.2 {
        return belly;
    }
    let mottle = crate::noise::fbm(Vec2::new(p.x, p.y) * 0.18, 71, 3);
    let side = smoothstep_f(-0.2, 0.3, n.z);
    let base = if mottle > 0.62 { shade(back, 1.6) } else { back };
    mix_color(belly, base, side)
}

fn smoothstep_f(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    Color::linear_rgb(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t)
}

/// Lofts a whale section with skin colours that vary along the body.
fn whale_loft(m: &mut Model, sections: &[(f32, f32, f32, f32)]) {
    let start = m.vertex_count();
    m.loft(sections, 20, |_, _| Color::WHITE);
    // Recolour from position + normal (the loft colour closure only sees y).
    m.recolor_from(start, whale_skin);
}

/// A humpback: broad flat head with knobbly tubercles, stocky body, a narrow
/// tail stock and notched flukes; long white flippers. Returned as body,
/// tail (origin at the tail-stock joint) and right/left flippers (origin at
/// the root of each flipper).
fn whale_parts() -> (Model, Model, Model, Model) {
    let mut body = Model::new();
    whale_loft(
        &mut body,
        &[
            (-22.0, 13.5, 10.5, 0.0),
            (-8.0, 16.5, 11.5, 0.5),
            (10.0, 17.5, 11.0, 0.5),
            (26.0, 16.0, 9.5, 0.0),
            (40.0, 13.5, 7.5, -0.8),
            (50.0, 10.0, 5.5, -1.3),
            (56.0, 5.5, 3.5, -1.6),
            (59.0, 0.1, 0.1, -1.8),
        ],
    );
    // Tubercles along the head and a dorsal hump with its little fin.
    for i in 0..9 {
        let y = 33.0 + i as f32 * 2.8;
        let x = if i % 2 == 0 { 2.6 } else { -2.6 };
        body.ellipsoid(Vec3::new(x, y, 8.2 - i as f32 * 0.7), Vec3::splat(1.4), rgb(0.11, 0.13, 0.16), true);
    }
    body.ellipsoid(Vec3::new(0.0, -14.0, 9.0), Vec3::new(4.0, 7.0, 2.6), rgb(0.13, 0.16, 0.2), true);
    body.fin(&pts(&[(-10.0, 9.5), (-15.0, 14.5), (-19.0, 14.0), (-18.0, 9.5)]), 0.0, 1.8, 0.0, rgb(0.13, 0.16, 0.2));
    // Blowholes.
    body.ellipsoid(Vec3::new(1.2, 30.0, 9.6), Vec3::new(0.7, 1.4, 0.4), rgb(0.05, 0.05, 0.06), true);
    body.ellipsoid(Vec3::new(-1.2, 30.0, 9.6), Vec3::new(0.7, 1.4, 0.4), rgb(0.05, 0.05, 0.06), true);

    let mut tail = Model::new();
    whale_loft(
        &mut tail,
        &[
            (0.0, 13.5, 10.5, 0.0),
            (-14.0, 9.0, 8.5, 0.0),
            (-28.0, 4.5, 6.5, 0.0),
            (-36.0, 2.6, 3.6, 0.0),
            (-40.0, 0.1, 0.1, 0.0),
        ],
    );
    let fluke_top = rgb(0.13, 0.15, 0.19);
    tail.symmetric(|m| {
        // Wide flukes with a central notch and a scalloped trailing edge.
        m.wing(
            &pts(&[(1.2, -35.0), (12.0, -38.0), (26.0, -46.0), (33.0, -54.0), (28.0, -55.0), (24.0, -52.0), (19.0, -53.0), (14.0, -50.0), (9.0, -50.5), (4.0, -47.0), (1.2, -45.0)]),
            0.0,
            2.0,
            fluke_top,
        );
    });

    let mut fin_r = Model::new();
    let fin_color = rgb(0.86, 0.88, 0.9);
    fin_r.wing(
        &pts(&[(0.0, 5.0), (10.0, 2.0), (22.0, -6.0), (34.0, -14.0), (40.0, -19.0), (38.0, -22.0), (28.0, -18.0), (14.0, -10.0), (0.0, -5.0)]),
        0.0,
        2.2,
        fin_color,
    );
    // Knobbly leading edge.
    for k in 0..6 {
        let t = k as f32 / 5.0;
        let p = Vec2::new(10.0, 2.0).lerp(Vec2::new(40.0, -19.0), t);
        fin_r.ellipsoid(p.extend(0.6), Vec3::splat(1.3), rgb(0.72, 0.74, 0.76), true);
    }
    let mut fin_l = Model::new();
    fin_l.append_mirror_x(&fin_r);
    (body, tail, fin_r, fin_l)
}

fn ice_chunk(i: u64) -> Model {
    let mut rng = crate::rng::Rng::new(500 + i);
    let mut m = Model::new();
    if i % 2 == 0 {
        let n = 7;
        let poly: Vec<Vec2> = (0..n)
            .map(|k| Vec2::from_angle(k as f32 / n as f32 * TAU + rng.range(-0.2, 0.2)) * rng.range(0.7, 1.0))
            .collect();
        m.extrude(&poly, -1.5, 0.35, rgb(0.93, 0.97, 1.0), rgb(0.6, 0.8, 0.92));
    } else {
        let mut jitter = crate::rng::Rng::new(900 + i);
        m.rock(Vec3::ZERO, Vec3::new(1.0, 0.85, 0.7), rgb(0.8, 0.92, 0.99), move || jitter.range(0.7, 1.15));
    }
    m
}

/// Two opposite fans of light, bright at the lantern and fading outwards.
fn beam_model() -> Model {
    let mut m = Model::new();
    // A wide soft glow and a narrow bright core for each of the two beams.
    for (reach, spread, glow) in [(330.0, 0.13, [2.6, 2.3, 1.5, 0.5]), (300.0, 0.045, [5.0, 4.4, 3.0, 0.75])] {
        for side in [0.0, std::f32::consts::PI] {
            let centre = m.vertex(Vec3::ZERO, Vec3::Z, glow);
            let steps = 8;
            let mut prev = None;
            for i in 0..=steps {
                let a = side - spread + 2.0 * spread * i as f32 / steps as f32;
                let p = Vec2::from_angle(a + FRAC_PI_2) * reach;
                let v = m.vertex(p.extend(0.0), Vec3::Z, [glow[0], glow[1], glow[2], 0.0]);
                if let Some(prev) = prev {
                    m.triangle(centre, prev, v);
                }
                prev = Some(v);
            }
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Bosses. Nose along +Y; they're spawned rotated to face the player.
// ---------------------------------------------------------------------------

fn boss_wing() -> Parts {
    let skin = rgb(0.43, 0.48, 0.56);
    let light = rgb(0.6, 0.64, 0.7);
    let dark = rgb(0.2, 0.22, 0.27);
    let red = rgb(0.78, 0.18, 0.14);
    let mut p = Parts::default();
    p.paint.loft(
        &[(-86.0, 10.0, 8.0, 0.0), (-70.0, 30.0, 14.0, 0.0), (-20.0, 42.0, 20.0, 0.0), (40.0, 36.0, 18.0, 0.0), (80.0, 22.0, 12.0, 0.0), (100.0, 0.1, 0.1, 0.0)],
        20,
        move |n, _| if n.z < -0.3 { shade(light, 0.6) } else { light },
    );
    p.paint.symmetric(|m| {
        m.wing(
            &pts(&[(30.0, 48.0), (190.0, -20.0), (196.0, -42.0), (150.0, -40.0), (120.0, -62.0), (70.0, -48.0), (40.0, -74.0), (25.0, -70.0)]),
            0.0,
            12.0,
            skin,
        );
        m.wing(&pts(&[(150.0, -12.0), (190.0, -20.0), (196.0, -42.0), (160.0, -36.0)]), 0.0, 12.8, red);
        m.frustum(Vec3::new(70.0, -12.0, -5.0), Vec3::new(70.0, -54.0, -5.0), 9.0, 11.0, 14, dark, true);
        m.frustum(Vec3::new(100.0, -16.0, -5.0), Vec3::new(100.0, -54.0, -5.0), 8.0, 10.0, 14, dark, true);
        m.fin(&pts(&[(-50.0, 6.0), (-70.0, 34.0), (-78.0, 34.0), (-76.0, 6.0)]), 30.0, 3.0, 0.25, skin);
    });
    p.paint.cylinder(Vec3::new(0.0, 4.0, 16.0), Vec3::new(0.0, 4.0, 22.0), 28.0, 24, dark, true);
    p.glass.ellipsoid(Vec3::new(0.0, 70.0, 13.0), Vec3::new(11.0, 16.0, 7.0), glass(GLASS_BLUE), true);
    p.glow.ellipsoid(Vec3::new(0.0, 4.0, 22.0), Vec3::new(15.0, 15.0, 7.0), hdr(4.0, 0.7, 0.45), true);
    p.glow.symmetric(|m| {
        m.ellipsoid(Vec3::new(70.0, -54.8, -5.0), Vec3::new(9.0, 1.0, 9.0), hdr(4.0, 1.6, 0.5), false);
        m.ellipsoid(Vec3::new(100.0, -54.8, -5.0), Vec3::new(8.0, 1.0, 8.0), hdr(4.0, 1.6, 0.5), false);
    });
    p
}

fn boss_crawler() -> Parts {
    let armor = rgb(0.64, 0.52, 0.36);
    let armor_dark = rgb(0.46, 0.36, 0.23);
    let tread = rgb(0.17, 0.15, 0.13);
    let mut p = Parts::default();
    p.paint.symmetric(|m| {
        m.cuboid(Vec3::new(135.0, 0.0, 0.0), Vec3::new(54.0, 300.0, 28.0), tread);
        for i in 0..15 {
            let y = -140.0 + i as f32 * 20.0;
            m.cuboid(Vec3::new(135.0, y, 14.4), Vec3::new(55.0, 6.0, 1.2), rgb(0.33, 0.3, 0.27));
        }
    });
    let hull = pts(&[(0.0, 150.0), (70.0, 140.0), (110.0, 110.0), (110.0, -120.0), (80.0, -145.0), (0.0, -150.0), (-80.0, -145.0), (-110.0, -120.0), (-110.0, 110.0), (-70.0, 140.0)]);
    p.paint.extrude(&hull, -6.0, 26.0, armor, shade(armor, 0.75));
    let deck = pts(&[(0.0, 120.0), (60.0, 110.0), (82.0, 80.0), (82.0, -90.0), (0.0, -100.0), (-82.0, -90.0), (-82.0, 80.0), (-60.0, 110.0)]);
    p.paint.extrude(&deck, 26.0, 38.0, armor_dark, shade(armor_dark, 0.8));
    p.paint.extrude(&ngon(Vec2::new(0.0, 10.0), 46.0, 6, 0.0), 38.0, 46.0, rgb(0.38, 0.3, 0.2), rgb(0.3, 0.24, 0.16));
    p.paint.cylinder(Vec3::new(0.0, 100.0, 34.0), Vec3::new(0.0, 175.0, 34.0), 8.0, 12, rgb(0.24, 0.2, 0.14), true);
    p.paint.cuboid(Vec3::new(0.0, -122.0, 30.0), Vec3::new(120.0, 18.0, 10.0), rgb(0.3, 0.25, 0.18));
    p.glow.ellipsoid(Vec3::new(0.0, 10.0, 46.0), Vec3::new(18.0, 18.0, 7.0), hdr(4.0, 1.2, 0.3), true);
    p.glow.symmetric(|m| {
        m.cuboid(Vec3::new(40.0, -131.5, 30.0), Vec3::new(14.0, 1.0, 6.0), hdr(3.6, 1.2, 0.3));
    });
    p
}

fn boss_citadel() -> Parts {
    let hull = rgb(0.52, 0.58, 0.7);
    let hull_dark = rgb(0.28, 0.32, 0.42);
    let trim = rgb(0.78, 0.84, 0.94);
    let mut p = Parts::default();
    p.paint.extrude(&ngon(Vec2::ZERO, 190.0, 6, FRAC_PI_2), -12.0, 8.0, hull_dark, shade(hull_dark, 0.8));
    p.paint.extrude(&ngon(Vec2::ZERO, 170.0, 6, FRAC_PI_2), 8.0, 20.0, hull, shade(hull, 0.8));
    for i in 0..6 {
        let a = FRAC_PI_2 + i as f32 * TAU / 6.0;
        let d = Vec2::from_angle(a);
        p.paint.box_tf((d * 105.0).extend(22.0), Vec3::new(10.0, 130.0, 6.0), Quat::from_rotation_z(a - FRAC_PI_2), hull_dark);
    }
    for i in 0..30 {
        let a = i as f32 * TAU / 30.0;
        let d = Vec2::from_angle(a);
        p.paint.box_tf((d * 154.0).extend(21.0), Vec3::new(30.0, 6.0, 3.0), Quat::from_rotation_z(a + FRAC_PI_2), trim);
    }
    p.paint.extrude(&ngon(Vec2::ZERO, 72.0, 6, 0.0), 20.0, 34.0, hull_dark, shade(hull_dark, 0.8));
    p.paint.ellipsoid(Vec3::new(0.0, 0.0, 34.0), Vec3::new(46.0, 46.0, 14.0), rgb(0.16, 0.12, 0.26), true);
    p.glow.ellipsoid(Vec3::new(0.0, 0.0, 40.0), Vec3::new(28.0, 28.0, 10.0), hdr(2.4, 0.6, 3.4), true);
    for i in 0..12 {
        let d = Vec2::from_angle(i as f32 * TAU / 12.0 + 0.26);
        p.glow.ellipsoid((d * 128.0).extend(21.0), Vec3::splat(3.0), hdr(0.4, 2.0, 3.0), false);
    }
    p
}

fn boss_turret() -> Parts {
    let mut p = Parts::default();
    p.paint.extrude(&ngon(Vec2::ZERO, 30.0, 8, 0.39), -4.0, 6.0, rgb(0.24, 0.25, 0.29), rgb(0.18, 0.19, 0.22));
    p.paint.extrude(&ngon(Vec2::ZERO, 26.0, 8, 0.39), 6.0, 10.0, rgb(0.46, 0.47, 0.52), rgb(0.36, 0.37, 0.41));
    p
}

fn boss_barrel() -> Parts {
    let mut p = Parts::default();
    p.paint.ellipsoid(Vec3::new(0.0, 0.0, 10.0), Vec3::new(15.0, 15.0, 9.0), rgb(0.36, 0.37, 0.42), true);
    p.paint.symmetric(|m| {
        m.cylinder(Vec3::new(5.0, 4.0, 14.0), Vec3::new(5.0, 34.0, 14.0), 2.8, 8, rgb(0.16, 0.16, 0.18), true);
        m.cylinder(Vec3::new(5.0, 31.0, 14.0), Vec3::new(5.0, 36.0, 14.0), 3.8, 8, rgb(0.1, 0.1, 0.12), true);
    });
    p.glow.ellipsoid(Vec3::new(0.0, -2.0, 18.5), Vec3::new(5.0, 5.0, 1.5), hdr(3.4, 0.6, 0.3), false);
    p
}

// ---------------------------------------------------------------------------
// Pickups.
// ---------------------------------------------------------------------------

fn star_model(scale: f32) -> Model {
    let mut m = Model::new();
    let outline: Vec<Vec2> = star_outline(14.0 * scale, 6.3 * scale, 5);
    m.extrude(&outline, -2.2 * scale, 2.2 * scale, rgb(1.0, 0.85, 0.35), rgb(0.85, 0.55, 0.1));
    let inner: Vec<Vec2> = star_outline(9.0 * scale, 4.1 * scale, 5);
    m.extrude(&inner, 2.2 * scale, 3.0 * scale, rgb(1.0, 0.95, 0.6), rgb(1.0, 0.8, 0.3));
    m
}

fn icon_weapon() -> Model {
    let mut m = Model::new();
    for y in [-6.0, 4.0] {
        m.extrude(&pts(&[(0.0, y + 10.0), (11.0, y), (11.0, y - 5.0), (0.0, y + 5.0), (-11.0, y - 5.0), (-11.0, y)]), 0.0, 2.0, Color::WHITE, Color::WHITE);
    }
    m
}

fn icon_shield() -> Model {
    let mut m = Model::new();
    let outline = pts(&[(0.0, 13.0), (11.0, 9.0), (10.0, -2.0), (0.0, -13.0), (-10.0, -2.0), (-11.0, 9.0)]);
    m.extrude(&outline, 0.0, 2.0, Color::WHITE, Color::WHITE);
    m
}

fn icon_repair() -> Model {
    let mut m = Model::new();
    m.cuboid(Vec3::new(0.0, 0.0, 1.0), Vec3::new(22.0, 7.0, 2.0), Color::WHITE);
    m.cuboid(Vec3::new(0.0, 0.0, 1.0), Vec3::new(7.0, 22.0, 2.0), Color::WHITE);
    m
}

fn icon_bomb() -> Model {
    let mut m = Model::new();
    m.ellipsoid(Vec3::new(0.0, -2.0, 0.0), Vec3::new(9.5, 9.5, 4.0), Color::WHITE, true);
    m.cylinder(Vec3::new(4.0, 6.0, 1.0), Vec3::new(8.0, 12.0, 1.0), 2.0, 6, Color::WHITE, true);
    m
}
