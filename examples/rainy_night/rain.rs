//! Weather: rain drops falling through the voxel world, splashes, roof
//! drips, the downspout stream and lightning.
//!
//! Drops are real 3D particles: they stop on whatever voxel they hit (so
//! the ground under the bus-stop roof stays dry), ripple the water they
//! fall into and bounce a few droplets off hard ground.

use crate::cat::{self, Cat};
use crate::props::{GRAVITY, Prop};
use crate::util::Rng;
use crate::water::Water;
use crate::world::{D, H, W, World};
use glam::{Vec3, vec3};

pub const MAX_DROPS: usize = 2600;
const MAX_PARTICLES: usize = 1800;
const FALL_SPEED: f32 = 230.0;

#[derive(Clone, Copy)]
pub struct Drop {
    pub p: Vec3,
    pub v: Vec3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Bounced droplet from a splash.
    Spray,
    /// Big drop from a roof edge or the downspout.
    Drip,
    /// Water pouring out of the downspout.
    Stream,
    /// Glitter when the fish is caught.
    Sparkle,
}

#[derive(Clone, Copy)]
pub struct Particle {
    pub p: Vec3,
    pub v: Vec3,
    pub life: f32,
    pub kind: Kind,
}

/// Something that happened this step and should make a sound.
#[derive(Clone, Copy)]
pub enum Sound {
    Drip(Vec3),
}

pub struct Weather {
    pub drops: Vec<Drop>,
    pub particles: Vec<Particle>,
    /// Number of drops currently falling (scaled with intensity).
    pub active: usize,
    pub intensity: f32,
    pub wind: f32,
    /// Lightning brightness, decays quickly.
    pub flash: f32,
    /// Seed and age of the visible bolt.
    pub bolt: Option<(u32, f32, f32)>,
    /// Drops that hit the bus-stop roof near the cat recently (0..1).
    pub roof_noise: f32,
    /// Center of the rain volume (the camera), set by the game.
    pub focus: Vec3,
    pub sounds: Vec<Sound>,
    /// (seconds left, strength, distance)
    pub thunder: Vec<(f32, f32, f32)>,
    next_strike: f32,
    flicker: Vec<f32>,
    target_intensity: f32,
    gust: f32,
}

impl Weather {
    pub fn new(rng: &mut Rng, center: Vec3) -> Weather {
        let mut w = Weather {
            drops: Vec::with_capacity(MAX_DROPS),
            particles: Vec::with_capacity(MAX_PARTICLES),
            active: MAX_DROPS,
            intensity: 0.8,
            wind: 18.0,
            flash: 0.0,
            bolt: None,
            roof_noise: 0.0,
            focus: center,
            sounds: Vec::new(),
            thunder: Vec::new(),
            next_strike: 5.0,
            flicker: Vec::new(),
            target_intensity: 0.85,
            gust: 0.0,
        };
        for _ in 0..MAX_DROPS {
            let mut d = Drop {
                p: Vec3::ZERO,
                v: Vec3::ZERO,
            };
            respawn(&mut d, rng, center, w.wind, true);
            w.drops.push(d);
        }
        w
    }

