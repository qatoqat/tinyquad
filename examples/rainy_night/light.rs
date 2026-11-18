//! Light transport through the voxel structure, baked once at start-up.
//!
//! Every surface voxel (a solid voxel with at least one open face) gets:
//! - sky light: how much of the night sky it sees, found by marching the
//!   height map in eight directions (so voxels under the bus-stop roof or
//!   at the foot of a building stay dark), times a local corner occlusion;
//! - point lights: a voxel ray (DDA) toward each light; opaque voxels block
//!   it and glass tints and dims it, so the bus-stop glass casts a blue-green
//!   light pattern and the roof a hard shadow; the street lamp is a downward
//!   spot, the advert a two-sided panel;
//! - a wetness flag for open tops under the rain (shade stays dry).
//!
//! Static lights never move, so the expensive part is done once; only
//! flicker and lightning scale the stored terms per frame. Moving things
//! (cat, floating boxes, rain) use `lighting_at` / the coarse light grid.

use crate::util::{Rgb, mul, scale, smoothstep};
use crate::world::{Light, Mat, World};
use glam::{Vec3, vec3};

pub const F_TOP: u8 = 1;
pub const F_WET: u8 = 2;
pub const F_EMISSIVE: u8 = 4;
pub const F_GLASS: u8 = 8;
pub const F_DYNAMIC: u8 = 16;
pub const F_CAT: u8 = 32;

/// Sky light color at full visibility, plus a weak bounce fill.
pub const SKY: Rgb = [0.25, 0.30, 0.50];
pub const FILL: Rgb = [0.06, 0.06, 0.09];

/// HDR light term packed in a byte: value = (b / 255)^2 * 4.
pub fn enc_hdr(v: f32) -> u8 {
    ((v.max(0.0) * 0.25).sqrt() * 255.0).round().min(255.0) as u8
}

pub fn enc_rgb_hdr(c: Rgb) -> [u8; 3] {
    [enc_hdr(c[0]), enc_hdr(c[1]), enc_hdr(c[2])]
}

pub fn enc_rgb_ldr(c: Rgb) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Decoding table for `enc_hdr`.
pub fn hdr_table() -> [f32; 256] {
    std::array::from_fn(|i| {
        let t = i as f32 / 255.0;
        t * t * 4.0
    })
}

/// Shading payload shared by baked voxels and per-frame splats.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct Shade {
    /// albedo * ambient (linear 0..1)
    pub amb: [u8; 3],
    /// albedo * street lamp (HDR)
    pub lamp: [u8; 3],
    /// albedo * every other light, or the emitted color (HDR)
    pub other: [u8; 3],
    pub flags: u8,
    /// wet tops: raw lamp intensity for reflections
    pub spec: u8,
    /// Interior wall/ceiling bits (`world::CUT_*`): hidden when facing the camera.
    pub cut: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Surf {
    pub pos: [u16; 3],
    pub shade: Shade,
}

pub struct Chunk {
    pub min: Vec3,
    pub max: Vec3,
    pub voxels: Vec<Surf>,
}

pub const CHUNK: i32 = 32;

/// Coarse light grid for moving things (cell = 8 voxels).
pub const GRID: i32 = 8;

pub struct Lighting {
    pub chunks: Vec<Chunk>,
    gw: i32,
    gh: i32,
    gd: i32,
    grid_lamp: Vec<Rgb>,
    grid_other: Vec<Rgb>,
}

const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
const DIRS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (S, S),
    (0.0, 1.0),
    (-S, S),
    (-1.0, 0.0),
    (-S, -S),
    (0.0, -1.0),
    (S, -S),
];

const FACES: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

