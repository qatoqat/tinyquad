//! Game state and the fixed-step update: camera views and their animated
//! transitions, the cat, water, props, weather, the fish, and what the
//! audio should be doing.

use crate::audio::{Ambience, Audio};
use crate::cat::{self, Cat, Mode};
use crate::light::Lighting;
use crate::model::{self, Model};
use crate::props::{self, Prop};
use crate::rain::{Sound, Weather};
use crate::render::{Camera, SH, SW, Scene};
use crate::util::{Rng, approach, smoothstep};
use crate::water::Water;
use crate::world::{D, W, World};
use glam::{Vec3, vec3};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

const TURN_TIME: f32 = 0.42;

/// Keys for one step: the cat's controls plus camera commands.
#[derive(Clone, Copy, Default)]
pub struct Controls {
    pub cat: cat::Input,
    /// S / Down pressed this step: drops through platforms in side view.
    pub down_pressed: bool,
    pub rotate: i32,
    pub toggle_top: bool,
}

struct View {
    /// Quarter turns around the vertical axis (unbounded, so turning keeps
    /// going the same way instead of unwinding).
    yaw: i32,
    top: bool,
    from: (f32, f32),
    to: (f32, f32),
    t: f32,
}

impl View {
    fn angles(&self) -> (f32, f32) {
        let e = smoothstep(0.0, 1.0, self.t);
        (
            self.from.0 + (self.to.0 - self.from.0) * e,
            self.from.1 + (self.to.1 - self.from.1) * e,
        )
    }

    fn turning(&self) -> bool {
        self.t < 1.0
    }

    fn target(&self) -> (f32, f32) {
        // "top-down" is a 3/4 view: 45 degrees with a sqrt(2) vertical
        // stretch, so tops and fronts both show at full 1:1 pixel ratio
        (
            self.yaw as f32 * FRAC_PI_2,
            if self.top { FRAC_PI_4 } else { 0.0 },
        )
    }

    fn start(&mut self) {
        self.from = self.angles();
        self.to = self.target();
        self.t = 0.0;
    }

    /// Screen right and (top-down) screen up as world axes.
    fn axes(&self) -> (Vec3, Vec3) {
        let k = self.yaw.rem_euclid(4);
        let r = [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z][k as usize];
        let up = [Vec3::Z, -Vec3::X, -Vec3::Z, Vec3::X][k as usize];
        (r, up)
    }

    fn mode(&self) -> Mode {
        let (r, up) = self.axes();
        if self.top {
            Mode::Top { right: r, up }
        } else {
            let axis = if r.x != 0.0 { 0 } else { 2 };
            Mode::Side {
                axis,
                sign: r[axis],
            }
        }
    }
}

pub struct Game {
    world: World,
    lighting: Lighting,
    water: Water,
    weather: Weather,
    props: Vec<Prop>,
    prop_models: Vec<Model>,
    cat: Cat,
    cat_model: Model,
    fish_model: Model,
    fish_away: f32,
    view: View,
    center: Vec3,
    rng: Rng,
    time: f32,
    lamp: f32,
    lamp_dip: Vec<f32>,
    panel: f32,
}

impl Game {
    pub fn new() -> Game {
        let world = World::build();
        let lighting = Lighting::bake(&world);
        let water = Water::new(&world);
        let mut rng = Rng::new(0x5eed_cafe);
        let cat = Cat::new(world.spawn);
        let weather = Weather::new(&mut rng, world.spawn);
        let props = props::spawn_props();
        let prop_models = props.iter().map(|p| p.kind.model()).collect();
        let center = world.spawn + vec3(0.0, 40.0, 0.0);
        Game {
            cat_model: model::cat(&cat.pose),
            fish_model: model::fish(),
            fish_away: 0.0,
            world,
            lighting,
            water,
            weather,
            props,
            prop_models,
            cat,
            view: View {
                yaw: 0,
                top: false,
                from: (0.0, 0.0),
                to: (0.0, 0.0),
                t: 1.0,
            },
            center,
            rng,
            time: 0.0,
            lamp: 1.0,
            lamp_dip: Vec::new(),
            panel: 1.0,
        }
    }

