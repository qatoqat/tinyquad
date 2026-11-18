//! The playable cat and its Fez-style physics.
//!
//! The cat is a 12x12x12 box in a 3D voxel world, but how it collides
//! depends on the camera:
//!
//! - **Side views** play as 2D. A/D walk along the screen; floors are taken
//!   from the *projection*: any top surface under the cat's feet on screen
//!   holds it, whatever its depth. While the surface at the cat's own depth
//!   still holds it the depth stays put; otherwise the cat is moved in depth
//!   to the nearest top that does (and that has room for it). Walls and
//!   ceilings only block at the cat's own depth, so things in front or
//!   behind are passed and can be landed on. Rotating the camera never moves
//!   the cat, so the depth it was snapped to becomes its new screen position:
//!   two platforms that merely *line up* in one view are a real path in it.
//! - **Top-down** is plain 3D: walk on the plane, collide with everything.
//!
//! Space jumps, Shift sprints while moving or charges a big jump while
//! standing, Ctrl (or S in side view) drops through the platform below.

use crate::model::CatPose;
use crate::props::{GRAVITY, Prop};
use crate::water::Water;
use crate::world::{D, GROUND, W, World};
use glam::{Vec3, vec3};

pub const HALF: f32 = 6.0;
pub const HEIGHT: f32 = 12.0;

const WALK: f32 = 62.0;
const SPRINT: f32 = 120.0;
const SWIM: f32 = 36.0;
const JUMP: f32 = 146.0; // ~20 voxels
const SUPER_JUMP: f32 = 206.0; // ~40 voxels at full charge
const SWIM_JUMP: f32 = 150.0;
const CHARGE_TIME: f32 = 0.7;
const EPS: f32 = 0.01;

#[derive(Clone, Copy, Default)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jump_pressed: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub drop_pressed: bool,
}

/// How the current camera wants the cat to collide.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// 2D projected physics; `axis` (0 = x, 2 = z) runs left to right on
    /// screen in direction `sign`.
    Side { axis: usize, sign: f32 },
    /// Plain 3D; screen right / up in world space.
    Top { right: Vec3, up: Vec3 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Support {
    None,
    Static,
    Prop(usize),
}

#[derive(Clone, Copy)]
struct Ghost {
    /// Height band of the platform being dropped through...
    lo: f32,
    hi: f32,
    /// ...but only under where the cat stood (x0, z0, x1, z1), so walls
    /// elsewhere at that height still block.
    area: [i32; 4],
    prop: Option<usize>,
    timer: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Jump,
    Land(f32),
    Step,
    Splash(f32),
}

pub struct Cat {
    pub pos: Vec3,
    pub vel: Vec3,
    pub facing: Vec3,
    pub support: Support,
    pub charge: f32,
    pub swimming: bool,
    pub pose: CatPose,
    pub events: Vec<Event>,
    coyote: f32,
    jump_buffer: f32,
    ghost: Option<Ghost>,
    blink_timer: f32,
    step_phase: f32,
}

/// What the cat collides with during one step.
struct Solids<'a> {
    world: &'a World,
    props: &'a [Prop],
    ghost: Option<Ghost>,
}

enum Hit {
    Voxel,
    Prop(usize),
}

