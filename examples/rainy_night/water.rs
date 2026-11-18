//! Water as a 2D height field over the world footprint.
//!
//! Every water column (canal ring and street puddles) stores its surface
//! height and vertical velocity; a damped wave equation spreads ripples
//! from rain drops, splashes and bobbing boxes. Each body relaxes back to
//! its own rest level. The renderer turns the field into a translucent
//! volume (surface plus the cut faces at the world edge).

use crate::util::Rng;
use crate::world::{D, GROUND, MOAT, W, WATER_FLOOR, WATER_LEVEL, World};

pub struct Water {
    pub h: Vec<f32>,
    v: Vec<f32>,
    pub rest: Vec<f32>,
    pub floor: Vec<f32>,
    /// Indices of water columns (`z * W + x`).
    pub cells: Vec<u32>,
    /// Neighbor indices per cell (self index where the neighbor is dry).
    neighbors: Vec<[u32; 4]>,
}

const WAVE_SPEED: f32 = 34.0;
const DAMPING: f32 = 0.9965;
const RESTORE: f32 = 6.0;

impl Water {
    pub fn new(world: &World) -> Water {
        let n = (W * D) as usize;
        let mut rest = vec![f32::NAN; n];
        let mut floor = vec![0.0; n];
        for z in 0..D {
            for x in 0..W {
                let i = (z * W + x) as usize;
                let canal = !(MOAT..W - MOAT).contains(&x) || !(MOAT..D - MOAT).contains(&z);
                if canal {
                    rest[i] = WATER_LEVEL;
                    floor[i] = WATER_FLOOR as f32;
                } else {
                    // puddle dips: an open column whose ground sits below the road
                    let top = world.height_at(x, z);
                    if top < GROUND && top > GROUND - 6 && !world.solid(x, GROUND - 1, z) {
                        rest[i] = GROUND as f32 - 0.45;
                        floor[i] = top as f32;
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
                let (x, z) = ((i as i32) % W, (i as i32) / W);
                let pick = |nx: i32, nz: i32| {
                    if (0..W).contains(&nx) && (0..D).contains(&nz) {
                        let j = (nz * W + nx) as usize;
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
            h,
            v: vec![0.0; n],
            rest,
            floor,
            cells,
            neighbors,
        }
    }

    #[inline]
    pub fn index(x: i32, z: i32) -> Option<usize> {
        ((0..W).contains(&x) && (0..D).contains(&z)).then(|| (z * W + x) as usize)
    }

    /// Surface height of the column under (x, z), if it holds water.
    pub fn surface(&self, x: f32, z: f32) -> Option<f32> {
        let i = Self::index(x.floor() as i32, z.floor() as i32)?;
        (!self.rest[i].is_nan()).then(|| self.h[i])
    }

    /// Adds a vertical velocity kick in a disc (negative = push down).
    pub fn impulse(&mut self, x: f32, z: f32, radius: f32, strength: f32) {
        let r = radius.max(0.5);
        let (x0, x1) = ((x - r).floor() as i32, (x + r).ceil() as i32);
        let (z0, z1) = ((z - r).floor() as i32, (z + r).ceil() as i32);
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                if let Some(i) = Self::index(cx, cz)
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
        if rng.chance(0.08) {
            let k = rng.below(self.cells.len() as u32) as usize;
            let i = self.cells[k] as i32;
            self.impulse((i % W) as f32, (i / W) as f32, 3.0, -2.5);
        }
    }

    /// Surface slope (dh/dx, dh/dz) for shading and drifting.
    pub fn slope(&self, x: i32, z: i32) -> (f32, f32) {
        let hc = |x: i32, z: i32, fallback: f32| match Self::index(x, z) {
            Some(i) if !self.rest[i].is_nan() => self.h[i],
            _ => fallback,
        };
        let Some(i) = Self::index(x, z) else {
            return (0.0, 0.0);
        };
        let h0 = self.h[i];
        (
            (hc(x + 1, z, h0) - hc(x - 1, z, h0)) * 0.5,
            (hc(x, z + 1, h0) - hc(x, z - 1, h0)) * 0.5,
        )
    }
}