impl Lighting {
    pub fn bake(world: &World) -> Lighting {
        let cx = (world.w + CHUNK - 1) / CHUNK;
        let cy = (world.h + CHUNK - 1) / CHUNK;
        let cz = (world.d + CHUNK - 1) / CHUNK;
        let mut chunks: Vec<Chunk> = (0..cx * cy * cz)
            .map(|i| {
                let (x, y, z) = (i % cx, (i / cx) % cy, i / (cx * cy));
                let min = vec3((x * CHUNK) as f32, (y * CHUNK) as f32, (z * CHUNK) as f32);
                Chunk {
                    min,
                    max: min + Vec3::splat(CHUNK as f32),
                    voxels: Vec::new(),
                }
            })
            .collect();
        for y in 0..world.h {
            for z in 0..world.d {
                for x in 0..world.w {
                    let m = world.get(x, y, z);
                    if m == Mat::Air {
                        continue;
                    }
                    let open = open_faces(world, x, y, z, m);
                    if open == 0 {
                        continue;
                    }
                    let shade = shade_voxel(world, x, y, z, m, open);
                    let ci = (x / CHUNK) + (y / CHUNK) * cx + (z / CHUNK) * cx * cy;
                    chunks[ci as usize].voxels.push(Surf {
                        pos: [x as u16, y as u16, z as u16],
                        shade,
                    });
                }
            }
        }
        chunks.retain(|c| !c.voxels.is_empty());

        let (gw, gh, gd) = (
            (world.w + GRID - 1) / GRID,
            (world.h + GRID - 1) / GRID,
            (world.d + GRID - 1) / GRID,
        );
        let mut lighting = Lighting {
            chunks,
            gw,
            gh,
            gd,
            grid_lamp: vec![[0.0; 3]; (gw * gh * gd) as usize],
            grid_other: vec![[0.0; 3]; (gw * gh * gd) as usize],
        };
        lighting.bake_grid(world);
        lighting
    }

    fn bake_grid(&mut self, world: &World) {
        let (gw, gh, gd) = (self.gw, self.gh, self.gd);
        for gy in 0..gh {
            for gz in 0..gd {
                for gx in 0..gw {
                    let p = vec3(gx as f32 + 0.5, gy as f32 + 0.5, gz as f32 + 0.5) * GRID as f32;
                    let i = ((gy * gd + gz) * gw + gx) as usize;
                    let (lamp, other) = point_lights(world, p, None);
                    self.grid_lamp[i] = lamp;
                    self.grid_other[i] = other;
                }
            }
        }
    }