impl Solids<'_> {
    #[inline]
    fn voxel(&self, x: i32, y: i32, z: i32) -> bool {
        if let Some(g) = self.ghost
            && (y as f32) >= g.lo - EPS
            && (y as f32) < g.hi
            && (g.area[0]..g.area[2]).contains(&x)
            && (g.area[1]..g.area[3]).contains(&z)
        {
            return false;
        }
        self.world.solid(x, y, z)
    }

    fn box_hit(&self, pos: Vec3) -> Option<Hit> {
        let lo = pos - vec3(HALF, 0.0, HALF) + Vec3::splat(EPS);
        let hi = pos + vec3(HALF, HEIGHT, HALF) - Vec3::splat(EPS);
        for y in lo.y.floor() as i32..hi.y.ceil() as i32 {
            for z in lo.z.floor() as i32..hi.z.ceil() as i32 {
                for x in lo.x.floor() as i32..hi.x.ceil() as i32 {
                    if self.voxel(x, y, z) {
                        return Some(Hit::Voxel);
                    }
                }
            }
        }
        for (i, p) in self.props.iter().enumerate() {
            if self.ghost.is_some_and(|g| g.prop == Some(i)) {
                continue;
            }
            let (pl, ph) = (p.pos, p.max());
            if lo.x < ph.x
                && hi.x > pl.x
                && lo.y < ph.y
                && hi.y > pl.y
                && lo.z < ph.z
                && hi.z > pl.z
            {
                return Some(Hit::Prop(i));
            }
        }
        None
    }

    fn free(&self, pos: Vec3) -> bool {
        self.box_hit(pos).is_none()
    }
}

impl Cat {
    pub fn new(pos: Vec3) -> Cat {
        Cat {
            pos,
            vel: Vec3::ZERO,
            facing: Vec3::X,
            support: Support::Static,
            charge: 0.0,
            swimming: false,
            pose: CatPose::default(),
            events: Vec::new(),
            coyote: 0.0,
            jump_buffer: 0.0,
            ghost: None,
            blink_timer: 2.0,
            step_phase: 0.0,
        }
    }

    /// No static voxel or prop overlaps the body (test helper).
    #[cfg(test)]
    pub fn is_free(&self, world: &World, props: &[Prop]) -> bool {
        Solids {
            world,
            props,
            ghost: self.ghost,
        }
        .free(self.pos)
    }

    pub fn grounded(&self) -> bool {
        self.support != Support::None
    }

    /// Center of the body, for the camera and sounds.
    pub fn center(&self) -> Vec3 {
        self.pos + vec3(0.0, HEIGHT * 0.5, 0.0)
    }

    pub fn step(
        &mut self,
        world: &World,
        props: &mut [Prop],
        water: &mut Water,
        input: &Input,
        mode: Mode,
        dt: f32,
    ) {
        // ride whatever we stand on
        if let Support::Prop(i) = self.support
            && let Some(p) = props.get_mut(i)
        {
            self.pos += p.delta;
            p.load = 1.0;
        }
        if let Some(g) = &mut self.ghost {
            g.timer -= dt;
        }
        if self
            .ghost
            .is_some_and(|g| g.timer <= 0.0 || self.pos.y + HEIGHT < g.lo - EPS)
        {
            let solids = Solids {
                world,
                props,
                ghost: None,
            };
            if solids.free(self.pos) {
                self.ghost = None;
            }
        }

        let solids = Solids {
            world,
            props,
            ghost: self.ghost,
        };
        self.depenetrate(&solids);
        self.control(&solids, water, input, mode, dt);
        self.integrate(&solids, mode, dt);
        self.check_water(water);
        self.animate(input, dt);

        // never leave the diorama
        self.pos.x = self.pos.x.clamp(HALF, W as f32 - HALF);
        self.pos.z = self.pos.z.clamp(HALF, D as f32 - HALF);
    }

    /// Floating props move on their own and may drift into the cat: push the
    /// cat out along the shallowest axis (preferring up, onto the prop).
    fn depenetrate(&mut self, solids: &Solids) {
        let i = match solids.box_hit(self.pos) {
            None => return,
            Some(Hit::Prop(i)) => i,
            Some(Hit::Voxel) => {
                // should not happen; as a last resort find the nearest free spot
                for r in 1..=12 {
                    let r = r as f32;
                    for d in [Vec3::Y, Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z, -Vec3::Y] {
                        if solids.free(self.pos + d * r) {
                            self.pos += d * r;
                            return;
                        }
                    }
                }
                return;
            }
        };
        let p = &solids.props[i];
        let lo = self.pos - vec3(HALF, 0.0, HALF);
        let hi = self.pos + vec3(HALF, HEIGHT, HALF);
        let mut options = [
            (p.max().y - lo.y, Vec3::Y),
            (p.max().x - lo.x, Vec3::X),
            (hi.x - p.pos.x, -Vec3::X),
            (p.max().z - lo.z, Vec3::Z),
            (hi.z - p.pos.z, -Vec3::Z),
            (hi.y - p.pos.y, -Vec3::Y),
        ];
        options.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (depth, dir) in options {
            let moved = self.pos + dir * (depth + EPS);
            if solids.free(moved) {
                self.pos = moved;
                if dir == Vec3::Y {
                    self.support = Support::Prop(i);
                    self.vel.y = self.vel.y.max(0.0);
                }
                return;
            }
        }
    }

