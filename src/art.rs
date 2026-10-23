//! All of the game's vector art, generated at startup.

use crate::shapes::{glow, pts, Shape};
use crate::textures;
use bevy::prelude::*;

#[derive(Resource)]
pub struct Art {
    pub mat_base: Handle<ColorMaterial>,
    pub mat_flash: Handle<ColorMaterial>,
    pub mat_shadow: Handle<ColorMaterial>,

    pub player: Handle<Mesh>,
    pub dart: Handle<Mesh>,
    pub swooper: Handle<Mesh>,
    pub chaser: Handle<Mesh>,
    pub gunship: Handle<Mesh>,
    pub bomber: Handle<Mesh>,
    pub turret_base: Handle<Mesh>,
    pub turret_barrel: Handle<Mesh>,
    pub tank_body: Handle<Mesh>,
    pub tank_turret: Handle<Mesh>,
    pub boat: Handle<Mesh>,
    pub hut: Handle<Mesh>,
    pub depot: Handle<Mesh>,
    pub bosses: [Handle<Mesh>; 3],
    pub boss_turret: Handle<Mesh>,
    pub boss_barrel: Handle<Mesh>,
    pub missile: Handle<Mesh>,

    pub star: Handle<Mesh>,
    pub big_star: Handle<Mesh>,
    pub icons: [Handle<Mesh>; 4],
    pub survivor: Handle<Mesh>,
    /// 1x1 quad with UVs, scaled per particle.
    pub quad: Handle<Mesh>,

    pub tex_glow: Handle<Image>,
    pub tex_orb: Handle<Image>,
    pub tex_disc: Handle<Image>,
    pub tex_ring: Handle<Image>,
    pub tex_streak: Handle<Image>,
    pub tex_clouds: Vec<Handle<Image>>,
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
            world.resource_scope(|world, mut materials: Mut<Assets<ColorMaterial>>| {
                let mut images = world.resource_mut::<Assets<Image>>();
                build_art(&mut meshes, &mut materials, &mut images)
            })
        });
        world.insert_resource(art);
    }
}

fn build_art(meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, images: &mut Assets<Image>) -> Art {
    let mut m = |s: Shape| meshes.add(s.build());
    Art {
        mat_base: materials.add(ColorMaterial::from_color(Color::WHITE)),
        mat_flash: materials.add(ColorMaterial::from_color(Color::linear_rgb(2.4, 2.4, 2.6))),
        mat_shadow: materials.add(ColorMaterial::from_color(Color::srgba(0.0, 0.02, 0.06, 0.28))),

        player: m(player()),
        dart: m(dart()),
        swooper: m(swooper()),
        chaser: m(chaser()),
        gunship: m(gunship()),
        bomber: m(bomber()),
        turret_base: m(turret_base()),
        turret_barrel: m(turret_barrel()),
        tank_body: m(tank_body()),
        tank_turret: m(tank_turret()),
        boat: m(boat()),
        hut: m(hut()),
        depot: m(depot()),
        bosses: [m(boss_wing()), m(boss_crawler()), m(boss_citadel())],
        boss_turret: m(boss_turret()),
        boss_barrel: m(boss_barrel()),
        missile: m(missile()),

        star: m(star(1.0)),
        big_star: m(star(1.6)),
        icons: [m(icon_weapon()), m(icon_shield()), m(icon_repair()), m(icon_bomb())],
        survivor: m(survivor()),
        quad: meshes.add(Rectangle::new(1.0, 1.0)),

        tex_glow: images.add(textures::soft_glow()),
        tex_orb: images.add(textures::orb()),
        tex_disc: images.add(textures::disc()),
        tex_ring: images.add(textures::ring()),
        tex_streak: images.add(textures::streak()),
        tex_clouds: (0..4).map(|i| images.add(textures::cloud(11 + i * 31))).collect(),
        tex_star_icon: images.add(textures::star_icon()),
        tex_medal: images.add(textures::medal_icon()),
    }
}

fn c(r: f32, g: f32, b: f32) -> Color {
    Color::srgb(r, g, b)
}

