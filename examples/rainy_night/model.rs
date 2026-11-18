//! Small voxel models for things that move: the cat, the floating boxes
//! and the fish. Built procedurally from shape tests; only surface voxels
//! are kept since the inside can never be seen.

use crate::util::{Rgb, noise3, rgb};
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub struct Vox {
    /// Local cell, in the model's own axes (a = forward, y = up, b = left).
    pub p: [i8; 3],
    pub color: Rgb,
    pub emissive: bool,
    /// Outward direction (unnormalized) used for cheap shading.
    pub n: [i8; 3],
}

#[derive(Default)]
pub struct Model {
    pub voxels: Vec<Vox>,
}

impl Model {
    /// Builds a model from a dense sampler over `[a0, a1) x [0, h) x [b0, b1)`.
    pub fn sample(
        (a0, a1): (i32, i32),
        h: i32,
        (b0, b1): (i32, i32),
        mut f: impl FnMut(i32, i32, i32) -> Option<(Rgb, bool)>,
    ) -> Model {
        let (na, nb) = ((a1 - a0) as usize, (b1 - b0) as usize);
        let nh = h as usize;
        let mut cells = vec![None; na * nb * nh];
        let at = |a: i32, y: i32, b: i32| {
            ((y as usize * nb) + (b - b0) as usize) * na + (a - a0) as usize
        };
        for y in 0..h {
            for b in b0..b1 {
                for a in a0..a1 {
                    cells[at(a, y, b)] = f(a, y, b);
                }
            }
        }
        let filled = |a: i32, y: i32, b: i32| {
            (a0..a1).contains(&a)
                && (0..h).contains(&y)
                && (b0..b1).contains(&b)
                && cells[at(a, y, b)].is_some()
        };
        let mut voxels = Vec::new();
        for y in 0..h {
            for b in b0..b1 {
                for a in a0..a1 {
                    let Some((color, emissive)) = cells[at(a, y, b)] else {
                        continue;
                    };
                    let mut n = [0i8; 3];
                    let mut open = false;
                    for (axis, (da, dy, db)) in [
                        (0, (1, 0, 0)),
                        (0, (-1, 0, 0)),
                        (1, (0, 1, 0)),
                        (1, (0, -1, 0)),
                        (2, (0, 0, 1)),
                        (2, (0, 0, -1)),
                    ] {
                        if !filled(a + da, y + dy, b + db) {
                            open = true;
                            n[axis] += (da + dy + db) as i8;
                        }
                    }
                    if open {
                        voxels.push(Vox {
                            p: [a as i8, y as i8, b as i8],
                            color,
                            emissive,
                            n,
                        });
                    }
                }
            }
        }
        Model { voxels }
    }
}

/// Animation parameters for the cat.
#[derive(Clone, Copy, Default)]
pub struct CatPose {
    /// Walk cycle phase in radians.
    pub phase: f32,
    /// 0 = standing, 1 = full stride.
    pub stride: f32,
    /// Squash before a charged jump (0..1).
    pub crouch: f32,
    /// Airborne stretch: >0 rising, <0 falling.
    pub air: f32,
    pub tail: f32,
    pub blink: bool,
    pub swim: bool,
    pub time: f32,
}

const FUR: u32 = 0xd8843c;
const FUR_DARK: u32 = 0x9f5426;
const CREAM: u32 = 0xf1dcbc;
const PINK: u32 = 0xe08e8e;

