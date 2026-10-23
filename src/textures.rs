//! Procedurally generated textures for glows, bullets, clouds and UI icons.

use crate::rng::Rng;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Builds an RGBA image by evaluating `f(u, v)` with `u, v` in `[-1, 1]`
/// (`v` pointing up). `f` returns straight (non-premultiplied) sRGB + alpha.
fn make(w: u32, h: u32, f: impl Fn(f32, f32) -> [f32; 4]) -> Image {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let v = 1.0 - (y as f32 + 0.5) / h as f32 * 2.0;
            let px = f(u, v);
            data.extend(px.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
        }
    }
    Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Soft radial glow, used for particles, flashes and engine flames.
pub fn soft_glow() -> Image {
    make(64, 64, |u, v| {
        let d = (u * u + v * v).sqrt();
        let a = (1.0 - d).max(0.0).powf(1.8);
        [1.0, 1.0, 1.0, a]
    })
}

/// Enemy bullet: bright core with a darker rim so it reads on any background.
pub fn orb() -> Image {
    make(64, 64, |u, v| {
        let d = (u * u + v * v).sqrt();
        let core = 1.0 - smoothstep(0.35, 0.62, d);
        let shade = 0.22 + 0.78 * core;
        let alpha = 1.0 - smoothstep(0.82, 0.98, d);
        [shade, shade, shade, alpha]
    })
}

/// Crisp anti-aliased disc.
pub fn disc() -> Image {
    make(64, 64, |u, v| {
        let d = (u * u + v * v).sqrt();
        [1.0, 1.0, 1.0, 1.0 - smoothstep(0.9, 1.0, d)]
    })
}

/// Soft ring for shockwaves and pickup halos.
pub fn ring() -> Image {
    make(128, 128, |u, v| {
        let d = (u * u + v * v).sqrt();
        let a = (1.0 - ((d - 0.85) / 0.13).abs()).max(0.0).powf(1.5);
        [1.0, 1.0, 1.0, a]
    })
}

/// Long thin glow for tracers and sparks.
pub fn streak() -> Image {
    make(32, 96, |u, v| {
        let across = (1.0 - u.abs()).max(0.0).powf(1.6);
        let along = (1.0 - v.abs()).max(0.0).powf(0.6);
        let core = (1.0 - u.abs() * 2.5).max(0.0);
        let a = (across * along).max(core * along);
        [1.0, 1.0, 1.0, a]
    })
}

/// A fluffy cloud made of overlapping soft puffs.
pub fn cloud(seed: u64) -> Image {
    let mut rng = Rng::new(seed);
    let puffs: Vec<(Vec2, f32)> = (0..9)
        .map(|_| {
            let c = Vec2::new(rng.range(-0.45, 0.45), rng.range(-0.25, 0.25));
            (c, rng.range(0.28, 0.5))
        })
        .collect();
    make(192, 128, move |u, v| {
        let p = Vec2::new(u, v * 0.66);
        let mut density = 0.0f32;
        let mut shade = 0.0f32;
        for (c, r) in &puffs {
            let d = (p - *c).length() / r;
            let k = (1.0 - d).max(0.0);
            density += k * k;
            // Light comes from the top-left: brighter on that side of each puff.
            shade += k * k * (0.85 + 0.15 * ((p - *c).dot(Vec2::new(-0.6, 0.8)) / r).clamp(-1.0, 1.0));
        }
        let a = smoothstep(0.05, 0.7, density);
        let s = if density > 0.0 { (shade / density).clamp(0.86, 1.0) } else { 1.0 };
        [s, s, s, a]
    })
}

/// Gold five-pointed star for the HUD.
pub fn star_icon() -> Image {
    make(64, 64, |u, v| {
        let d = star_sdf(Vec2::new(u, v) * 1.1);
        let fill = 1.0 - smoothstep(-0.02, 0.02, d);
        let edge = 1.0 - smoothstep(0.03, 0.08, d);
        let inner = 1.0 - smoothstep(-0.16, -0.1, d);
        let col = if inner > 0.5 { [1.0, 0.88, 0.3] } else { [0.95, 0.62, 0.1] };
        let rgb = if fill > 0.0 { col } else { [0.35, 0.2, 0.05] };
        [rgb[0], rgb[1], rgb[2], edge.max(fill)]
    })
}

/// Medal badge: a disc with a star cut into it. Tinted gold or grey in the UI.
pub fn medal_icon() -> Image {
    make(96, 96, |u, v| {
        let d = (u * u + v * v).sqrt();
        let disc = 1.0 - smoothstep(0.88, 0.96, d);
        let rim = smoothstep(0.72, 0.78, d);
        let star = 1.0 - smoothstep(-0.02, 0.03, star_sdf(Vec2::new(u, v) * 1.75));
        let shade = 0.55 + 0.45 * rim.max(star);
        [shade, shade, shade, disc]
    })
}

/// Approximate signed distance to a five-pointed star of radius ~0.9.
fn star_sdf(p: Vec2) -> f32 {
    // Polygon distance via edges of the 10-point outline.
    let n = 10;
    let pts: Vec<Vec2> = (0..n)
        .map(|i| {
            let r = if i % 2 == 0 { 0.92 } else { 0.4 };
            let a = std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
            Vec2::from_angle(a) * r
        })
        .collect();
    let mut dist = f32::MAX;
    let mut inside = false;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        dist = dist.min((p - (a + ab * t)).length());
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    if inside { -dist } else { dist }
}