// ---------------------------------------------------------------------------
// Aircraft. All are drawn nose-up (+Y); enemies are rotated to face travel.
// ---------------------------------------------------------------------------

fn player() -> Shape {
    let hull = c(0.86, 0.89, 0.93);
    let hull_mid = c(0.62, 0.68, 0.76);
    let hull_dark = c(0.3, 0.34, 0.42);
    let accent = c(0.98, 0.5, 0.12);
    let mut s = Shape::new();
    // Swept main wing.
    s.sym(
        &pts(&[(0.0, 16.0), (12.0, 14.0), (42.0, -8.0), (44.0, -19.0), (12.0, -14.0), (0.0, -14.0)], 1.0),
        hull_mid,
    );
    // Wing-tip accents.
    s.pair(&pts(&[(33.0, -2.5), (42.5, -9.5), (44.0, -19.0), (36.0, -16.0)], 1.0), accent);
    // Canards.
    s.pair(&pts(&[(7.0, 24.0), (17.0, 16.0), (17.0, 13.0), (7.0, 15.0)], 1.0), hull_mid);
    // Tail fins.
    s.pair(&pts(&[(6.0, -24.0), (21.0, -37.0), (22.0, -43.0), (6.0, -36.0)], 1.0), hull_dark);
    // Fuselage.
    s.sym(
        &pts(&[(0.0, 48.0), (5.0, 38.0), (8.5, 22.0), (10.5, 4.0), (10.5, -26.0), (8.0, -40.0), (0.0, -42.0)], 1.0),
        hull,
    );
    s.line(Vec2::new(0.0, 8.0), Vec2::new(0.0, -36.0), 3.0, hull_mid);
    // Air intakes and engine nozzles.
    s.rect_pair(Vec2::new(11.5, 2.0), Vec2::new(3.5, 16.0), hull_dark);
    s.rect_pair(Vec2::new(4.5, -42.0), Vec2::new(6.0, 5.0), c(0.18, 0.18, 0.22));
    // Nose cone stripe.
    s.sym(&pts(&[(0.0, 48.0), (4.0, 39.0), (5.5, 34.0), (0.0, 35.0)], 1.0), accent);
    // Canopy.
    s.ellipse(Vec2::new(0.0, 20.0), Vec2::new(4.6, 10.5), glow(0.25, 0.95, 1.5));
    s.ellipse(Vec2::new(-1.4, 24.0), Vec2::new(1.3, 3.5), c(0.95, 1.0, 1.0));
    // Navigation lights.
    s.circle(Vec2::new(41.5, -15.5), 2.0, glow(0.3, 2.6, 0.5));
    s.circle(Vec2::new(-41.5, -15.5), 2.0, glow(2.8, 0.3, 0.25));
    s
}

fn dart() -> Shape {
    let body = c(0.36, 0.37, 0.43);
    let wing = c(0.74, 0.17, 0.14);
    let dark = c(0.2, 0.2, 0.25);
    let mut s = Shape::new();
    s.sym(&pts(&[(0.0, 10.0), (23.0, -12.0), (23.0, -18.0), (8.0, -14.0), (0.0, -14.0)], 1.0), wing);
    s.pair(&pts(&[(17.0, -9.0), (23.0, -12.0), (23.0, -18.0), (17.0, -16.0)], 1.0), c(0.95, 0.85, 0.75));
    s.pair(&pts(&[(4.0, -12.0), (11.0, -24.0), (4.0, -22.0)], 1.0), dark);
    s.sym(&pts(&[(0.0, 27.0), (5.0, 14.0), (6.0, -16.0), (3.0, -23.0), (0.0, -23.0)], 1.0), body);
    s.ellipse(Vec2::new(0.0, 8.0), Vec2::new(3.0, 6.0), glow(2.2, 1.4, 0.3));
    s
}