    /// Trilinear sample of the coarse grid: (lamp, other) light reaching `p`.
    pub fn grid_at(&self, p: Vec3) -> (Rgb, Rgb) {
        let g = p / GRID as f32 - Vec3::splat(0.5);
        let base = g.floor();
        let f = g - base;
        let (bx, by, bz) = (base.x as i32, base.y as i32, base.z as i32);
        let mut lamp = [0.0; 3];
        let mut other = [0.0; 3];
        for corner in 0..8 {
            let (ox, oy, oz) = (corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
            let (x, y, z) = (
                (bx + ox).clamp(0, self.gw - 1),
                (by + oy).clamp(0, self.gh - 1),
                (bz + oz).clamp(0, self.gd - 1),
            );
            let w = (if ox == 1 { f.x } else { 1.0 - f.x })
                * (if oy == 1 { f.y } else { 1.0 - f.y })
                * (if oz == 1 { f.z } else { 1.0 - f.z });
            let i = ((y * self.gd + z) * self.gw + x) as usize;
            for c in 0..3 {
                lamp[c] += self.grid_lamp[i][c] * w;
                other[c] += self.grid_other[i][c] * w;
            }
        }
        (lamp, other)
    }
}

/// Bit set of faces (index into `FACES`) that see air, or glass for
/// opaque voxels (things behind glass must still be drawn).
fn open_faces(world: &World, x: i32, y: i32, z: i32, m: Mat) -> u8 {
    let mut open = 0;
    for (i, (dx, dy, dz)) in FACES.iter().enumerate() {
        let n = world.get(x + dx, y + dy, z + dz);
        if n == Mat::Air || (n.is_glass() && !m.is_glass()) {
            open |= 1 << i;
        }
    }
    open
}

fn shade_voxel(world: &World, x: i32, y: i32, z: i32, m: Mat, open: u8) -> Shade {
    let cut = world.cut_bits(x, y, z);
    let albedo = m.albedo(x, y, z);
    let top = open & (1 << 2) != 0;
    let mut flags = 0;
    if top {
        flags |= F_TOP;
        if world.height_at(x, z) == y + 1 {
            flags |= F_WET;
        }
    }
    if m.is_glass() {
        flags |= F_GLASS;
    }

    if let Some(e) = m.emissive(x, y, z) {
        flags |= F_EMISSIVE;
        let (lamp, other) = if m == Mat::LampBulb {
            (e, [0.0; 3])
        } else {
            ([0.0; 3], e)
        };
        return Shade {
            amb: enc_rgb_ldr(scale(albedo, 0.1)),
            lamp: enc_rgb_hdr(lamp),
            other: enc_rgb_hdr(other),
            flags,
            spec: 0,
            cut,
        };
    }

    // sky: best over open faces of (horizon visibility * corner occlusion)
    let mut sky = 0.0f32;
    for (i, &(dx, dy, dz)) in FACES.iter().enumerate() {
        if open & (1 << i) == 0 {
            continue;
        }
        let ao = face_ao(world, x + dx, y + dy, z + dz, (dx, dy, dz));
        let vis = match dy {
            1 => horizon(world, x, y + 1, z, None),
            -1 => 0.08,
            _ => 0.55 * horizon(world, x + dx, y, z + dz, Some((dx as f32, dz as f32))),
        };
        sky = sky.max(vis * ao);
    }
    let fill = world.info.fill;
    let ambient = [
        SKY[0] * sky + fill[0],
        SKY[1] * sky + fill[1],
        SKY[2] * sky + fill[2],
    ];

    let center = vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
    let (lamp, other) = point_lights(world, center, Some(open));
    let spec = if flags & F_WET != 0 {
        enc_hdr((lamp[0] + lamp[1] + lamp[2]) * 0.4)
    } else {
        0
    };
    Shade {
        amb: enc_rgb_ldr(mul(albedo, ambient)),
        lamp: enc_rgb_hdr(mul(albedo, lamp)),
        other: enc_rgb_hdr(mul(albedo, other)),
        flags,
        spec,
        cut,
    }
}

/// Corner occlusion of one face: opaque voxels around the open cell.
fn face_ao(world: &World, x: i32, y: i32, z: i32, n: (i32, i32, i32)) -> f32 {
    let (ua, va) = match n {
        (_, 0, 0) => ((0, 1, 0), (0, 0, 1)),
        (0, _, 0) => ((1, 0, 0), (0, 0, 1)),
        _ => ((1, 0, 0), (0, 1, 0)),
    };
    let mut blocked = 0;
    for a in -1..=1 {
        for b in -1..=1 {
            if (a, b) == (0, 0) {
                continue;
            }
            let (px, py, pz) = (
                x + ua.0 * a + va.0 * b,
                y + ua.1 * a + va.1 * b,
                z + ua.2 * a + va.2 * b,
            );
            let m = world.get(px, py, pz);
            if m != Mat::Air && !m.is_glass() {
                blocked += 1;
            }
        }
    }
    1.0 - blocked as f32 * 0.075
}

/// Fraction of the sky seen from a cell, by marching the height map in
/// eight directions (optionally only the half facing `facing`).
fn horizon(world: &World, x: i32, y: i32, z: i32, facing: Option<(f32, f32)>) -> f32 {
    const STEPS: [f32; 7] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0];
    let mut total = 0.0;
    let mut n = 0.0;
    for (dx, dz) in DIRS {
        if let Some((fx, fz)) = facing
            && dx * fx + dz * fz < 0.1
        {
            continue;
        }
        let mut slope = -1.0f32;
        for t in STEPS {
            let hx = (x as f32 + 0.5 + dx * t).floor() as i32;
            let hz = (z as f32 + 0.5 + dz * t).floor() as i32;
            let h = world.height_at(hx, hz) as f32;
            slope = slope.max((h - y as f32) / t);
        }
        // 0 slope = open horizon (full), 45 degrees = half, steep = none
        total += 1.0 - slope.max(0.0).atan() * std::f32::consts::FRAC_2_PI;
        n += 1.0;
    }
    if n > 0.0 { total / n } else { 0.0 }
}