    fn control(&mut self, solids: &Solids, water: &mut Water, input: &Input, mode: Mode, dt: f32) {
        let axis_input = |neg: bool, pos: bool| f32::from(u8::from(pos)) - f32::from(u8::from(neg));
        let (desired, moving) = match mode {
            Mode::Side { axis, sign } => {
                let mv = axis_input(input.left, input.right);
                let mut d = Vec3::ZERO;
                d[axis] = mv * sign;
                (d, mv != 0.0)
            }
            Mode::Top { right, up } => {
                let d = right * axis_input(input.left, input.right)
                    + up * axis_input(input.down, input.up);
                (d.normalize_or_zero(), d != Vec3::ZERO)
            }
        };
        if moving {
            self.facing = if desired.x.abs() >= desired.z.abs() {
                vec3(desired.x.signum(), 0.0, 0.0)
            } else {
                vec3(0.0, 0.0, desired.z.signum())
            };
        }

        // Shift: sprint while moving, charge while standing on something
        let charging = input.shift && !moving && self.grounded();
        self.charge = if charging {
            (self.charge + dt / CHARGE_TIME).min(1.0)
        } else {
            (self.charge - dt * 4.0).max(0.0)
        };
        let speed = if self.swimming {
            SWIM
        } else if input.shift && moving {
            SPRINT
        } else {
            WALK
        };
        let accel = if self.grounded() || self.swimming {
            900.0
        } else {
            380.0
        };
        for axis in [0, 2] {
            let target = desired[axis] * speed;
            let dv = (target - self.vel[axis]).clamp(-accel * dt, accel * dt);
            self.vel[axis] += dv;
        }

        // jumping, with a little buffering and coyote time
        if input.jump_pressed {
            self.jump_buffer = 0.12;
        }
        self.jump_buffer = (self.jump_buffer - dt).max(0.0);
        self.coyote = if self.grounded() {
            0.09
        } else {
            (self.coyote - dt).max(0.0)
        };
        let can_swim_jump = self.swimming
            && water
                .surface(self.pos.x, self.pos.z)
                .is_some_and(|h| self.pos.y > h - 9.0);
        if self.jump_buffer > 0.0 && (self.coyote > 0.0 || can_swim_jump) {
            self.vel.y = if self.swimming {
                SWIM_JUMP
            } else {
                JUMP + (SUPER_JUMP - JUMP) * self.charge
            };
            if self.swimming {
                water.impulse(self.pos.x, self.pos.z, 7.0, -18.0);
                self.events.push(Event::Splash(0.3));
            }
            self.events.push(Event::Jump);
            self.support = Support::None;
            self.charge = 0.0;
            self.coyote = 0.0;
            self.jump_buffer = 0.0;
        }

        // drop through the platform we stand on
        if input.drop_pressed && self.grounded() && self.pos.y > GROUND as f32 + 1.5 {
            self.start_drop(solids);
        }
    }