fn swooper() -> Shape {
    let wing = c(0.18, 0.5, 0.42);
    let wing_dark = c(0.1, 0.3, 0.26);
    let body = c(0.3, 0.32, 0.37);
    let mut s = Shape::new();
    s.sym(
        &pts(&[(0.0, 12.0), (14.0, 9.0), (31.0, -1.0), (30.0, -15.0), (19.0, -8.0), (10.0, -17.0), (0.0, -11.0)], 1.0),
        wing,
    );
    s.pair(&pts(&[(20.0, 3.0), (31.0, -1.0), (30.0, -15.0), (24.0, -10.0)], 1.0), wing_dark);
    s.sym(&pts(&[(0.0, 19.0), (5.5, 10.0), (6.0, -12.0), (0.0, -19.0)], 1.0), body);
    s.circle(Vec2::new(0.0, 4.0), 3.6, glow(0.4, 2.6, 1.3));
    s
}

fn chaser() -> Shape {
    let mut s = Shape::new();
    s.star(Vec2::ZERO, 19.0, 8.0, 4, c(0.24, 0.24, 0.28));
    s.circle(Vec2::ZERO, 10.0, c(0.92, 0.42, 0.1));
    s.ring(Vec2::ZERO, 10.0, 12.0, c(0.15, 0.15, 0.18));
    s.circle(Vec2::ZERO, 5.0, glow(3.5, 1.2, 0.2));
    s
}

fn gunship() -> Shape {
    let olive = c(0.47, 0.52, 0.34);
    let olive_dark = c(0.31, 0.36, 0.22);
    let pod = c(0.23, 0.25, 0.21);
    let mut s = Shape::new();
    s.sym(
        &pts(&[(0.0, 20.0), (20.0, 18.0), (56.0, 1.0), (56.0, -13.0), (20.0, -17.0), (0.0, -17.0)], 1.0),
        olive_dark,
    );
    s.rect_pair(Vec2::new(46.0, -6.0), Vec2::new(7.0, 13.0), c(0.86, 0.74, 0.2));
    s.rect_pair(Vec2::new(30.0, -4.0), Vec2::new(13.0, 36.0), pod);
    s.circle_pair(Vec2::new(30.0, -23.0), 4.5, glow(3.0, 1.2, 0.3));
    s.sym(
        &pts(&[(0.0, 44.0), (9.0, 31.0), (14.0, 10.0), (14.0, -30.0), (8.0, -41.0), (0.0, -43.0)], 1.0),
        olive,
    );
    s.pair(&pts(&[(5.0, -30.0), (20.0, -44.0), (20.0, -49.0), (5.0, -41.0)], 1.0), olive_dark);
    s.ellipse(Vec2::new(0.0, 25.0), Vec2::new(5.0, 9.0), glow(2.0, 1.3, 0.35));
    s.circle(Vec2::new(0.0, -8.0), 8.0, pod);
    s.circle(Vec2::new(0.0, -8.0), 4.0, c(0.5, 0.52, 0.45));
    s
}

fn bomber() -> Shape {
    let steel = c(0.4, 0.44, 0.52);
    let steel_light = c(0.58, 0.61, 0.68);
    let dark = c(0.2, 0.22, 0.27);
    let mut s = Shape::new();
    s.sym(
        &pts(&[(0.0, 26.0), (30.0, 23.0), (96.0, -3.0), (96.0, -17.0), (30.0, -14.0), (0.0, -14.0)], 1.0),
        steel,
    );
    s.sym(&pts(&[(0.0, -38.0), (32.0, -55.0), (32.0, -64.0), (0.0, -58.0)], 1.0), steel);
    s.rect_pair(Vec2::new(32.0, 6.0), Vec2::new(13.0, 32.0), dark);
    s.rect_pair(Vec2::new(63.0, -1.0), Vec2::new(11.0, 26.0), dark);
    s.circle_pair(Vec2::new(32.0, -11.0), 4.5, glow(3.0, 1.3, 0.3));
    s.circle_pair(Vec2::new(63.0, -15.0), 4.0, glow(3.0, 1.3, 0.3));
    s.circle_pair(Vec2::new(82.0, -8.0), 6.0, c(0.82, 0.2, 0.16));
    s.circle_pair(Vec2::new(82.0, -8.0), 2.5, c(0.95, 0.95, 0.95));
    s.sym(
        &pts(&[(0.0, 60.0), (12.0, 46.0), (16.0, 10.0), (15.0, -44.0), (10.0, -60.0), (0.0, -62.0)], 1.0),
        steel_light,
    );
    s.sym(&pts(&[(0.0, 55.0), (7.0, 46.0), (7.0, 39.0), (0.0, 41.0)], 1.0), glow(0.4, 1.4, 2.0));
    s.line(Vec2::new(0.0, 30.0), Vec2::new(0.0, -50.0), 4.0, steel);
    s
}

