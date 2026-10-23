//! Tiny deterministic random number generator (xorshift64*).
//!
//! Stages are generated from a fixed seed so every attempt at a stage has the
//! same enemy count, which keeps the "destroy 100%" medal fair.

use bevy::prelude::*;
use std::f32::consts::TAU;

#[derive(Resource, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self((seed ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn from_time() -> Self {
        // `SystemTime` panics on wasm32-unknown-unknown, so ask the browser.
        #[cfg(target_arch = "wasm32")]
        let seed = (js_sys::Date::now() * 1000.0) as u64 ^ (js_sys::Math::random() * u32::MAX as f64) as u64;
        #[cfg(not(target_arch = "wasm32"))]
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678);
        Self::new(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform integer in `lo..=hi`.
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u64() % ((hi - lo + 1).max(1) as u64)) as i32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn dir(&mut self) -> Vec2 {
        Vec2::from_angle(self.f32() * TAU)
    }

    /// Random point inside a disc.
    pub fn in_disc(&mut self, radius: f32) -> Vec2 {
        self.dir() * radius * self.f32().sqrt()
    }
}
