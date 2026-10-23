//! A small procedural 3D modelling toolkit. Every aircraft, building, tree
//! and pickup in the game is assembled from these primitives at startup.
//!
//! Conventions: models face +Y (nose up the screen), +Z points up towards
//! the camera and +X is the right wing.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::TAU;

pub fn lin(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
}

pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::srgb(r, g, b)
}

/// HDR colour for glowing parts (values above 1 bloom).
pub fn hdr(r: f32, g: f32, b: f32) -> Color {
    Color::linear_rgb(r, g, b)
}

pub fn shade(c: Color, k: f32) -> Color {
    let l = c.to_linear();
    Color::linear_rgb(l.red * k, l.green * k, l.blue * k)
}

#[derive(Default, Clone)]
pub struct Model {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
    /// Texture coordinates; only filled in by [`Model::vertex_uv`] (terrain).
    uv: Vec<[f32; 2]>,
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        // Once texture coordinates are in use, keep them in step.
        if !self.uv.is_empty() {
            self.uv.push([0.0, 0.0]);
        }
        self.push(p, n, c)
    }

    fn push(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        self.pos.push(p.to_array());
        self.nor.push(n.normalize_or(Vec3::Z).to_array());
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    pub fn vertex_uv(&mut self, p: Vec3, n: Vec3, c: [f32; 4], uv: Vec2) -> u32 {
        self.uv.resize(self.pos.len(), [0.0, 0.0]);
        self.uv.push(uv.to_array());
        self.push(p, n, c)
    }

    pub fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.idx.extend([a, b, c]);
    }

    /// Flat-shaded triangle; counter-clockwise when seen from the front.
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) -> &mut Self {
        let n = (b - a).cross(c - a);
        let col = lin(color);
        let i = self.vertex(a, n, col);
        let j = self.vertex(b, n, col);
        let k = self.vertex(c, n, col);
        self.triangle(i, j, k);
        self
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: Color) -> &mut Self {
        let n = (b - a).cross(c - a) + (c - a).cross(d - a);
        let col = lin(color);
        let i = [a, b, c, d].map(|p| self.vertex(p, n, col));
        self.triangle(i[0], i[1], i[2]);
        self.triangle(i[0], i[2], i[3]);
        self
    }

    /// Smooth body lofted along +Y through elliptical cross-sections given as
    /// `(y, half_width, half_height, z_offset)`, ordered from tail to nose.
    /// `color(normal, y)` picks the paint, e.g. a darker belly.
    pub fn loft(&mut self, sections: &[(f32, f32, f32, f32)], segs: usize, color: impl Fn(Vec3, f32) -> Color) -> &mut Self {
        let ring = segs + 1;
        let base = self.pos.len() as u32;
        for &(y, hw, hh, zo) in sections {
            for j in 0..ring {
                let a = j as f32 / segs as f32 * TAU;
                let (c, s) = (a.cos(), a.sin());
                let p = Vec3::new(c * hw, y, zo + s * hh);
                let n = Vec3::new(c / hw.max(0.01), 0.0, s / hh.max(0.01)).normalize_or(Vec3::Z);
                let col = lin(color(n, y));
                self.vertex(p, n, col);
            }
        }
        for i in 0..sections.len() - 1 {
            for j in 0..segs {
                let a = base + (i * ring + j) as u32;
                let b = a + 1;
                let d = a + ring as u32;
                let c = d + 1;
                self.triangle(a, c, b);
                self.triangle(a, d, c);
            }
        }
        // Close both ends.
        for (end, flip) in [(0usize, true), (sections.len() - 1, false)] {
            let (y, hw, hh, zo) = sections[end];
            if hw < 0.05 && hh < 0.05 {
                continue;
            }
            let n = if flip { Vec3::NEG_Y } else { Vec3::Y };
            let col = lin(color(n, y));
            let centre = self.vertex(Vec3::new(0.0, y, zo), n, col);
            let start = self.pos.len() as u32;
            for j in 0..ring {
                let a = j as f32 / segs as f32 * TAU;
                self.vertex(Vec3::new(a.cos() * hw, y, zo + a.sin() * hh), n, col);
            }
            for j in 0..segs as u32 {
                if flip {
                    self.triangle(centre, start + j, start + j + 1);
                } else {
                    self.triangle(centre, start + j + 1, start + j);
                }
            }
        }
        self
    }

    /// Prism from a 2D outline in the XY plane, spanning `z0..z1`.
    pub fn extrude(&mut self, poly: &[Vec2], z0: f32, z1: f32, top: Color, side: Color) -> &mut Self {
        self.extrude_tf(poly, z0, z1, top, side, Mat3::IDENTITY, Vec3::ZERO)
    }

    /// Like [`Model::extrude`], then rotated by `m` and moved by `offset`.
    pub fn extrude_tf(
        &mut self,
        poly: &[Vec2],
        z0: f32,
        z1: f32,
        top: Color,
        side: Color,
        m: Mat3,
        offset: Vec3,
    ) -> &mut Self {
        if poly.len() < 3 {
            return self;
        }
        let mut pts = poly.to_vec();
        if signed_area(&pts) < 0.0 {
            pts.reverse();
        }
        let tf = |p: Vec3| m * p + offset;
        let tn = |n: Vec3| (m * n).normalize_or(Vec3::Z);
        let tris = triangulate(&pts);
        // Top and bottom caps.
        for (z, n, color, flip) in [(z1, Vec3::Z, top, false), (z0, Vec3::NEG_Z, shade(side, 0.7), true)] {
            let col = lin(color);
            let base = self.pos.len() as u32;
            for p in &pts {
                self.vertex(tf(p.extend(z)), tn(n), col);
            }
            for [a, b, c] in &tris {
                let (a, b, c) = (base + *a as u32, base + *b as u32, base + *c as u32);
                if flip {
                    self.triangle(a, c, b);
                } else {
                    self.triangle(a, b, c);
                }
            }
        }
        // Side walls.
        let col = lin(side);
        for i in 0..pts.len() {
            let p = pts[i];
            let q = pts[(i + 1) % pts.len()];
            let e = q - p;
            let n = tn(Vec3::new(e.y, -e.x, 0.0));
            let a = self.vertex(tf(p.extend(z0)), n, col);
            let b = self.vertex(tf(q.extend(z0)), n, col);
            let c = self.vertex(tf(q.extend(z1)), n, col);
            let d = self.vertex(tf(p.extend(z1)), n, col);
            self.triangle(a, b, c);
            self.triangle(a, c, d);
        }
        self
    }

    /// Thin wing panel from a planform outline, centred at height `z`.
    pub fn wing(&mut self, poly: &[Vec2], z: f32, thickness: f32, color: Color) -> &mut Self {
        self.extrude(poly, z - thickness / 2.0, z + thickness / 2.0, color, shade(color, 0.75))
    }

    /// Vertical fin from an outline in the (y, z) plane, `thickness` wide in x,
    /// leaning outwards by `cant` radians.
    pub fn fin(&mut self, outline_yz: &[Vec2], x: f32, thickness: f32, cant: f32, color: Color) -> &mut Self {
        // Local (u, v, w) = (y, z, x) -> world (x, y, z), then cant around the y axis.
        let m = Mat3::from_rotation_y(cant) * Mat3::from_cols(Vec3::Y, Vec3::Z, Vec3::X);
        self.extrude_tf(outline_yz, -thickness / 2.0, thickness / 2.0, color, shade(color, 0.85), m, Vec3::new(x, 0.0, 0.0))
    }

    pub fn ellipsoid(&mut self, c: Vec3, r: Vec3, color: Color, top_only: bool) -> &mut Self {
        let (su, sv) = (14usize, 8usize);
        let col = lin(color);
        let base = self.pos.len() as u32;
        let v_end = if top_only { sv / 2 } else { sv };
        for i in 0..=v_end {
            let phi = std::f32::consts::PI * (0.5 - i as f32 / sv as f32);
            for j in 0..=su {
                let th = j as f32 / su as f32 * TAU;
                let d = Vec3::new(phi.cos() * th.cos(), phi.cos() * th.sin(), phi.sin());
                let n = Vec3::new(d.x / r.x, d.y / r.y, d.z / r.z);
                self.vertex(c + d * r, n, col);
            }
        }
        for i in 0..v_end as u32 {
            for j in 0..su as u32 {
                let a = base + i * (su as u32 + 1) + j;
                let b = a + 1;
                let d = a + su as u32 + 1;
                let e = d + 1;
                self.triangle(a, e, b);
                self.triangle(a, d, e);
            }
        }
        self
    }

    /// Chunky, flat-shaded boulder: a coarse sphere with jittered vertices.
    /// `jitter` returns a radius multiplier for each of its inputs.
    pub fn rock(&mut self, c: Vec3, r: Vec3, color: Color, mut jitter: impl FnMut() -> f32) -> &mut Self {
        let (su, sv) = (7usize, 4usize);
        let mut grid = vec![Vec3::ZERO; (su + 1) * (sv + 1)];
        for i in 0..=sv {
            let phi = std::f32::consts::PI * (0.5 - i as f32 / sv as f32);
            for j in 0..=su {
                let th = (j % su) as f32 / su as f32 * TAU;
                let d = Vec3::new(phi.cos() * th.cos(), phi.cos() * th.sin(), phi.sin());
                // Seam and poles share a single jitter so the rock stays closed.
                let k = if i == 0 || i == sv { 1.0 } else if j == su { 0.0 } else { jitter() };
                grid[i * (su + 1) + j] = d * r * k;
            }
            grid[i * (su + 1) + su] = grid[i * (su + 1)];
        }
        // Flatten the bottom so rocks sit on the ground.
        for v in &mut grid {
            v.z = v.z.max(-r.z * 0.35);
        }
        for i in 0..sv {
            for j in 0..su {
                let a = grid[i * (su + 1) + j] + c;
                let b = grid[i * (su + 1) + j + 1] + c;
                let d = grid[(i + 1) * (su + 1) + j] + c;
                let e = grid[(i + 1) * (su + 1) + j + 1] + c;
                let tone = 0.85 + 0.3 * ((i * 7 + j * 3) % 5) as f32 / 4.0;
                let col = shade(color, tone);
                self.tri(a, e, b, col);
                self.tri(a, d, e, col);
            }
        }
        self
    }

    /// Cylinder from `a` to `b`. Set `cap` to close the ends.
    pub fn cylinder(&mut self, a: Vec3, b: Vec3, radius: f32, segs: usize, color: Color, cap: bool) -> &mut Self {
        self.frustum(a, b, radius, radius, segs, color, cap)
    }

    pub fn cone(&mut self, base: Vec3, tip: Vec3, radius: f32, segs: usize, color: Color) -> &mut Self {
        self.frustum(base, tip, radius, 0.0, segs, color, true)
    }

    /// Truncated cone from `a` (radius `ra`) to `b` (radius `rb`).
    pub fn frustum(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, segs: usize, color: Color, cap: bool) -> &mut Self {
        let axis = (b - a).normalize_or(Vec3::Z);
        let u = axis.any_orthonormal_vector();
        let v = axis.cross(u);
        let col = lin(color);
        let slope = (ra - rb) / (b - a).length().max(0.001);
        let base = self.pos.len() as u32;
        for j in 0..=segs {
            let t = j as f32 / segs as f32 * TAU;
            let d = u * t.cos() + v * t.sin();
            let n = d + axis * slope;
            self.vertex(a + d * ra, n, col);
            self.vertex(b + d * rb, n, col);
        }
        for j in 0..segs as u32 {
            let (a0, b0, a1, b1) = (base + j * 2, base + j * 2 + 1, base + j * 2 + 2, base + j * 2 + 3);
            self.triangle(a0, a1, b1);
            self.triangle(a0, b1, b0);
        }
        if cap {
            for (centre, r, n) in [(a, ra, -axis), (b, rb, axis)] {
                if r <= 0.01 {
                    continue;
                }
                let ccol = lin(shade(color, if n.z > 0.5 { 1.05 } else { 0.8 }));
                let c = self.vertex(centre, n, ccol);
                let start = self.pos.len() as u32;
                for j in 0..=segs {
                    let t = j as f32 / segs as f32 * TAU;
                    self.vertex(centre + (u * t.cos() + v * t.sin()) * r, n, ccol);
                }
                for j in 0..segs as u32 {
                    if n.dot(axis) > 0.0 {
                        self.triangle(c, start + j, start + j + 1);
                    } else {
                        self.triangle(c, start + j + 1, start + j);
                    }
                }
            }
        }
        self
    }

    /// Axis-aligned box centred at `c`.
    pub fn cuboid(&mut self, c: Vec3, size: Vec3, color: Color) -> &mut Self {
        self.box_tf(c, size, Quat::IDENTITY, color)
    }

    pub fn box_tf(&mut self, c: Vec3, size: Vec3, rot: Quat, color: Color) -> &mut Self {
        let h = size / 2.0;
        let p = |x: f32, y: f32, z: f32| c + rot * Vec3::new(x * h.x, y * h.y, z * h.z);
        let top = shade(color, 1.0);
        let side = shade(color, 0.82);
        let bottom = shade(color, 0.6);
        self.quad(p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.), top);
        self.quad(p(-1., 1., -1.), p(1., 1., -1.), p(1., -1., -1.), p(-1., -1., -1.), bottom);
        self.quad(p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.), side);
        self.quad(p(1., 1., -1.), p(-1., 1., -1.), p(-1., 1., 1.), p(1., 1., 1.), side);
        self.quad(p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(1., -1., 1.), side);
        self.quad(p(-1., 1., -1.), p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), side);
        self
    }

    /// Gable roof (a triangular prism) over a `size.x` by `size.y` footprint.
    pub fn roof(&mut self, c: Vec3, size: Vec2, height: f32, color: Color) -> &mut Self {
        let (hx, hy) = (size.x / 2.0, size.y / 2.0);
        let p = |x: f32, y: f32, z: f32| c + Vec3::new(x, y, z);
        let light = color;
        let dark = shade(color, 0.72);
        self.quad(p(-hx, -hy, 0.0), p(0.0, -hy, height), p(0.0, hy, height), p(-hx, hy, 0.0), dark);
        self.quad(p(hx, hy, 0.0), p(0.0, hy, height), p(0.0, -hy, height), p(hx, -hy, 0.0), light);
        self.tri(p(-hx, -hy, 0.0), p(hx, -hy, 0.0), p(0.0, -hy, height), shade(color, 0.6));
        self.tri(p(hx, hy, 0.0), p(-hx, hy, 0.0), p(0.0, hy, height), shade(color, 0.6));
        self
    }

    /// Adds another model, transformed.
    pub fn append(&mut self, other: &Model, t: Transform) -> &mut Self {
        let base = self.pos.len() as u32;
        let m = t.to_matrix();
        for i in 0..other.pos.len() {
            let p = m.transform_point3(Vec3::from_array(other.pos[i]));
            let n = (t.rotation * Vec3::from_array(other.nor[i])).normalize_or(Vec3::Z);
            self.pos.push(p.to_array());
            self.nor.push(n.to_array());
            self.col.push(other.col[i]);
        }
        self.idx.extend(other.idx.iter().map(|i| i + base));
        if !self.uv.is_empty() {
            self.uv.resize(self.pos.len(), [0.0, 0.0]);
        }
        self
    }

    /// Adds a mirror image of `other` across the YZ plane (left/right).
    pub fn append_mirror_x(&mut self, other: &Model) -> &mut Self {
        let base = self.pos.len() as u32;
        for i in 0..other.pos.len() {
            let [x, y, z] = other.pos[i];
            let [nx, ny, nz] = other.nor[i];
            self.pos.push([-x, y, z]);
            self.nor.push([-nx, ny, nz]);
            self.col.push(other.col[i]);
        }
        for t in other.idx.chunks(3) {
            self.idx.extend([t[0] + base, t[2] + base, t[1] + base]);
        }
        if !self.uv.is_empty() {
            self.uv.resize(self.pos.len(), [0.0, 0.0]);
        }
        self
    }

    /// Builds `part` with `f` and adds it twice: as is and mirrored.
    pub fn symmetric(&mut self, f: impl FnOnce(&mut Model)) -> &mut Self {
        let mut part = Model::new();
        f(&mut part);
        self.append(&part, Transform::IDENTITY);
        self.append_mirror_x(&part)
    }

    /// Recolours every vertex from `start` on using its position and normal.
    pub fn recolor_from(&mut self, start: usize, f: impl Fn(Vec3, Vec3) -> Color) {
        for i in start..self.pos.len() {
            let c = f(Vec3::from_array(self.nor[i]), Vec3::from_array(self.pos[i]));
            self.col[i] = lin(c);
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.pos.len()
    }

    pub fn build(&self) -> Mesh {
        let n = self.pos.len();
        let uv = if self.uv.len() == n { self.uv.clone() } else { vec![[0.0f32, 0.0]; n] };
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col.clone())
            .with_inserted_indices(Indices::U32(self.idx.clone()))
    }
}