fn missile() -> Shape {
    let mut s = Shape::new();
    s.pair(&pts(&[(2.0, -6.0), (6.0, -11.0), (2.0, -10.0)], 1.0), c(0.5, 0.5, 0.55));
    s.sym(&pts(&[(0.0, 12.0), (2.5, 7.0), (2.5, -10.0), (0.0, -10.0)], 1.0), c(0.92, 0.92, 0.95));
    s.sym(&pts(&[(0.0, 12.0), (2.5, 7.0), (0.0, 6.0)], 1.0), c(0.95, 0.3, 0.15));
    s
}

// ---------------------------------------------------------------------------
// Ground targets (top-down).
// ---------------------------------------------------------------------------

fn turret_base() -> Shape {
    let mut s = Shape::new();
    s.regular(Vec2::new(3.0, -3.0), 22.0, 8, 0.39, c(0.1, 0.1, 0.12).with_alpha(0.5));
    s.regular(Vec2::ZERO, 21.0, 8, 0.39, c(0.36, 0.37, 0.4));
    s.ring(Vec2::ZERO, 13.0, 16.0, c(0.22, 0.23, 0.26));
    s.circle(Vec2::ZERO, 12.0, c(0.5, 0.51, 0.54));
    for i in 0..4 {
        let a = i as f32 * std::f32::consts::FRAC_PI_2 + 0.785;
        s.circle(Vec2::from_angle(a) * 18.0, 1.6, c(0.2, 0.2, 0.22));
    }
    s
}

fn turret_barrel() -> Shape {
    let mut s = Shape::new();
    s.rect(Vec2::new(0.0, 14.0), Vec2::new(7.0, 24.0), c(0.18, 0.18, 0.2));
    s.rect(Vec2::new(0.0, 26.0), Vec2::new(9.0, 4.0), c(0.12, 0.12, 0.14));
    s.circle(Vec2::ZERO, 8.5, c(0.28, 0.29, 0.32));
    s.circle(Vec2::ZERO, 3.0, glow(2.2, 0.5, 0.25));
    s
}

fn tank_body() -> Shape {
    let mut s = Shape::new();
    s.rect(Vec2::new(4.0, -4.0), Vec2::new(44.0, 52.0), c(0.0, 0.0, 0.0).with_alpha(0.3));
    s.rect_pair(Vec2::new(17.0, 0.0), Vec2::new(10.0, 50.0), c(0.17, 0.17, 0.16));
    for i in 0..8 {
        let y = -22.0 + i as f32 * 6.3;
        s.rect_pair(Vec2::new(17.0, y), Vec2::new(10.0, 1.6), c(0.3, 0.3, 0.28));
    }
    s.rect(Vec2::new(0.0, 0.0), Vec2::new(26.0, 44.0), c(0.42, 0.45, 0.3));
    s.rect(Vec2::new(0.0, -17.0), Vec2::new(22.0, 6.0), c(0.33, 0.36, 0.23));
    s
}

fn tank_turret() -> Shape {
    let mut s = Shape::new();
    s.rect(Vec2::new(0.0, 17.0), Vec2::new(5.0, 24.0), c(0.22, 0.24, 0.16));
    s.circle(Vec2::ZERO, 11.0, c(0.52, 0.56, 0.38));
    s.circle(Vec2::new(-3.0, -2.0), 3.5, c(0.33, 0.36, 0.23));
    s
}

