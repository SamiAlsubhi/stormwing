//! A small vector-art toolkit: build flat-shaded, vertex-coloured 2D meshes out
//! of polygons, circles, rings and lines. All of the game's planes, islands and
//! pickups are drawn with this, so the game ships without any image assets.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::TAU;

/// Converts a colour to the linear RGBA floats stored in the vertex buffer.
/// Values above 1.0 are kept, which makes those pixels glow through bloom.
pub fn lin(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
}

/// Bright HDR colour used for glowing details (canopies, engines, cores).
pub fn glow(r: f32, g: f32, b: f32) -> Color {
    Color::linear_rgb(r, g, b)
}

#[derive(Default, Clone)]
pub struct Shape {
    pos: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
    z: f32,
}

impl Shape {
    pub fn new() -> Self {
        Self::default()
    }

    fn vertex(&mut self, p: Vec2, c: [f32; 4]) -> u32 {
        self.pos.push([p.x, p.y, self.z]);
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    /// Every primitive sits a hair above the previous one so later shapes are
    /// always painted on top, regardless of how the GPU orders triangles.
    fn next_layer(&mut self) {
        self.z += 0.0004;
    }

    pub fn poly(&mut self, pts: &[Vec2], color: Color) -> &mut Self {
        if pts.len() < 3 {
            return self;
        }
        let c = lin(color);
        let base = self.pos.len() as u32;
        for &p in pts {
            self.vertex(p, c);
        }
        for [a, b, d] in triangulate(pts) {
            self.idx
                .extend([base + a as u32, base + b as u32, base + d as u32]);
        }
        self.next_layer();
        self
    }

    /// Symmetric polygon: `half` runs down the right-hand side (x >= 0) and is
    /// mirrored to produce the left-hand side.
    pub fn sym(&mut self, half: &[Vec2], color: Color) -> &mut Self {
        let mut pts: Vec<Vec2> = half.to_vec();
        for p in half.iter().rev() {
            if p.x.abs() > 1e-4 {
                pts.push(Vec2::new(-p.x, p.y));
            }
        }
        self.poly(&pts, color)
    }

    /// Draws a polygon and its mirror image across the y axis.
    pub fn pair(&mut self, pts: &[Vec2], color: Color) -> &mut Self {
        self.poly(pts, color);
        let mirrored: Vec<Vec2> = pts.iter().map(|p| Vec2::new(-p.x, p.y)).collect();
        self.poly(&mirrored, color)
    }

    pub fn circle(&mut self, c: Vec2, r: f32, color: Color) -> &mut Self {
        self.ellipse(c, Vec2::splat(r), color)
    }

    pub fn circle_pair(&mut self, c: Vec2, r: f32, color: Color) -> &mut Self {
        self.circle(c, r, color);
        self.circle(Vec2::new(-c.x, c.y), r, color)
    }

    pub fn ellipse(&mut self, c: Vec2, r: Vec2, color: Color) -> &mut Self {
        let segs = ((r.max_element() * 0.9) as usize).clamp(12, 56);
        let col = lin(color);
        let center = self.vertex(c, col);
        for i in 0..segs {
            let a = i as f32 / segs as f32 * TAU;
            self.vertex(c + Vec2::new(a.cos() * r.x, a.sin() * r.y), col);
        }
        for i in 0..segs {
            let a = center + 1 + i as u32;
            let b = center + 1 + ((i + 1) % segs) as u32;
            self.idx.extend([center, a, b]);
        }
        self.next_layer();
        self
    }

    pub fn ring(&mut self, c: Vec2, r_in: f32, r_out: f32, color: Color) -> &mut Self {
        let segs = ((r_out * 0.9) as usize).clamp(16, 64);
        let col = lin(color);
        let base = self.pos.len() as u32;
        for i in 0..segs {
            let d = Vec2::from_angle(i as f32 / segs as f32 * TAU);
            self.vertex(c + d * r_in, col);
            self.vertex(c + d * r_out, col);
        }
        for i in 0..segs as u32 {
            let j = (i + 1) % segs as u32;
            let (i0, o0, i1, o1) = (base + i * 2, base + i * 2 + 1, base + j * 2, base + j * 2 + 1);
            self.idx.extend([i0, o0, o1, i0, o1, i1]);
        }
        self.next_layer();
        self
    }

    pub fn rect(&mut self, c: Vec2, size: Vec2, color: Color) -> &mut Self {
        self.rot_rect(c, size, 0.0, color)
    }

    pub fn rect_pair(&mut self, c: Vec2, size: Vec2, color: Color) -> &mut Self {
        self.rect(c, size, color);
        self.rect(Vec2::new(-c.x, c.y), size, color)
    }

    pub fn rot_rect(&mut self, c: Vec2, size: Vec2, angle: f32, color: Color) -> &mut Self {
        let h = size * 0.5;
        let rot = Vec2::from_angle(angle);
        let pts = [
            Vec2::new(-h.x, -h.y),
            Vec2::new(h.x, -h.y),
            Vec2::new(h.x, h.y),
            Vec2::new(-h.x, h.y),
        ]
        .map(|p| c + rot.rotate(p));
        self.poly(&pts, color)
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: Color) -> &mut Self {
        let d = (b - a).normalize_or_zero().perp() * width * 0.5;
        self.poly(&[a - d, b - d, b + d, a + d], color)
    }

    pub fn regular(&mut self, c: Vec2, r: f32, sides: usize, rot: f32, color: Color) -> &mut Self {
        let pts: Vec<Vec2> = (0..sides)
            .map(|i| c + Vec2::from_angle(rot + i as f32 / sides as f32 * TAU) * r)
            .collect();
        self.poly(&pts, color)
    }

    pub fn star(&mut self, c: Vec2, r_out: f32, r_in: f32, points: usize, color: Color) -> &mut Self {
        let pts: Vec<Vec2> = (0..points * 2)
            .map(|i| {
                let r = if i % 2 == 0 { r_out } else { r_in };
                let a = std::f32::consts::FRAC_PI_2 + i as f32 / (points * 2) as f32 * TAU;
                c + Vec2::from_angle(a) * r
            })
            .collect();
        self.poly(&pts, color)
    }

    pub fn build(&self) -> Mesh {
        let n = self.pos.len();
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; n])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n])
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col.clone())
            .with_inserted_indices(Indices::U32(self.idx.clone()))
    }
}