    fn start_drop(&mut self, solids: &Solids) {
        let top = self.pos.y;
        let prop = match self.support {
            Support::Prop(i) => Some(i),
            _ => None,
        };
        // find how thick the floor is under the feet
        let mut lo = top;
        if prop.is_none() {
            let x0 = (self.pos.x - HALF + EPS).floor() as i32;
            let x1 = (self.pos.x + HALF - EPS).ceil() as i32;
            let z0 = (self.pos.z - HALF + EPS).floor() as i32;
            let z1 = (self.pos.z + HALF - EPS).ceil() as i32;
            let mut y = top as i32 - 1;
            while y > 0 && (top as i32 - y) < 30 {
                let any = (x0..x1).any(|x| (z0..z1).any(|z| solids.world.solid(x, y, z)));
                if !any {
                    break;
                }
                lo = y as f32;
                y -= 1;
            }
            if top - lo > 24.0 {
                return; // a solid block, not a platform
            }
        }
        let area = [
            (self.pos.x - HALF).floor() as i32,
            (self.pos.z - HALF).floor() as i32,
            (self.pos.x + HALF).ceil() as i32,
            (self.pos.z + HALF).ceil() as i32,
        ];
        self.ghost = Some(Ghost {
            lo,
            hi: top,
            area,
            prop,
            timer: 0.6,
        });
        self.support = Support::None;
        self.vel.y = -40.0;
    }

    fn integrate(&mut self, solids: &Solids, mode: Mode, dt: f32) {
        if self.swimming {
            // buoyancy holds the head above the surface; handled in check_water
        } else {
            let fast_fall = if self.vel.y < 0.0 { 1.25 } else { 1.0 };
            self.vel.y -= GRAVITY * fast_fall * dt;
            self.vel.y = self.vel.y.max(-420.0);
        }

        match mode {
            Mode::Side { axis, .. } => self.move_horizontal(solids, axis, self.vel[axis] * dt),
            Mode::Top { .. } => {
                self.move_horizontal(solids, 0, self.vel.x * dt);
                self.move_horizontal(solids, 2, self.vel.z * dt);
            }
        }
        let side = match mode {
            Mode::Side { axis, .. } => Some(axis),
            Mode::Top { .. } => None,
        };
        if self.grounded() && self.vel.y <= 0.0 {
            self.vel.y = 0.0;
            self.keep_support(solids, side);
        } else {
            self.move_vertical(solids, self.vel.y * dt, side);
        }
    }

    fn move_horizontal(&mut self, solids: &Solids, axis: usize, delta: f32) {
        if delta == 0.0 {
            return;
        }
        let steps = (delta.abs() / 0.5).ceil().max(1.0);
        let inc = delta / steps;
        for _ in 0..steps as i32 {
            let mut next = self.pos;
            next[axis] += inc;
            if solids.free(next) {
                self.pos = next;
                continue;
            }
            // climb curbs and small ledges
            let mut stepped = false;
            if self.grounded() {
                for up in [1.0, 2.0, 3.0] {
                    let mut raised = next;
                    raised.y = (self.pos.y + up).floor();
                    if raised.y > self.pos.y && solids.free(raised) {
                        self.pos = raised;
                        stepped = true;
                        break;
                    }
                }
            }
            if !stepped {
                self.vel[axis] = 0.0;
                break;
            }
        }
    }

    fn move_vertical(&mut self, solids: &Solids, delta: f32, side: Option<usize>) {
        let steps = (delta.abs() / 0.5).ceil().max(1.0);
        let inc = delta / steps;
        for _ in 0..steps as i32 {
            let mut next = self.pos;
            next.y += inc;
            if let Some(hit) = solids.box_hit(next) {
                if inc > 0.0 {
                    self.vel.y = 0.0; // bonk
                } else {
                    let top = match hit {
                        Hit::Voxel => next.y.floor() + 1.0,
                        Hit::Prop(i) => solids.props[i].max().y,
                    };
                    let mut landed = self.pos;
                    landed.y = top.max(next.y);
                    if solids.free(landed) {
                        self.pos = landed;
                    }
                    self.land(match hit {
                        Hit::Voxel => Support::Static,
                        Hit::Prop(i) => Support::Prop(i),
                    });
                }
                return;
            }
            if inc < 0.0
                && let Some(axis) = side
                && let Some((y, depth, support)) =
                    self.projected_top(solids, axis, next.y, self.pos.y)
            {
                self.pos.y = y;
                self.pos[2 - axis] = depth;
                self.land(support);
                return;
            }
            self.pos = next;
        }
    }