fn boat() -> Shape {
    let mut s = Shape::new();
    s.sym(
        &pts(&[(0.0, 54.0), (10.0, 36.0), (14.0, 10.0), (14.0, -34.0), (11.0, -44.0), (0.0, -46.0)], 1.0),
        c(0.46, 0.48, 0.52),
    );
    s.sym(
        &pts(&[(0.0, 46.0), (7.0, 34.0), (10.0, 10.0), (10.0, -32.0), (0.0, -38.0)], 1.0),
        c(0.6, 0.62, 0.66),
    );
    s.rect(Vec2::new(0.0, -14.0), Vec2::new(14.0, 22.0), c(0.84, 0.85, 0.87));
    s.rect(Vec2::new(0.0, -8.0), Vec2::new(10.0, 4.0), glow(0.4, 1.0, 1.4));
    s.circle(Vec2::new(0.0, -32.0), 3.0, c(0.3, 0.3, 0.32));
    s
}

fn hut() -> Shape {
    let mut s = Shape::new();
    s.rect(Vec2::new(5.0, -5.0), Vec2::new(44.0, 44.0), c(0.0, 0.0, 0.0).with_alpha(0.3));
    s.rect(Vec2::ZERO, Vec2::new(42.0, 42.0), c(0.8, 0.76, 0.66));
    s.poly(&pts(&[(-23.0, -2.0), (23.0, -2.0), (15.0, 21.0), (-15.0, 21.0)], 1.0), c(0.2, 0.42, 0.78));
    s.poly(&pts(&[(-23.0, 2.0), (23.0, 2.0), (15.0, -21.0), (-15.0, -21.0)], 1.0), c(0.15, 0.32, 0.62));
    s.rect(Vec2::ZERO, Vec2::new(46.0, 3.0), c(0.96, 0.82, 0.25));
    s.circle(Vec2::new(14.0, 12.0), 3.0, glow(2.8, 2.2, 0.4));
    s
}

fn depot() -> Shape {
    let mut s = Shape::new();
    s.rect(Vec2::new(4.0, -4.0), Vec2::new(64.0, 34.0), c(0.0, 0.0, 0.0).with_alpha(0.3));
    s.rect(Vec2::ZERO, Vec2::new(30.0, 6.0), c(0.3, 0.3, 0.3));
    for x in [-17.0, 17.0] {
        s.circle(Vec2::new(x, 0.0), 16.0, c(0.58, 0.5, 0.38));
        s.ring(Vec2::new(x, 0.0), 12.0, 16.0, c(0.42, 0.36, 0.27));
        s.circle(Vec2::new(x, 0.0), 6.0, c(0.85, 0.3, 0.15));
    }
    s
}

// ---------------------------------------------------------------------------
// Bosses (drawn nose-up, spawned rotated to face the player).
// ---------------------------------------------------------------------------

/// Stage 1: a huge flying wing.
fn boss_wing() -> Shape {
    let skin = c(0.42, 0.47, 0.55);
    let skin_light = c(0.6, 0.64, 0.7);
    let dark = c(0.2, 0.22, 0.27);
    let red = c(0.78, 0.18, 0.14);
    let mut s = Shape::new();
    s.sym(
        &pts(
            &[(0.0, 70.0), (60.0, 48.0), (190.0, -20.0), (196.0, -42.0), (150.0, -40.0), (120.0, -62.0), (70.0, -48.0), (40.0, -74.0), (0.0, -60.0)],
            1.0,
        ),
        skin,
    );
    s.pair(&pts(&[(150.0, -12.0), (190.0, -20.0), (196.0, -42.0), (160.0, -36.0)], 1.0), red);
    s.sym(
        &pts(&[(0.0, 100.0), (26.0, 70.0), (40.0, 20.0), (38.0, -50.0), (20.0, -82.0), (0.0, -86.0)], 1.0),
        skin_light,
    );
    for x in [70.0, 100.0] {
        s.rect_pair(Vec2::new(x, -30.0), Vec2::new(18.0, 34.0), dark);
        s.circle_pair(Vec2::new(x, -50.0), 6.0, glow(3.2, 1.4, 0.35));
    }
    s.sym(&pts(&[(0.0, 92.0), (12.0, 74.0), (12.0, 62.0), (0.0, 66.0)], 1.0), glow(0.4, 1.5, 2.2));
    s.line(Vec2::new(0.0, 50.0), Vec2::new(0.0, -70.0), 6.0, skin);
    s.ring(Vec2::new(0.0, 4.0), 22.0, 28.0, dark);
    s.circle(Vec2::new(0.0, 4.0), 22.0, c(0.3, 0.32, 0.38));
    s.circle(Vec2::new(0.0, 4.0), 12.0, glow(3.5, 0.6, 0.4));
    s
}