/// Converts a list of `(x, y)` pairs into points, scaled.
pub fn pts(raw: &[(f32, f32)], scale: f32) -> Vec<Vec2> {
    raw.iter().map(|&(x, y)| Vec2::new(x, y) * scale).collect()
}

/// Ear-clipping triangulation for simple (non self-intersecting) polygons of
/// either winding. Output triangles are counter-clockwise.
pub fn triangulate(pts: &[Vec2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum();
    let mut ring: Vec<usize> = if area >= 0.0 {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };

    let mut tris = Vec::with_capacity(n - 2);
    while ring.len() > 3 {
        let m = ring.len();
        let mut clipped = false;
        for i in 0..m {
            let ia = ring[(i + m - 1) % m];
            let ib = ring[i];
            let ic = ring[(i + 1) % m];
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if (b - a).perp_dot(c - b) <= 1e-7 {
                continue; // reflex or degenerate corner
            }
            let blocked = ring.iter().any(|&j| {
                j != ia && j != ib && j != ic && point_in_triangle(pts[j], a, b, c)
            });
            if blocked {
                continue;
            }
            tris.push([ia, ib, ic]);
            ring.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // Degenerate input: fall back to a fan so we still draw something.
            for k in 1..ring.len() - 1 {
                tris.push([ring[0], ring[k], ring[k + 1]]);
            }
            return tris;
        }
    }
    tris.push([ring[0], ring[1], ring[2]]);
    tris
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = (b - a).perp_dot(p - a);
    let d2 = (c - b).perp_dot(p - b);
    let d3 = (a - c).perp_dot(p - c);
    d1 > 1e-6 && d2 > 1e-6 && d3 > 1e-6
}