    pub fn step(&mut self, c: &Controls, dt: f32) {
        self.time += dt;
        if !self.view.turning() {
            if c.rotate != 0 {
                self.view.yaw += c.rotate;
                self.view.start();
            } else if c.toggle_top {
                self.view.top = !self.view.top;
                self.view.start();
            }
        }
        if self.view.turning() {
            // the cat and the boxes hold still while the camera turns (Fez)
            self.view.t = (self.view.t + dt / TURN_TIME).min(1.0);
        } else {
            // props first: the cat then rides them and is pushed out of them
            props::step(
                &mut self.props,
                &self.world,
                &mut self.water,
                &mut self.rng,
                dt,
            );
            let mut input = c.cat;
            if let Mode::Side { .. } = self.view.mode() {
                input.drop_pressed |= c.down_pressed;
            }
            self.cat.step(
                &self.world,
                &mut self.props,
                &mut self.water,
                &input,
                self.view.mode(),
                dt,
            );
            if self.cat.pos.y < -20.0 {
                self.cat = Cat::new(self.world.spawn);
            }
        }
        self.water.step(dt, &mut self.rng);
        let cat_c = self.cat.center();
        self.weather.focus = self.center;
        self.weather.step(
            &self.world,
            &mut self.water,
            &self.props,
            &self.cat,
            &mut self.rng,
            dt,
        );

        // the fish
        if self.fish_away > 0.0 {
            self.fish_away -= dt;
        } else if (cat_c - (self.world.fish + vec3(0.0, 4.0, 0.0))).length() < 12.0 {
            self.fish_away = 30.0;
            self.weather
                .sparkle(self.world.fish + vec3(0.0, 4.0, 0.0), &mut self.rng);
        }

        self.update_lights(dt);
        self.update_camera(dt);
    }

    fn update_lights(&mut self, dt: f32) {
        if let Some(level) = self.lamp_dip.pop() {
            self.lamp = level;
        } else {
            self.lamp += (1.0 - self.lamp) * approach(20.0, dt);
            if self.rng.chance(dt / 14.0) {
                // the old lamp stutters now and then
                let n = 8 + self.rng.below(40) as usize;
                self.lamp_dip = (0..n)
                    .map(|_| {
                        if self.rng.chance(0.5) {
                            self.rng.range(0.05, 0.4)
                        } else {
                            1.0
                        }
                    })
                    .collect();
            }
        }
        self.panel = 0.95 + 0.05 * (self.time * 13.0).sin() * (self.time * 0.7).sin();
    }

    fn update_camera(&mut self, dt: f32) {
        let (r, up) = self.view.axes();
        let cat_c = self.cat.center();
        let mut target = cat_c + self.cat.facing * 14.0;
        let ext = |axis: Vec3| if axis.x != 0.0 { W as f32 } else { D as f32 };
        let clamp_along = |p: Vec3, axis: Vec3, half: f32| {
            let e = ext(axis);
            let v = p.dot(axis.abs());
            let clamped = if e > half * 2.0 - 40.0 {
                v.clamp(half - 20.0, e - half + 20.0)
            } else {
                e * 0.5
            };
            p + axis.abs() * (clamped - v)
        };
        target = clamp_along(target, r, SW as f32 * 0.5);
        if self.view.top {
            target = clamp_along(target, up, SH as f32 * 0.5);
        }
        target.y = (cat_c.y + 12.0).clamp(74.0, 84.0);
        self.center += (target - self.center) * approach(5.0, dt);
    }