/// Stage 2: a tracked desert fortress.
fn boss_crawler() -> Shape {
    let armor = c(0.62, 0.5, 0.34);
    let armor_dark = c(0.45, 0.35, 0.22);
    let tread = c(0.18, 0.16, 0.14);
    let mut s = Shape::new();
    s.rect(Vec2::new(10.0, -10.0), Vec2::new(330.0, 300.0), c(0.0, 0.0, 0.0).with_alpha(0.3));
    s.rect_pair(Vec2::new(135.0, 0.0), Vec2::new(54.0, 300.0), tread);
    for i in 0..15 {
        let y = -140.0 + i as f32 * 20.0;
        s.rect_pair(Vec2::new(135.0, y), Vec2::new(54.0, 5.0), c(0.32, 0.3, 0.27));
    }
    s.sym(
        &pts(&[(0.0, 150.0), (70.0, 140.0), (110.0, 110.0), (110.0, -120.0), (80.0, -145.0), (0.0, -150.0)], 1.0),
        armor,
    );
    s.sym(&pts(&[(0.0, 120.0), (60.0, 110.0), (82.0, 80.0), (82.0, -90.0), (0.0, -100.0)], 1.0), armor_dark);
    s.rect(Vec2::new(0.0, -120.0), Vec2::new(120.0, 18.0), c(0.3, 0.25, 0.18));
    s.rect_pair(Vec2::new(40.0, -128.0), Vec2::new(14.0, 10.0), glow(3.0, 1.0, 0.25));
    s.regular(Vec2::new(0.0, 10.0), 46.0, 6, 0.0, c(0.38, 0.3, 0.2));
    s.regular(Vec2::new(0.0, 10.0), 34.0, 6, 0.0, c(0.25, 0.2, 0.14));
    s.circle(Vec2::new(0.0, 10.0), 16.0, glow(3.4, 1.0, 0.25));
    s.rect(Vec2::new(0.0, 100.0), Vec2::new(16.0, 60.0), c(0.25, 0.2, 0.14));
    s
}

/// Stage 3: a hexagonal sky citadel.
fn boss_citadel() -> Shape {
    let hull = c(0.5, 0.56, 0.68);
    let hull_dark = c(0.28, 0.32, 0.42);
    let trim = c(0.75, 0.82, 0.92);
    let mut s = Shape::new();
    s.regular(Vec2::ZERO, 190.0, 6, std::f32::consts::FRAC_PI_2, hull_dark);
    s.regular(Vec2::ZERO, 170.0, 6, std::f32::consts::FRAC_PI_2, hull);
    for i in 0..6 {
        let a = std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 6.0;
        let d = Vec2::from_angle(a);
        s.line(d * 40.0, d * 165.0, 10.0, hull_dark);
        s.circle(d * 120.0, 6.0, glow(0.4, 1.8, 2.4));
    }
    s.ring(Vec2::ZERO, 150.0, 158.0, trim);
    s.regular(Vec2::ZERO, 70.0, 6, 0.0, hull_dark);
    s.ring(Vec2::ZERO, 44.0, 56.0, trim);
    s.circle(Vec2::ZERO, 42.0, c(0.15, 0.1, 0.25));
    s.circle(Vec2::ZERO, 26.0, glow(2.2, 0.6, 3.2));
    s
}

fn boss_turret() -> Shape {
    let mut s = Shape::new();
    s.regular(Vec2::ZERO, 30.0, 8, 0.39, c(0.22, 0.23, 0.27));
    s.regular(Vec2::ZERO, 26.0, 8, 0.39, c(0.46, 0.47, 0.52));
    s.ring(Vec2::ZERO, 16.0, 19.0, c(0.26, 0.27, 0.3));
    s
}