/// A ginger tabby, ~24 voxels nose to tail, 14 tall to the ear tips,
/// 8 wide. a = -12..12 (tail to nose), b = -4..4.
pub fn cat(pose: &CatPose) -> Model {
    let crouch = pose.crouch;
    let body_y = 7.5 - crouch * 2.0 + pose.air.min(0.0) * 0.5;
    let head = (7.5 + pose.air.max(0.0) * 0.6, 10.6 - crouch * 2.6, 0.0);
    let swim_sink = if pose.swim { 1.0 } else { 0.0 };
    let tail_sway = pose.tail.sin() * 1.6;

    Model::sample((-13, 13), 16, (-4, 4), |a, y, b| {
        let (fa, fy, fb) = (a as f32 + 0.5, y as f32 + 0.5, b as f32 + 0.5);
        let stripe = |fa: f32, fy: f32| ((fa * 0.42 + fy * 0.18).floor() as i32).rem_euclid(2) == 0;
        let fur = |fa: f32, fy: f32, belly: bool| -> Rgb {
            if belly {
                rgb(CREAM)
            } else if stripe(fa, fy) && fy > body_y - 1.0 {
                rgb(FUR_DARK)
            } else {
                let n = noise3(a, y, b);
                let c = rgb(FUR);
                [c[0] * (0.94 + 0.08 * n), c[1] * (0.94 + 0.08 * n), c[2]]
            }
        };

        // head
        let (hx, hy, hz) = (fa - head.0, fy - head.1, fb - head.2);
        if superellipse(hx / 4.3, hy / 3.6, hz / 3.7) {
            // eyes: one on each side of the face, glowing at night
            if (hx - 2.6).abs() < 0.8 && (hy - 0.6).abs() < 0.8 && (hz.abs() - 2.0).abs() < 0.9 {
                return Some(if pose.blink {
                    (rgb(0x4a2a1a), false)
                } else {
                    ([0.85, 1.35, 0.35], true)
                });
            }
            if hx > 3.4 && (hy + 0.6).abs() < 0.7 && hz.abs() < 1.0 {
                return Some((rgb(PINK), false));
            }
            let muzzle = hx > 2.2 && hy < -0.6;
            return Some((fur(fa, fy, muzzle), false));
        }
        // ears
        let ear_y = fy - (head.1 + 2.6);
        if (0.0..3.2).contains(&ear_y) {
            let ear_a = fa - head.0 + 0.2;
            let side = fb.abs() - 2.0;
            let half = 1.6 - ear_y * 0.5;
            if ear_a.abs() < half && side.abs() < half {
                let inner = ear_a > 0.3 && side < 0.2 && ear_y < 1.8;
                return Some((if inner { rgb(PINK) } else { fur(fa, fy, false) }, false));
            }
        }
        // body
        let (bx, by, bz) = (fa + 1.0, fy - body_y, fb);
        if superellipse(bx / 9.2, by / 3.5, bz / 3.6) {
            return Some((fur(fa, fy, by < -1.6 || (bx > 6.0 && by < 0.5)), false));
        }
        // legs: hip fixed under the body, foot swings with the walk phase
        if !pose.swim || fy > 2.0 {
            for (leg_a, leg_b, phase_off) in [
                (5.5, 1.8, 0.0),
                (5.5, -1.8, PI),
                (-7.0, 1.8, PI),
                (-7.0, -1.8, 0.0),
            ] {
                let ph = pose.phase + phase_off;
                let swing = ph.sin() * 2.2 * pose.stride;
                let lift = (ph.cos().max(0.0) * 1.6 * pose.stride) + pose.air.abs().min(1.0) * 1.5;
                let tuck = if pose.air > 0.2 {
                    -1.0
                } else if pose.air < -0.2 {
                    1.2
                } else {
                    0.0
                };
                let hip_y = body_y - 2.0;
                let foot_y = lift + swim_sink * 2.0;
                if fy < foot_y || fy > hip_y {
                    continue;
                }
                let t = (fy - foot_y) / (hip_y - foot_y).max(0.1);
                let la = leg_a + (swing + tuck) * (1.0 - t);
                if (fa - la).abs() < 1.05 && (fb - leg_b).abs() < 1.05 {
                    let paw = fy < foot_y + 1.0;
                    return Some((fur(fa, fy, paw), false));
                }
            }
        }
        // tail: a curve rising from the rump
        for i in 0..14 {
            let t = i as f32 / 13.0;
            let ta = -9.6 - t * 3.0 + (t * t) * 1.5;
            let ty = body_y + 0.5 + t * 6.5 - crouch * 1.5;
            let tb = tail_sway * t * t;
            let (dx, dy, dz) = (fa - ta, fy - ty, fb - tb);
            if dx * dx + dy * dy + dz * dz < 1.1 {
                let tip = t > 0.85;
                return Some((
                    if tip {
                        rgb(FUR_DARK)
                    } else {
                        fur(fa + 3.0, fy, false)
                    },
                    false,
                ));
            }
        }
        None
    })
}

