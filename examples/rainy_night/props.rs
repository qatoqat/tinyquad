//! Floating clutter in the canal: a pallet, cardboard boxes and a crate.
//!
//! Each prop is an axis-aligned box that floats by Archimedes: the deeper
//! it sits below the local water surface the stronger it is pushed up, so
//! it settles at its density line and bobs on passing ripples. It drifts
//! down the surface slope, is bumped by rain-free currents, bounces off the
//! canal walls and the other props, and sinks a little when the cat stands
//! on it. Moving props push back on the water, making their own ripples.

use crate::model::PropKind;
use crate::util::Rng;
use crate::water::Water;
use crate::world::World;
use glam::{Vec3, vec3};

pub const GRAVITY: f32 = 520.0;

pub struct Prop {
    pub kind: PropKind,
    /// Minimum corner.
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: Vec3,
    /// Extra downward load (the cat), reset every step.
    pub load: f32,
    /// Movement during the last step, used to carry the cat.
    pub delta: Vec3,
    drift: Vec3,
}

impl Prop {
    pub fn new(kind: PropKind, x: f32, z: f32) -> Prop {
        let s = kind.size();
        let size = vec3(s[0] as f32, s[1] as f32, s[2] as f32);
        let y = crate::world::WATER_LEVEL - size.y * kind.density();
        Prop {
            kind,
            pos: vec3(x, y, z),
            vel: Vec3::ZERO,
            size,
            load: 0.0,
            delta: Vec3::ZERO,
            drift: Vec3::ZERO,
        }
    }

    pub fn max(&self) -> Vec3 {
        self.pos + self.size
    }

    pub fn center(&self) -> Vec3 {
        self.pos + self.size * 0.5
    }

    /// Water height averaged over the prop's corners.
    fn water_under(&self, water: &Water) -> Option<f32> {
        let mut sum = 0.0;
        let mut n = 0.0;
        for (fx, fz) in [(0.2, 0.2), (0.8, 0.2), (0.2, 0.8), (0.8, 0.8), (0.5, 0.5)] {
            if let Some(h) =
                water.surface(self.pos.x + self.size.x * fx, self.pos.z + self.size.z * fz)
            {
                sum += h;
                n += 1.0;
            }
        }
        (n > 0.0).then(|| sum / n)
    }

    fn hits_world(&self, world: &World, pos: Vec3) -> bool {
        let lo = pos + Vec3::splat(0.05);
        let hi = pos + self.size - Vec3::splat(0.05);
        for y in lo.y.floor() as i32..hi.y.ceil() as i32 {
            for z in lo.z.floor() as i32..hi.z.ceil() as i32 {
                for x in lo.x.floor() as i32..hi.x.ceil() as i32 {
                    if world.solid(x, y, z) {
                        return true;
                    }
                }
            }
        }
        // invisible walls at the world edge
        lo.x < 0.0 || lo.z < 0.0 || hi.x > crate::world::W as f32 || hi.z > crate::world::D as f32
    }
}

pub fn spawn_props() -> Vec<Prop> {
    vec![
        Prop::new(PropKind::Pallet, 120.0, 3.0),
        Prop::new(PropKind::Cardboard, 196.0, 4.0),
        Prop::new(PropKind::SmallBox, 262.0, 6.0),
        Prop::new(PropKind::Crate, 400.0, 3.0),
        Prop::new(PropKind::Cardboard, 3.0, 120.0),
        Prop::new(PropKind::Pallet, 300.0, 205.0),
        Prop::new(PropKind::SmallBox, 432.0, 60.0),
    ]
}

pub fn step(props: &mut [Prop], world: &World, water: &mut Water, rng: &mut Rng, dt: f32) {
    for p in props.iter_mut() {
        let before = p.pos;
        let mut acc = vec3(0.0, -GRAVITY, 0.0);
        if let Some(surface) = p.water_under(water) {
            let submerged = (surface - p.pos.y).clamp(0.0, p.size.y);
            let float_line = p.size.y * p.kind.density();
            acc.y += GRAVITY * submerged / float_line - p.load * 300.0;
            if submerged > 0.0 {
                // water drag, stronger vertically to keep the bob calm
                acc.y -= p.vel.y * 6.0;
                acc.x -= p.vel.x * 0.8;
                acc.z -= p.vel.z * 0.8;
                // slide down the local surface slope + a lazy wandering current
                let (sx, sz) = water.slope(p.center().x as i32, p.center().z as i32);
                if rng.chance(0.01) {
                    p.drift = vec3(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)) * 4.0;
                }
                acc.x += -sx * 120.0 + p.drift.x;
                acc.z += -sz * 120.0 + p.drift.z;
            }
        }
        p.load = 0.0;
        p.vel += acc * dt;
        // move axis by axis so a wall hit only cancels that axis
        for axis in [1usize, 0, 2] {
            let mut next = p.pos;
            next[axis] += p.vel[axis] * dt;
            if p.hits_world(world, next) {
                p.vel[axis] *= -0.3;
            } else {
                p.pos = next;
            }
        }
        // bobbing and drifting pushes on the water
        let c = p.center();
        let push = p.vel.y * 0.02 + (p.vel.x.abs() + p.vel.z.abs()) * 0.004;
        if push.abs() > 0.01 {
            water.impulse(c.x, c.z, p.size.x.max(p.size.z) * 0.6, push * 60.0 * dt);
        }
        p.delta = p.pos - before;
    }
    // props bump into each other
    for i in 0..props.len() {
        for j in i + 1..props.len() {
            let (a, b) = props.split_at_mut(j);
            let (a, b) = (&mut a[i], &mut b[0]);
            let overlap = (a.max().min(b.max()) - a.pos.max(b.pos)).max(Vec3::ZERO);
            if overlap.x > 0.0 && overlap.y > 0.0 && overlap.z > 0.0 {
                let axis = if overlap.x < overlap.z { 0 } else { 2 };
                let dir = if a.center()[axis] < b.center()[axis] {
                    -1.0
                } else {
                    1.0
                };
                let push = overlap[axis] * 0.5 + 0.01;
                let mut na = a.pos;
                na[axis] += dir * push;
                if !a.hits_world(world, na) {
                    a.pos = na;
                }
                let mut nb = b.pos;
                nb[axis] -= dir * push;
                if !b.hits_world(world, nb) {
                    b.pos = nb;
                }
                let avg = (a.vel[axis] + b.vel[axis]) * 0.5;
                a.vel[axis] = avg;
                b.vel[axis] = avg;
            }
        }
    }
}