/// Converts `(x, y)` pairs to points.
pub fn pts(raw: &[(f32, f32)]) -> Vec<Vec2> {
    raw.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
}

/// Points of a regular polygon.
pub fn ngon(c: Vec2, r: f32, sides: usize, rot: f32) -> Vec<Vec2> {
    (0..sides)
        .map(|i| c + Vec2::from_angle(rot + i as f32 / sides as f32 * TAU) * r)
        .collect()
}

/// Points of a star outline.
pub fn star_outline(r_out: f32, r_in: f32, points: usize) -> Vec<Vec2> {
    (0..points * 2)
        .map(|i| {
            let r = if i % 2 == 0 { r_out } else { r_in };
            let a = std::f32::consts::FRAC_PI_2 + i as f32 / (points * 2) as f32 * TAU;
            Vec2::from_angle(a) * r
        })
        .collect()
}

fn signed_area(pts: &[Vec2]) -> f32 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5
}

/// Ear-clipping triangulation for simple polygons of either winding.
/// Output triangles are counter-clockwise.
pub fn triangulate(pts: &[Vec2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let mut ring: Vec<usize> = if signed_area(pts) >= 0.0 {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };
    let mut tris = Vec::with_capacity(n - 2);
    while ring.len() > 3 {
        let m = ring.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if (b - a).perp_dot(c - b) <= 1e-7 {
                continue;
            }
            let blocked = ring
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && point_in_triangle(pts[j], a, b, c));
            if blocked {
                continue;
            }
            tris.push([ia, ib, ic]);
            ring.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
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