/// Rounded box: |x|^3 + |y|^3 + |z|^3 <= 1.
fn superellipse(x: f32, y: f32, z: f32) -> bool {
    let cube = |v: f32| v.abs() * v * v;
    cube(x) + cube(y) + cube(z) <= 1.0
}

/// Floating clutter.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Pallet,
    Cardboard,
    SmallBox,
    Crate,
}

impl PropKind {
    /// Size in voxels (x, y, z).
    pub fn size(self) -> [i32; 3] {
        match self {
            PropKind::Pallet => [22, 5, 15],
            PropKind::Cardboard => [13, 11, 12],
            PropKind::SmallBox => [10, 8, 9],
            PropKind::Crate => [14, 12, 14],
        }
    }

    /// Fraction that floats under water.
    pub fn density(self) -> f32 {
        match self {
            PropKind::Pallet => 0.45,
            PropKind::Cardboard | PropKind::SmallBox => 0.3,
            PropKind::Crate => 0.55,
        }
    }

    pub fn model(self) -> Model {
        let [sx, sy, sz] = self.size();
        Model::sample((0, sx), sy, (0, sz), |x, y, z| {
            let n = noise3(x, y, z + 99);
            Some(match self {
                PropKind::Pallet => {
                    let top = y >= sy - 2 && z.rem_euclid(4) != 3;
                    let bottom = y < 2 && (z < 3 || z >= sz - 3 || (z - sz / 2).abs() < 2);
                    let block = (2..sy - 2).contains(&y)
                        && (x < 3 || x >= sx - 3 || (x - sx / 2).abs() < 2)
                        && (z < 3 || z >= sz - 3 || (z - sz / 2).abs() < 2);
                    if !(top || bottom || block) {
                        return None;
                    }
                    let c = rgb(0x9a6a3a);
                    (
                        [
                            c[0] * (0.8 + 0.3 * n),
                            c[1] * (0.8 + 0.3 * n),
                            c[2] * (0.8 + 0.2 * n),
                        ],
                        false,
                    )
                }
                PropKind::Cardboard | PropKind::SmallBox => {
                    let tape = y >= sy - 1 && (z - sz / 2).abs() <= 1
                        || (x == 0 || x == sx - 1) && (z - sz / 2).abs() <= 1 && y > sy - 4;
                    let soggy = y < 3;
                    let c = if tape {
                        rgb(0xc8b48a)
                    } else if soggy {
                        rgb(0x7a5530)
                    } else {
                        rgb(0xa77a48)
                    };
                    (
                        [
                            c[0] * (0.92 + 0.12 * n),
                            c[1] * (0.92 + 0.12 * n),
                            c[2] * (0.92 + 0.1 * n),
                        ],
                        false,
                    )
                }
                PropKind::Crate => {
                    let edge = (x == 0 || x == sx - 1) as i32
                        + (y == 0 || y == sy - 1) as i32
                        + (z == 0 || z == sz - 1) as i32;
                    let plank = y.rem_euclid(4) == 0;
                    let c = if edge >= 2 {
                        rgb(0x5a3a1e)
                    } else if plank {
                        rgb(0x6e4a28)
                    } else {
                        rgb(0x8c6034)
                    };
                    (
                        [
                            c[0] * (0.85 + 0.25 * n),
                            c[1] * (0.85 + 0.25 * n),
                            c[2] * (0.85 + 0.2 * n),
                        ],
                        false,
                    )
                }
            })
        })
    }
}

/// The reward: a little silver fish with a faint glow.
pub fn fish() -> Model {
    const ROWS: [&str; 5] = [
        "..####...#",
        ".######.##",
        "########.#",
        ".######.##",
        "..####...#",
    ];
    Model::sample((0, 10), 5, (0, 2), |a, y, _| {
        let row = ROWS[(4 - y) as usize].as_bytes();
        if row[a as usize] != b'#' {
            return None;
        }
        if a == 6 && y == 3 {
            return Some((rgb(0x101820), false));
        }
        let shine = 0.85 + 0.15 * (y as f32 / 4.0);
        Some(([0.75 * shine, 0.9 * shine, 1.15 * shine], true))
    })
}
