//! Water as a 2D height field over the world footprint.
//!
//! Every water column (canal, puddles, fountains: the level's water boxes) stores its surface
//! height and vertical velocity; a damped wave equation spreads ripples
//! from rain drops, splashes and bobbing boxes. Each body relaxes back to
//! its own rest level. The renderer turns the field into a translucent
//! volume (surface plus the cut faces at the world edge).

use crate::util::Rng;
use crate::world::World;

pub struct Water {
    w: i32,
    d: i32,
    pub h: Vec<f32>,
    v: Vec<f32>,
    pub rest: Vec<f32>,
    pub floor: Vec<f32>,
    /// Indices of water columns (`z * w + x`).
    pub cells: Vec<u32>,
    /// Neighbor indices per cell (self index where the neighbor is dry).
    neighbors: Vec<[u32; 4]>,
}

const WAVE_SPEED: f32 = 34.0;
const DAMPING: f32 = 0.9965;
const RESTORE: f32 = 6.0;

impl Water {
    pub fn new(world: &World) -> Water {
        let (w, d) = (world.w, world.d);
        let n = (w * d) as usize;
        let mut rest = vec![f32::NAN; n];
        let mut floor = vec![0.0; n];
        for b in &world.water {
            for z in b.z0.max(0)..b.z1.min(d) {
                for x in b.x0.max(0)..b.x1.min(w) {
                    // only open columns hold water
                    if !world.solid(x, b.floor as i32, z) {
                        let i = (z * w + x) as usize;
                        rest[i] = b.level;
                        floor[i] = b.floor;
                    }
                }
            }
        }
        let cells: Vec<u32> = (0..n as u32)
            .filter(|&i| !rest[i as usize].is_nan())
            .collect();
        let neighbors = cells
            .iter()
            .map(|&i| {
                let (x, z) = ((i as i32) % w, (i as i32) / w);
                let pick = |nx: i32, nz: i32| {
                    if (0..w).contains(&nx) && (0..d).contains(&nz) {
                        let j = (nz * w + nx) as usize;
                        if !rest[j].is_nan() && rest[j] == rest[i as usize] {
                            return j as u32;
                        }
                    }
                    i
                };
                [
                    pick(x - 1, z),
                    pick(x + 1, z),
                    pick(x, z - 1),
                    pick(x, z + 1),
                ]
            })
            .collect();
        let h = rest
            .iter()
            .map(|r| if r.is_nan() { 0.0 } else { *r })
            .collect();
        Water {
            w,
            d,
            h,
            v: vec![0.0; n],
            rest,
            floor,
            cells,
            neighbors,
        }
    }

    #[inline]
    pub fn index(&self, x: i32, z: i32) -> Option<usize> {
        ((0..self.w).contains(&x) && (0..self.d).contains(&z)).then(|| (z * self.w + x) as usize)
    }

    /// (x, z) of a cell index.
    pub fn cell_xz(&self, i: u32) -> (i32, i32) {
        (i as i32 % self.w, i as i32 / self.w)
    }

    /// Surface height of the column under (x, z), if it holds water.
    pub fn surface(&self, x: f32, z: f32) -> Option<f32> {
        let i = self.index(x.floor() as i32, z.floor() as i32)?;
        (!self.rest[i].is_nan()).then(|| self.h[i])
    }

    /// Adds a vertical velocity kick in a disc (negative = push down).
    pub fn impulse(&mut self, x: f32, z: f32, radius: f32, strength: f32) {
        let r = radius.max(0.5);
        let (x0, x1) = ((x - r).floor() as i32, (x + r).ceil() as i32);
        let (z0, z1) = ((z - r).floor() as i32, (z + r).ceil() as i32);
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                if let Some(i) = self.index(cx, cz)
                    && !self.rest[i].is_nan()
                {
                    let (dx, dz) = (cx as f32 + 0.5 - x, cz as f32 + 0.5 - z);
                    let d2 = (dx * dx + dz * dz) / (r * r);
                    if d2 < 1.0 {
                        self.v[i] += strength * (1.0 - d2);
                    }
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32, rng: &mut Rng) {
        let c2 = WAVE_SPEED * WAVE_SPEED * dt;
        for (k, &i) in self.cells.iter().enumerate() {
            let i = i as usize;
            let [a, b, c, d] = self.neighbors[k];
            let lap =
                self.h[a as usize] + self.h[b as usize] + self.h[c as usize] + self.h[d as usize]
                    - 4.0 * self.h[i];
            let restore = (self.rest[i] - self.h[i]) * RESTORE;
            self.v[i] = (self.v[i] + (lap * c2 + restore * dt)) * DAMPING;
        }
        for &i in &self.cells {
            let i = i as usize;
            let lo = self.floor[i] + 0.2;
            self.h[i] = (self.h[i] + self.v[i] * dt).max(lo);
        }
        // a slow swell keeps the canal alive between drops
        if !self.cells.is_empty() && rng.chance(0.08) {
            let k = rng.below(self.cells.len() as u32) as usize;
            let (x, z) = self.cell_xz(self.cells[k]);
            self.impulse(x as f32, z as f32, 3.0, -2.5);
        }
    }

    /// Surface slope (dh/dx, dh/dz) for shading and drifting.
    pub fn slope(&self, x: i32, z: i32) -> (f32, f32) {
        let hc = |x: i32, z: i32, fallback: f32| match self.index(x, z) {
            Some(i) if !self.rest[i].is_nan() => self.h[i],
            _ => fallback,
        };
        let Some(i) = self.index(x, z) else {
            return (0.0, 0.0);
        };
        let h0 = self.h[i];
        (
            (hc(x + 1, z, h0) - hc(x - 1, z, h0)) * 0.5,
            (hc(x, z + 1, h0) - hc(x, z - 1, h0)) * 0.5,
        )
    }
}