    fn land(&mut self, support: Support) {
        if self.vel.y < -60.0 {
            self.events.push(Event::Land(-self.vel.y));
        }
        self.vel.y = 0.0;
        self.support = support;
    }

    /// While standing: is there still something under the feet? In side
    /// view fall back to any top on screen (snapping depth) before falling.
    fn keep_support(&mut self, solids: &Solids, side: Option<usize>) {
        if let Some(s) = self.support_3d(solids) {
            self.support = s;
            return;
        }
        if let Some(axis) = side
            && let Some((y, depth, s)) =
                self.projected_top(solids, axis, self.pos.y - 0.6, self.pos.y + 0.4)
        {
            self.pos.y = y;
            self.pos[2 - axis] = depth;
            self.support = s;
            return;
        }
        self.support = Support::None;
    }

    fn support_3d(&mut self, solids: &Solids) -> Option<Support> {
        let x0 = (self.pos.x - HALF + EPS).floor() as i32;
        let x1 = (self.pos.x + HALF - EPS).ceil() as i32;
        let z0 = (self.pos.z - HALF + EPS).floor() as i32;
        let z1 = (self.pos.z + HALF - EPS).ceil() as i32;
        let below = (self.pos.y - 0.5).floor() as i32;
        if (self.pos.y - self.pos.y.round()).abs() < 0.02 {
            for z in z0..z1 {
                for x in x0..x1 {
                    if solids.voxel(x, below, z) {
                        self.pos.y = self.pos.y.round();
                        return Some(Support::Static);
                    }
                }
            }
        }
        for (i, p) in solids.props.iter().enumerate() {
            if solids.ghost.is_some_and(|g| g.prop == Some(i)) {
                continue;
            }
            let top = p.max().y;
            let over = self.pos.x + HALF > p.pos.x
                && self.pos.x - HALF < p.max().x
                && self.pos.z + HALF > p.pos.z
                && self.pos.z - HALF < p.max().z;
            if over && (self.pos.y - top).abs() < 1.5 {
                let mut snapped = self.pos;
                snapped.y = top;
                if solids.free(snapped) {
                    self.pos = snapped;
                    return Some(Support::Prop(i));
                }
            }
        }
        None
    }

    /// Fez rule: the highest top surface crossed between `y_lo` and `y_hi`
    /// anywhere along the view depth under the feet on screen. Returns the
    /// landing height, the depth to snap to (as close to the current depth
    /// as the cat fits) and what supports it.
    fn projected_top(
        &self,
        solids: &Solids,
        axis: usize,
        y_lo: f32,
        y_hi: f32,
    ) -> Option<(f32, f32, Support)> {
        let daxis = 2 - axis;
        let extent = if daxis == 0 { W } else { D };
        let h0 = (self.pos[axis] - HALF + EPS).floor() as i32;
        let h1 = (self.pos[axis] + HALF - EPS).ceil() as i32;
        let cur = self.pos[daxis];
        let at = |h: i32, y: i32, d: i32| if axis == 0 { (h, y, d) } else { (d, y, h) };

        let top_lo = (y_lo + EPS).ceil() as i32;
        let top_hi = (y_hi + EPS).floor() as i32;
        let mut best: Option<(f32, f32, f32, Support)> = None; // (y, cost, depth, support)
        let consider =
            |y: f32, depth: f32, support: Support, best: &mut Option<(f32, f32, f32, Support)>| {
                let cost = (depth - cur).abs();
                if best.is_none_or(|(by, bc, _, _)| y > by || (y == by && cost < bc)) {
                    let mut p = self.pos;
                    p.y = y;
                    p[daxis] = depth;
                    if solids.free(p) {
                        *best = Some((y, cost, depth, support));
                    }
                }
            };

        for top in (top_lo..=top_hi).rev() {
            if top <= 0 {
                break;
            }
            let mut depths: Vec<i32> = Vec::new();
            for d in 0..extent {
                let found = (h0..h1).any(|h| {
                    let (x, y, z) = at(h, top - 1, d);
                    let (ax, ay, az) = at(h, top, d);
                    solids.voxel(x, y, z) && !solids.voxel(ax, ay, az)
                });
                if found {
                    depths.push(d);
                }
            }
            depths.sort_by_key(|&d| ((d as f32 + 0.5 - cur).abs() * 4.0) as i32);
            for d in depths.into_iter().take(24) {
                let depth = cur.clamp(d as f32 + 1.0 - HALF, d as f32 + HALF);
                consider(top as f32, depth, Support::Static, &mut best);
            }
            if best.is_some() {
                break;
            }
        }
        for (i, p) in solids.props.iter().enumerate() {
            if solids.ghost.is_some_and(|g| g.prop == Some(i)) {
                continue;
            }
            let top = p.max().y;
            let overlaps =
                self.pos[axis] + HALF > p.pos[axis] && self.pos[axis] - HALF < p.max()[axis];
            if overlaps && top > y_lo && top <= y_hi + 0.5 {
                let depth = cur.clamp(p.pos[daxis] + 1.0 - HALF, p.max()[daxis] - 1.0 + HALF);
                consider(top, depth, Support::Prop(i), &mut best);
            }
        }
        best.map(|(y, _, depth, support)| (y, depth, support))
    }