    /// Pushes this frame's ambience and one-shots to the audio thread.
    pub fn sound(&mut self, audio: &mut Audio) {
        let cat_c = self.cat.center();
        let (r, _) = self.view.axes();
        let pan = |p: Vec3| ((p - cat_c).dot(r) / 120.0).clamp(-1.0, 1.0);
        let near = |p: Vec3, range: f32| (1.0 - (p - cat_c).length() / range).clamp(0.0, 1.0);

        let covered = self.world.height_at(cat_c.x as i32, cat_c.z as i32) as f32 > cat_c.y + 8.0;
        let drain = near(self.world.drain, 150.0).max(near(self.world.downspout, 150.0) * 0.8);
        let drain_src = if near(self.world.drain, 150.0) >= near(self.world.downspout, 150.0) * 0.8
        {
            self.world.drain
        } else {
            self.world.downspout
        };
        audio.set_ambience(Ambience {
            rain: self.weather.intensity * if covered { 0.75 } else { 1.0 },
            roof: (self.weather.roof_noise * 0.6 + if covered { 0.7 } else { 0.0 }).min(1.0),
            drain: drain * drain * (0.5 + 0.5 * self.weather.intensity),
            drain_pan: pan(drain_src),
            muffle: if self.cat.swimming { 0.55 } else { 0.0 },
            master: 0.9,
        });
        while let Some((strength, distance)) = self.weather.take_thunder() {
            audio.thunder(strength, distance);
        }
        for s in std::mem::take(&mut self.weather.sounds) {
            match s {
                Sound::Drip(p) => audio.drip(near(p, 120.0) * 0.8, pan(p)),
            }
        }
        for e in std::mem::take(&mut self.cat.events) {
            match e {
                cat::Event::Splash(force) => {
                    let at = vec3(
                        self.cat.pos.x,
                        self.water
                            .surface(self.cat.pos.x, self.cat.pos.z)
                            .unwrap_or(self.cat.pos.y),
                        self.cat.pos.z,
                    );
                    self.weather.splash(at, force, &mut self.rng);
                    audio.splash(force, 0.0);
                }
                cat::Event::Land(v) => audio.step((v / 300.0).clamp(0.3, 1.0), 0.0),
                cat::Event::Step => audio.step(0.18, 0.0),
                cat::Event::Jump => {}
            }
        }
    }

    pub fn scene(&mut self) -> Scene<'_> {
        self.cat_model = model::cat(&self.cat.pose);
        let (yaw, pitch) = self.view.angles();
        Scene {
            world: &self.world,
            lighting: &self.lighting,
            water: &self.water,
            weather: &self.weather,
            props: &self.props,
            prop_models: &self.prop_models,
            cat: &self.cat,
            cat_model: &self.cat_model,
            fish: (self.fish_away <= 0.0).then_some((self.world.fish, &self.fish_model)),
            camera: Camera {
                center: self.center,
                yaw,
                pitch,
            },
            time: self.time,
            lamp: self.lamp,
            panel: self.panel,
        }
    }
}

#[cfg(test)]
impl Game {
    pub fn cat_pos(&self) -> Vec3 {
        self.cat.pos
    }

    pub fn fish_taken(&self) -> bool {
        self.fish_away > 0.0
    }

    pub fn cat_is_free(&self) -> bool {
        let free = self.cat.is_free(&self.world, &self.props);
        if !free {
            let c = self.cat.pos;
            for p in &self.props {
                let hit = c.x + 6.0 > p.pos.x
                    && c.x - 6.0 < p.max().x
                    && c.z + 6.0 > p.pos.z
                    && c.z - 6.0 < p.max().z
                    && c.y + 12.0 > p.pos.y
                    && c.y < p.max().y;
                if hit {
                    eprintln!(
                        "overlapping prop {:?} .. {:?} vel {:?} turning {}",
                        p.pos,
                        p.max(),
                        p.vel,
                        self.view.turning()
                    );
                }
            }
            eprintln!("{}", self.debug());
            let p = self.cat.pos;
            for y in (p.y - 0.0).floor() as i32..(p.y + 12.0).ceil() as i32 {
                for z in (p.z - 6.0).floor() as i32..(p.z + 6.0).ceil() as i32 {
                    for x in (p.x - 6.0).floor() as i32..(p.x + 6.0).ceil() as i32 {
                        if self.world.solid(x, y, z) {
                            eprintln!("voxel {x} {y} {z} {:?}", self.world.get(x, y, z));
                        }
                    }
                }
            }
        }
        free
    }

    pub fn debug(&self) -> String {
        format!(
            "cat pos {:.1?} vel {:.1?} support {:?} swim {} view yaw {} top {}",
            self.cat.pos,
            self.cat.vel,
            self.cat.support,
            self.cat.swimming,
            self.view.yaw,
            self.view.top
        )
    }
}