    pub fn step(
        &mut self,
        world: &World,
        water: &mut Water,
        props: &[Prop],
        cat: &Cat,
        rng: &mut Rng,
        dt: f32,
    ) {
        let center = self.focus;
        self.sounds.clear();
        // slowly wandering intensity and gusty wind
        if rng.chance(dt * 0.08) {
            self.target_intensity = rng.range(0.45, 1.0);
        }
        self.intensity += (self.target_intensity - self.intensity) * dt * 0.3;
        self.gust += (rng.range(-1.0, 1.0) * 40.0 - self.gust) * dt * 0.5;
        self.wind = 16.0 + self.gust;
        self.active = (MAX_DROPS as f32 * (0.35 + 0.65 * self.intensity)) as usize;

        let cat_c = cat.center();
        let mut roof_hits = 0;
        for i in 0..self.active {
            let d = &mut self.drops[i];
            d.v.x += (self.wind - d.v.x) * dt * 2.0;
            let next = d.p + d.v * dt;
            let (x, y, z) = (
                next.x.floor() as i32,
                next.y.floor() as i32,
                next.z.floor() as i32,
            );
            let mut hit = None;
            if let Some(h) = water.surface(next.x, next.z)
                && next.y <= h
            {
                hit = Some((vec3(next.x, h, next.z), true));
            } else if world.solid(x, y, z) {
                hit = Some((vec3(next.x, y as f32 + 1.0, next.z), false));
                if y > 55 && (next - cat_c).length_squared() < 70.0 * 70.0 {
                    roof_hits += 1;
                }
            } else if props
                .iter()
                .any(|p| next.cmpge(p.pos).all() && next.cmplt(p.max()).all())
                || inside_cat(cat, next)
            {
                hit = Some((next, false));
            }
            if let Some((at, wet)) = hit {
                if wet {
                    water.impulse(at.x, at.z, 1.4, -2.2);
                }
                if self.particles.len() < MAX_PARTICLES && rng.chance(if wet { 0.35 } else { 0.6 })
                {
                    let n = if wet { 1 } else { 1 + rng.below(2) };
                    for _ in 0..n {
                        self.particles.push(Particle {
                            p: at + vec3(0.0, 0.3, 0.0),
                            v: vec3(
                                rng.range(-22.0, 22.0),
                                rng.range(25.0, 55.0),
                                rng.range(-22.0, 22.0),
                            ),
                            life: rng.range(0.15, 0.3),
                            kind: Kind::Spray,
                        });
                    }
                }
                respawn(d, rng, center, self.wind, false);
            } else if next.y < -8.0 {
                respawn(d, rng, center, self.wind, false);
            } else {
                d.p = next;
            }
        }
        self.roof_noise += (roof_hits as f32 / 12.0 - self.roof_noise).clamp(-1.0, 1.0) * dt * 4.0;
        self.roof_noise = self.roof_noise.clamp(0.0, 1.0);

        // heavy drips off the roof edges
        let drip_rate = 0.25 * self.intensity;
        for &p in &world.drip_points {
            if rng.chance(drip_rate * dt) && self.particles.len() < MAX_PARTICLES {
                self.particles.push(Particle {
                    p,
                    v: vec3(0.0, -10.0, 0.0),
                    life: 3.0,
                    kind: Kind::Drip,
                });
            }
        }
        // the downspout pours steadily, harder in heavy rain
        let spout = world.downspout;
        let pour = (40.0 + 140.0 * self.intensity) * dt;
        let mut n = pour.floor() as i32 + i32::from(rng.chance(pour.fract()));
        while n > 0 && self.particles.len() < MAX_PARTICLES {
            n -= 1;
            self.particles.push(Particle {
                p: spout + vec3(rng.range(-1.0, 1.0), 0.0, rng.range(-0.5, 0.5)),
                v: vec3(
                    rng.range(-3.0, 3.0),
                    rng.range(-10.0, 0.0),
                    rng.range(-26.0, -18.0),
                ),
                life: 2.0,
                kind: Kind::Stream,
            });
        }

        self.step_particles(world, water, rng, cat_c, dt);
        self.step_lightning(rng, dt);
    }

    fn step_particles(
        &mut self,
        world: &World,
        water: &mut Water,
        rng: &mut Rng,
        cat_c: Vec3,
        dt: f32,
    ) {
        let mut i = 0;
        while i < self.particles.len() {
            let mut p = self.particles[i];
            p.life -= dt;
            let g = if p.kind == Kind::Sparkle {
                -60.0
            } else {
                GRAVITY * 0.8
            };
            p.v.y -= g * dt;
            let next = p.p + p.v * dt;
            let into_water = water.surface(next.x, next.z).is_some_and(|h| next.y <= h);
            let into_solid = world.solid_at(next);
            let mut dead = p.life <= 0.0;
            if (into_water || into_solid) && p.kind != Kind::Sparkle {
                dead = true;
                if matches!(p.kind, Kind::Drip | Kind::Stream) {
                    self.impact(p, into_water, water, rng, cat_c);
                }
            }
            if dead {
                self.particles.swap_remove(i);
            } else {
                p.p = next;
                self.particles[i] = p;
                i += 1;
            }
        }
    }

    /// A big drop or the downspout stream hits something.
    fn impact(
        &mut self,
        p: Particle,
        into_water: bool,
        water: &mut Water,
        rng: &mut Rng,
        cat_c: Vec3,
    ) {
        let at = p.p;
        if into_water {
            water.impulse(at.x, at.z, 2.0, -6.0);
        }
        if p.kind == Kind::Drip && (at - cat_c).length() < 120.0 {
            self.sounds.push(Sound::Drip(at));
        }
        let spray = if p.kind == Kind::Drip {
            3
        } else {
            u32::from(rng.chance(0.3))
        };
        for _ in 0..spray {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            self.particles.push(Particle {
                p: at,
                v: vec3(
                    rng.range(-30.0, 30.0),
                    rng.range(30.0, 70.0),
                    rng.range(-30.0, 30.0),
                ),
                life: rng.range(0.2, 0.35),
                kind: Kind::Spray,
            });
        }
    }

