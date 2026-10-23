//! Value noise and fractal Brownian motion, used for terrain, water and clouds.

use bevy::math::Vec2;

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

fn smooth(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Smooth value noise in `[0, 1]`. With `period > 0` the noise tiles every
/// `period` units, which lets textures repeat seamlessly.
pub fn value(p: Vec2, seed: u32, period: i32) -> f32 {
    let x0 = p.x.floor();
    let y0 = p.y.floor();
    let (fx, fy) = (smooth(p.x - x0), smooth(p.y - y0));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let wrap = |v: i32| if period > 0 { v.rem_euclid(period) } else { v };
    let h = |dx: i32, dy: i32| hash(wrap(ix + dx), wrap(iy + dy), seed);
    let a = h(0, 0) + (h(1, 0) - h(0, 0)) * fx;
    let b = h(0, 1) + (h(1, 1) - h(0, 1)) * fx;
    a + (b - a) * fy
}

/// Fractal noise in roughly `[0, 1]`.
pub fn fbm(p: Vec2, seed: u32, octaves: u32) -> f32 {
    fbm_tiled(p, seed, octaves, 0)
}

/// Fractal noise that tiles every `period` units at the base octave.
pub fn fbm_tiled(p: Vec2, seed: u32, octaves: u32, period: i32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    let mut freq = 1.0;
    let mut per = period;
    for o in 0..octaves {
        sum += amp * value(p * freq, seed.wrapping_add(o * 101), per);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
        per *= 2;
    }
    sum / norm
}
