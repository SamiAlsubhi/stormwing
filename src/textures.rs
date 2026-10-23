//! Procedurally generated textures: glows, bullets, clouds, water and sand
//! normal maps, and UI icons.

use crate::noise::{fbm, fbm_tiled};
use crate::rng::Rng;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Builds an RGBA image by evaluating `f(u, v)` with `u, v` in `[-1, 1]`
/// (`v` pointing up). Colour data is sRGB unless `linear` is set (normal maps).
fn make(w: u32, h: u32, linear: bool, f: impl Fn(f32, f32) -> [f32; 4]) -> Image {
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
        if linear { TextureFormat::Rgba8Unorm } else { TextureFormat::Rgba8UnormSrgb },
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn repeating(mut image: Image) -> Image {
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Soft radial glow, used for particles, flashes and engine flames.
pub fn soft_glow() -> Image {
    make(64, 64, false, |u, v| {
        let d = (u * u + v * v).sqrt();
        [1.0, 1.0, 1.0, (1.0 - d).max(0.0).powf(1.8)]
    })
}

/// Billowing smoke puff.
pub fn smoke() -> Image {
    make(64, 64, false, |u, v| {
        let d = (u * u + v * v).sqrt();
        let n = fbm(Vec2::new(u, v) * 3.0, 9, 4);
        let a = ((1.0 - d) * (0.6 + 0.8 * n)).clamp(0.0, 1.0).powf(1.4);
        let s = 0.75 + 0.25 * n;
        [s, s, s, a]
    })
}

/// Enemy bullet: bright core with a darker rim so it reads on any background.
pub fn orb() -> Image {
    make(64, 64, false, |u, v| {
        let d = (u * u + v * v).sqrt();
        let core = 1.0 - smoothstep(0.35, 0.62, d);
        let shade = 0.22 + 0.78 * core;
        [shade, shade, shade, 1.0 - smoothstep(0.82, 0.98, d)]
    })
}

/// Crisp anti-aliased disc.
pub fn disc() -> Image {
    make(64, 64, false, |u, v| {
        let d = (u * u + v * v).sqrt();
        [1.0, 1.0, 1.0, 1.0 - smoothstep(0.9, 1.0, d)]
    })
}

/// Soft ring for shockwaves and pickup halos.
pub fn ring() -> Image {
    make(128, 128, false, |u, v| {
        let d = (u * u + v * v).sqrt();
        let a = (1.0 - ((d - 0.85) / 0.13).abs()).max(0.0).powf(1.5);
        [1.0, 1.0, 1.0, a]
    })
}

/// Long thin glow for tracers and sparks.
pub fn streak() -> Image {
    make(32, 96, false, |u, v| {
        let across = (1.0 - u.abs()).max(0.0).powf(1.6);
        let along = (1.0 - v.abs()).max(0.0).powf(0.6);
        let core = (1.0 - u.abs() * 2.5).max(0.0);
        [1.0, 1.0, 1.0, (across * along).max(core * along)]
    })
}

/// A cumulus cloud seen from above: puffy outline, fractal detail and
/// lighting baked in (sunlit edges towards the top-left, shaded underside).
pub fn cloud(seed: u64) -> Image {
    let mut rng = Rng::new(seed);
    let puffs: Vec<(Vec2, f32)> = (0..11)
        .map(|_| (Vec2::new(rng.range(-0.5, 0.5), rng.range(-0.28, 0.28)), rng.range(0.22, 0.46)))
        .collect();
    let s = seed as u32;
    let density = move |p: Vec2| -> f32 {
        let mut shape = 0.0f32;
        for (c, r) in &puffs {
            let k = (1.0 - (p - *c).length() / r).max(0.0);
            shape += k * k;
        }
        let detail = fbm(p * 4.5 + Vec2::splat(s as f32 * 1.7), s, 5);
        (shape * (0.5 + 0.9 * detail) - 0.12).max(0.0)
    };
    let light = Vec2::new(-0.55, 0.75).normalize() * 0.05;
    make(256, 176, false, move |u, v| {
        let p = Vec2::new(u, v * 0.69);
        let d = density(p);
        let a = smoothstep(0.02, 0.38, d);
        if a <= 0.0 {
            return [1.0, 1.0, 1.0, 0.0];
        }
        // Sample towards the sun: denser there means this point is shaded.
        let towards = density(p + light);
        let lit = (1.0 - (towards - d) * 3.2).clamp(0.0, 1.0);
        let shadow = [0.66, 0.71, 0.82];
        let c = [
            shadow[0] + (1.0 - shadow[0]) * lit,
            shadow[1] + (1.0 - shadow[1]) * lit,
            shadow[2] + (1.0 - shadow[2]) * lit,
        ];
        [c[0], c[1], c[2], a]
    })
}

/// Encodes a tangent-space normal from height derivatives.
fn encode_normal(dx: f32, dy: f32, strength: f32) -> [f32; 4] {
    let n = Vec3::new(-dx * strength, -dy * strength, 1.0).normalize();
    [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0]
}

/// Tileable wave normal map for the sea surface.
pub fn water_normal() -> Image {
    let size = 256.0;
    let height = |u: f32, v: f32| -> f32 {
        let p = Vec2::new(u, v);
        fbm_tiled(p * 8.0, 31, 4, 8) + 0.35 * fbm_tiled(p * 24.0, 77, 2, 24)
    };
    let e = 1.0 / size;
    repeating(make(256, 256, true, move |u, v| {
        let (u, v) = (u * 0.5 + 0.5, v * 0.5 + 0.5);
        let dx = (height(u + e, v) - height(u - e, v)) / (2.0 * e);
        let dy = (height(u, v + e) - height(u, v - e)) / (2.0 * e);
        encode_normal(dx, dy, 0.0065)
    }))
}

/// Tileable wind-ripple normal map for sand.
pub fn sand_normal() -> Image {
    let size = 256.0;
    let height = |u: f32, v: f32| -> f32 {
        let p = Vec2::new(u, v);
        let warp = fbm_tiled(p * 4.0, 5, 3, 4) * 2.2;
        let ripple = ((v * 18.0 + warp) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        ripple * 0.6 + fbm_tiled(p * 16.0, 8, 3, 16) * 0.4
    };
    let e = 1.0 / size;
    repeating(make(256, 256, true, move |u, v| {
        let (u, v) = (u * 0.5 + 0.5, v * 0.5 + 0.5);
        let dx = (height(u + e, v) - height(u - e, v)) / (2.0 * e);
        let dy = (height(u, v + e) - height(u, v - e)) / (2.0 * e);
        encode_normal(dx, dy, 0.006)
    }))
}

/// Gold five-pointed star for the HUD.
pub fn star_icon() -> Image {
    make(64, 64, false, |u, v| {
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
    make(96, 96, false, |u, v| {
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