    fn step_lightning(&mut self, rng: &mut Rng, dt: f32) {
        self.flash = (self.flash - dt * 3.5).max(0.0);
        if let Some(level) = self.flicker.pop() {
            self.flash = self.flash.max(level);
        }
        if let Some(b) = &mut self.bolt {
            b.1 += dt;
            if b.1 > 0.35 {
                self.bolt = None;
            }
        }
        for t in &mut self.thunder {
            t.0 -= dt;
        }
        self.next_strike -= dt * (0.4 + self.intensity);
        if self.next_strike <= 0.0 {
            self.next_strike = rng.range(7.0, 18.0);
            let distance = rng.f32();
            let strength = rng.range(0.55, 1.0) * (1.0 - distance * 0.5);
            // two or three quick flickers, the brightest first
            let mut seq = vec![strength];
            for _ in 0..1 + rng.below(3) {
                seq.extend(std::iter::repeat_n(0.0, 3 + rng.below(6) as usize));
                seq.push(strength * rng.range(0.4, 0.9));
            }
            seq.reverse();
            self.flicker = seq;
            if distance < 0.7 {
                self.bolt = Some((rng.next_u32(), 0.0, rng.range(0.1, 0.9)));
            }
            self.thunder
                .push((0.25 + distance * 3.5, strength, distance));
        }
    }

    /// Thunder whose delay has run out, removed from the queue.
    pub fn take_thunder(&mut self) -> Option<(f32, f32)> {
        let i = self.thunder.iter().position(|t| t.0 <= 0.0)?;
        let (_, s, d) = self.thunder.swap_remove(i);
        Some((s, d))
    }

    pub fn sparkle(&mut self, at: Vec3, rng: &mut Rng) {
        for _ in 0..40 {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            self.particles.push(Particle {
                p: at,
                v: vec3(
                    rng.range(-50.0, 50.0),
                    rng.range(-10.0, 60.0),
                    rng.range(-50.0, 50.0),
                ),
                life: rng.range(0.4, 0.9),
                kind: Kind::Sparkle,
            });
        }
    }

    /// Splash the cat makes when it jumps into the water.
    pub fn splash(&mut self, at: Vec3, size: f32, rng: &mut Rng) {
        let n = (10.0 + 50.0 * size) as usize;
        for _ in 0..n {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            let a = rng.range(0.0, std::f32::consts::TAU);
            let r = rng.range(15.0, 55.0) * (0.5 + size);
            self.particles.push(Particle {
                p: at + vec3(a.cos() * 4.0, 0.5, a.sin() * 4.0),
                v: vec3(
                    a.cos() * r,
                    rng.range(50.0, 120.0) * (0.5 + size),
                    a.sin() * r,
                ),
                life: rng.range(0.3, 0.7),
                kind: Kind::Spray,
            });
        }
    }
}

fn inside_cat(cat: &Cat, p: Vec3) -> bool {
    let d = p - cat.pos;
    d.x.abs() < cat::HALF && d.z.abs() < cat::HALF && d.y > 0.0 && d.y < cat::HEIGHT
}

/// Puts a drop back at the top of the rain volume around `center`.
fn respawn(d: &mut Drop, rng: &mut Rng, center: Vec3, wind: f32, anywhere: bool) {
    // uniform over the part of the rain box that overlaps the world (+ a
    // margin), so drops never pile up on a clamped edge
    let span = |c: f32, half: f32, size: f32| {
        let lo = (c - half).max(-30.0);
        let hi = (c + half).min(size + 30.0);
        if lo < hi {
            (lo, hi)
        } else {
            (-30.0, size + 30.0)
        }
    };
    let (x0, x1) = span(center.x, 250.0, W as f32);
    let (z0, z1) = span(center.z, 160.0, D as f32);
    let y = if anywhere {
        rng.range(0.0, H as f32 + 40.0)
    } else {
        H as f32 + rng.range(0.0, 50.0)
    };
    d.p = vec3(rng.range(x0, x1), y, rng.range(z0, z1));
    d.v = vec3(wind, -FALL_SPEED * rng.range(0.9, 1.1), 0.0);
}