/// Sum of all static point lights at `p`; with `open` faces the light is
/// weighted by the best-facing open face (Lambert with a soft wrap),
/// without it (moving things) only distance, cone and occlusion count.
pub fn point_lights(world: &World, p: Vec3, open: Option<u8>) -> (Rgb, Rgb) {
    let mut lamp = [0.0; 3];
    let mut other = [0.0; 3];
    for light in &world.lights {
        let to = light.pos - p;
        let dist = to.length();
        if dist >= light.radius || dist < 1e-3 {
            continue;
        }
        let l = to / dist;
        let facing = match open {
            Some(open) => {
                let mut best = -1.0f32;
                for (i, &(dx, dy, dz)) in FACES.iter().enumerate() {
                    if open & (1 << i) != 0 {
                        best = best.max(dx as f32 * l.x + dy as f32 * l.y + dz as f32 * l.z);
                    }
                }
                if best <= 0.0 {
                    continue;
                }
                0.25 + 0.75 * best
            }
            None => 1.0,
        };
        let cone = emission(light, l);
        if cone <= 0.0 {
            continue;
        }
        let falloff = {
            let r = dist / light.radius;
            let window = (1.0 - r * r).max(0.0);
            window * window / (1.0 + (dist / (light.radius * 0.28)).powi(2))
        };
        let start = match open {
            Some(_) => p + l * 0.9,
            None => p,
        };
        let vis = transmittance(world, start, light.pos);
        let k = facing * cone * falloff * 2.2;
        let target = if light.channel == 0 {
            &mut lamp
        } else {
            &mut other
        };
        for c in 0..3 {
            target[c] += light.color[c] * vis[c] * k;
        }
    }
    (lamp, other)
}

/// Directional emission profile: downward spot or one-sided panel.
fn emission(light: &Light, to_light: Vec3) -> f32 {
    if light.dir == Vec3::ZERO {
        return 1.0;
    }
    let c = (-to_light).dot(light.dir);
    if light.cone > 0.0 {
        0.06 + 0.94 * smoothstep(light.cone, light.cone + 0.4, c)
    } else {
        c.max(0.0).powf(0.6)
    }
}

/// Voxel ray march (Amanatides & Woo) from `a` to `b`: opaque voxels stop
/// the light, glass tints it. Voxels within 2 of `b` (the fixture) are
/// ignored.
pub fn transmittance(world: &World, a: Vec3, b: Vec3) -> Rgb {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return [1.0; 3];
    }
    let dir = d / len;
    let (mut x, mut y, mut z) = (a.x.floor() as i32, a.y.floor() as i32, a.z.floor() as i32);
    let step = |v: f32| if v > 0.0 { 1 } else { -1 };
    let (sx, sy, sz) = (step(dir.x), step(dir.y), step(dir.z));
    let inv = |v: f32| {
        if v.abs() < 1e-9 {
            f32::INFINITY
        } else {
            1.0 / v.abs()
        }
    };
    let (dx, dy, dz) = (inv(dir.x), inv(dir.y), inv(dir.z));
    let frac = |p: f32, s: i32| {
        if s > 0 {
            p.floor() + 1.0 - p
        } else {
            p - p.floor()
        }
    };
    let (mut tx, mut ty, mut tz) = (frac(a.x, sx) * dx, frac(a.y, sy) * dy, frac(a.z, sz) * dz);
    let mut t = 0.0;
    let mut tr = [1.0f32; 3];
    let ignore = (len - 2.0).max(0.0);
    while t < ignore {
        if t > 0.0 {
            match world.get(x, y, z) {
                Mat::Air => {}
                Mat::Glass => {
                    tr = mul(tr, [0.72, 0.86, 0.94]);
                }
                _ => return [0.0; 3],
            }
        }
        if tx < ty && tx < tz {
            x += sx;
            t = tx;
            tx += dx;
        } else if ty < tz {
            y += sy;
            t = ty;
            ty += dy;
        } else {
            z += sz;
            t = tz;
            tz += dz;
        }
    }
    tr
}