    fn check_water(&mut self, water: &mut Water) {
        let Some(h) = water.surface(self.pos.x, self.pos.z) else {
            self.swimming = false;
            return;
        };
        let depth = h - self.pos.y;
        if !self.swimming && depth > 4.0 {
            self.swimming = true;
            self.support = Support::None;
            let force = (-self.vel.y / 300.0).clamp(0.15, 1.0);
            water.impulse(self.pos.x, self.pos.z, 9.0, -60.0 * force);
            self.events.push(Event::Splash(force));
            self.vel.y *= 0.3;
        } else if self.swimming && depth < 2.0 {
            self.swimming = false;
        }
        if self.swimming {
            // float with the head and ears above the surface
            let target = h - 7.0;
            self.vel.y += ((target - self.pos.y) * 70.0 - self.vel.y * 7.0) * (1.0 / 120.0);
            if self.vel.x.abs() + self.vel.z.abs() > 5.0 && (self.pose.time * 7.0).fract() < 0.1 {
                water.impulse(self.pos.x, self.pos.z, 5.0, -6.0);
            }
        }
    }

    fn animate(&mut self, input: &Input, dt: f32) {
        let pose = &mut self.pose;
        pose.time += dt;
        let speed = (self.vel.x * self.vel.x + self.vel.z * self.vel.z).sqrt();
        let target_stride = if self.support != Support::None || self.swimming {
            (speed / WALK).min(1.3)
        } else {
            0.0
        };
        pose.stride += (target_stride - pose.stride) * (dt * 12.0).min(1.0);
        let old_phase = self.step_phase;
        pose.phase += speed * dt * 0.21;
        self.step_phase = pose.phase;
        if self.support != Support::None
            && speed > 10.0
            && (old_phase / std::f32::consts::PI).floor()
                != (self.step_phase / std::f32::consts::PI).floor()
        {
            self.events.push(Event::Step);
        }
        pose.crouch = if self.charge > 0.0 {
            self.charge + (pose.time * 30.0).sin() * 0.08 * self.charge
        } else if input.down && self.support != Support::None {
            0.4
        } else {
            0.0
        };
        pose.air = if self.support == Support::None && !self.swimming {
            (self.vel.y / 150.0).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        pose.swim = self.swimming;
        pose.tail = pose.time * if speed > 5.0 { 5.0 } else { 1.6 };
        self.blink_timer -= dt;
        if self.blink_timer < 0.0 {
            pose.blink = true;
            if self.blink_timer < -0.12 {
                pose.blink = false;
                self.blink_timer = 2.0 + (pose.time * 7.3).sin().abs() * 3.0;
            }
        }
    }
}