fn boss_barrel() -> Shape {
    let mut s = Shape::new();
    s.rect_pair(Vec2::new(5.0, 18.0), Vec2::new(6.0, 30.0), c(0.16, 0.16, 0.18));
    s.rect_pair(Vec2::new(5.0, 33.0), Vec2::new(8.0, 4.0), c(0.1, 0.1, 0.12));
    s.circle(Vec2::ZERO, 14.0, c(0.34, 0.35, 0.4));
    s.circle(Vec2::ZERO, 6.0, glow(3.0, 0.6, 0.3));
    s
}

// ---------------------------------------------------------------------------
// Pickups.
// ---------------------------------------------------------------------------

fn star(scale: f32) -> Shape {
    let mut s = Shape::new();
    s.star(Vec2::ZERO, 14.0 * scale, 6.5 * scale, 5, c(0.95, 0.55, 0.08));
    s.star(Vec2::ZERO, 10.0 * scale, 4.6 * scale, 5, glow(2.4, 1.8, 0.35));
    s.circle(Vec2::new(-2.0, 2.5) * scale, 1.6 * scale, glow(3.0, 3.0, 2.5));
    s
}

fn icon_weapon() -> Shape {
    let mut s = Shape::new();
    for y in [-6.0, 4.0] {
        s.poly(
            &pts(&[(0.0, y + 10.0), (11.0, y), (11.0, y - 5.0), (0.0, y + 5.0), (-11.0, y - 5.0), (-11.0, y)], 1.0),
            Color::WHITE,
        );
    }
    s
}

fn icon_shield() -> Shape {
    let mut s = Shape::new();
    let outer = pts(&[(0.0, 13.0), (11.0, 9.0), (10.0, -2.0), (0.0, -13.0)], 1.0);
    s.sym(&outer, Color::WHITE);
    s.sym(&pts(&[(0.0, 9.0), (7.0, 6.0), (6.5, -1.5), (0.0, -8.5)], 1.0), c(0.2, 0.5, 0.9));
    s
}

fn icon_repair() -> Shape {
    let mut s = Shape::new();
    s.rect(Vec2::ZERO, Vec2::new(22.0, 7.0), Color::WHITE);
    s.rect(Vec2::ZERO, Vec2::new(7.0, 22.0), Color::WHITE);
    s
}

fn icon_bomb() -> Shape {
    let mut s = Shape::new();
    s.circle(Vec2::new(0.0, -2.0), 9.5, Color::WHITE);
    s.rect(Vec2::new(5.0, 8.0), Vec2::new(5.0, 6.0), Color::WHITE);
    s.circle(Vec2::new(8.0, 12.0), 2.5, glow(3.0, 2.0, 0.4));
    s.circle(Vec2::new(-3.0, 1.0), 2.5, c(0.7, 0.2, 0.2));
    s
}

fn survivor() -> Shape {
    let mut s = Shape::new();
    s.circle(Vec2::ZERO, 15.0, c(1.0, 1.0, 1.0).with_alpha(0.35));
    s.ring(Vec2::ZERO, 13.0, 15.0, c(1.0, 1.0, 1.0));
    s.line(Vec2::new(-3.0, 3.0), Vec2::new(-8.0, 11.0), 2.4, c(0.95, 0.75, 0.55));
    s.line(Vec2::new(3.0, 3.0), Vec2::new(8.0, 11.0), 2.4, c(0.95, 0.75, 0.55));
    s.line(Vec2::new(-2.0, -4.0), Vec2::new(-3.0, -11.0), 2.6, c(0.2, 0.25, 0.4));
    s.line(Vec2::new(2.0, -4.0), Vec2::new(3.0, -11.0), 2.6, c(0.2, 0.25, 0.4));
    s.rect(Vec2::new(0.0, 0.0), Vec2::new(7.0, 9.0), c(1.0, 0.82, 0.1));
    s.circle(Vec2::new(0.0, 7.5), 3.2, c(0.95, 0.75, 0.55));
    s
}
