//! Software renderer: the voxel world drawn orthographically into a
//! 320x180 frame buffer, one voxel per pixel.
//!
//! 1. **Splat** every surface voxel of the visible chunks into a depth +
//!    shading buffer (2x2 splats while the camera turns), then the moving
//!    models, the glass and the water volume (front/back depth per pixel).
//! 2. **Composite** per pixel: baked sky/lamp/panel light scaled by flicker
//!    and lightning, dynamic shadows of the cat and boxes, wet rims, depth
//!    fog, translucent water and glass, the lamp's light cone in the rain,
//!    and the sky with clouds, a distant skyline and lightning bolts.
//! 3. Rain and particles, bloom, then a soft tone curve with ordered
//!    dithering to a reduced palette for the pixel-art look.

use crate::cat::{self, Cat};
use crate::light::{self, F_CAT, F_DYNAMIC, F_EMISSIVE, F_GLASS, F_TOP, F_WET, Lighting, Shade};
use crate::model::Model;
use crate::props::Prop;
use crate::rain::{Kind, Weather};
use crate::util::{Rgb, hash3, lerp, mix, noise3, smoothstep};
use crate::water::Water;
use crate::world::{D, GROUND, H, W, World};
use glam::{Vec3, vec3};

pub const SW: usize = 320;
pub const SH: usize = 180;
const HW: f32 = SW as f32 * 0.5;
const HH: f32 = SH as f32 * 0.5;
const N: usize = SW * SH;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub center: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// Screen axes in world space for a yaw/pitch pair.
#[derive(Clone, Copy)]
pub struct Basis {
    pub r: Vec3,
    pub u: Vec3,
    pub f: Vec3,
    /// Vertical stretch that keeps the plane facing the camera square
    /// while pitching (at 45 degrees both tops and fronts are 1:1).
    pub stretch: f32,
    /// Pixel-perfect view: yaw on a quarter turn, pitch 0 or 45 degrees,
    /// so every voxel lands on exactly one pixel.
    pub cardinal: bool,
    /// Horizontal forward (view direction without pitch).
    pub fh: Vec3,
}

impl Camera {
    pub fn basis(&self) -> Basis {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let r = vec3(cy, 0.0, sy);
        let fh = vec3(-sy, 0.0, cy);
        let f = fh * cp - Vec3::Y * sp;
        let u = Vec3::Y * cp + fh * sp;
        let snapped = |a: f32, step: f32| (a / step - (a / step).round()).abs() < 1e-3;
        Basis {
            r,
            u,
            f,
            stretch: 1.0 / sp.abs().max(cp.abs()),
            cardinal: snapped(self.yaw, std::f32::consts::FRAC_PI_2)
                && snapped(self.pitch, std::f32::consts::FRAC_PI_4),
            fh,
        }
    }
}

impl Basis {
    /// World offset from the camera center to (screen x, screen y, depth).
    #[inline(always)]
    pub fn project(&self, rel: Vec3) -> (f32, f32, f32) {
        (
            rel.dot(self.r) + HW,
            HH - rel.dot(self.u) * self.stretch,
            rel.dot(self.f),
        )
    }

    /// World position under pixel center (x, y) at depth `d`.
    pub fn unproject(&self, center: Vec3, x: usize, y: usize, d: f32) -> Vec3 {
        let sx = x as f32 + 0.5 - HW;
        let sy = (HH - y as f32 - 0.5) / self.stretch;
        center + self.r * sx + self.u * sy + self.f * d
    }

    /// True top-down share of the view (0 = side, 1 = top).
    pub fn topness(&self) -> f32 {
        -self.f.y
    }
}

/// Everything the renderer looks at in one frame.
pub struct Scene<'a> {
    pub world: &'a World,
    pub lighting: &'a Lighting,
    pub water: &'a Water,
    pub weather: &'a Weather,
    pub props: &'a [Prop],
    pub prop_models: &'a [Model],
    pub cat: &'a Cat,
    pub cat_model: &'a Model,
    pub fish: Option<(Vec3, &'a Model)>,
    pub camera: Camera,
    pub time: f32,
    /// Street lamp brightness (flicker), 0..1.
    pub lamp: f32,
    /// Advert / sign / window brightness, 0..1.
    pub panel: f32,
}

/// Background ray setup for one frame (see `background_setup`).
struct Background {
    y0: f32,
    ydx: f32,
    ydy: f32,
    a0: f32,
    adx: f32,
    ady: f32,
}

/// Per-object lighting environment for moving models.
struct Env {
    sky: f32,
    lamp: Rgb,
    other: Rgb,
    lamp_dir: Vec3,
}

/// The static world projected once for a cardinal view; while the camera
/// stays in that view each frame just copies the visible window out of it.
struct WorldImage {
    key: (i32, i32, i32),
    w: usize,
    h: usize,
    rmin: f32,
    umax: f32,
    depth: Vec<f32>,
    shade: Vec<Shade>,
    glass_d: Vec<f32>,
    glass: Vec<Shade>,
}

pub struct Renderer {
    #[cfg(test)]
    pub min_times: [f64; 7],
    /// Time the last frame took to render, in milliseconds.
    pub frame_ms: f64,
    image: Option<WorldImage>,
    depth: Vec<f32>,
    shade: Vec<Shade>,
    oid: Vec<u8>,
    glass_d: Vec<f32>,
    glass: Vec<Shade>,
    wf: Vec<f32>,
    wb: Vec<f32>,
    wcell: Vec<u32>,
    cat_d: Vec<f32>,
    hdr: Vec<Rgb>,
    half: Vec<Rgb>,
    half_tmp: Vec<Rgb>,
    bolt: Vec<bool>,
    clouds: Vec<f32>,
    skyline: [[(i32, f32); SW]; 2],
    tone: Vec<f32>,
    pub rgba: Vec<u8>,
    table: [f32; 256],
    cam: Vec3,
    basis: Basis,
}

/// Bayer 4x4 thresholds for ordered dithering.
const BAYER: [f32; 16] = {
    let order = [
        0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0,
    ];
    let mut t = [0.0; 16];
    let mut i = 0;
    while i < 16 {
        t[i] = (order[i] + 0.5) / 16.0;
        i += 1;
    }
    t
};
/// Tone curve lookup: HDR value * TONE_SCALE indexes it.
const TONE_LEN: usize = 4096;
const TONE_SCALE: f32 = 512.0;
const LEVELS: f32 = 40.0;
const CLOUD_W: usize = 512;
const CLOUD_H: usize = 192;

impl Renderer {
    pub fn new() -> Renderer {
        Renderer {
            #[cfg(test)]
            min_times: [f64::MAX; 7],
            frame_ms: 0.0,
            image: None,
            depth: vec![f32::INFINITY; N],
            shade: vec![Shade::default(); N],
            oid: vec![0; N],
            glass_d: vec![f32::INFINITY; N],
            glass: vec![Shade::default(); N],
            wf: vec![f32::INFINITY; N],
            wb: vec![f32::NEG_INFINITY; N],
            wcell: vec![0; N],
            cat_d: vec![f32::INFINITY; N],
            hdr: vec![[0.0; 3]; N],
            half: vec![[0.0; 3]; N / 4],
            half_tmp: vec![[0.0; 3]; N / 4],
            bolt: vec![false; N],
            clouds: (0..CLOUD_W * CLOUD_H)
                .map(|i| {
                    let (u, v) = ((i % CLOUD_W) as f32, (i / CLOUD_W) as f32);
                    let n = tiled_noise(u * 0.05, v * 0.09, CLOUD_W as f32 * 0.05) * 0.6
                        + tiled_noise(u * 0.13, v * 0.2, CLOUD_W as f32 * 0.13) * 0.4;
                    smoothstep(0.45, 0.8, n)
                })
                .collect(),
            skyline: [[(0, 0.0); SW]; 2],
            tone: (0..TONE_LEN)
                .map(|i| {
                    let v = i as f32 / TONE_SCALE;
                    // soft shoulder, then scaled to palette levels
                    (v / (1.0 + v * 0.35) * 1.18).min(1.0) * LEVELS
                })
                .collect(),
            rgba: vec![255; N * 4],
            table: light::hdr_table(),
            cam: Vec3::ZERO,
            basis: Camera {
                center: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
            }
            .basis(),
        }
    }

    pub fn render(&mut self, s: &Scene) {
        self.basis = s.camera.basis();
        // whole-pixel camera in the cardinal views keeps pixel art crisp
        self.cam = if self.basis.cardinal {
            s.camera.center.round()
        } else {
            s.camera.center
        };

        self.depth.fill(f32::INFINITY);
        self.glass_d.fill(f32::INFINITY);
        self.wf.fill(f32::INFINITY);
        self.wb.fill(f32::NEG_INFINITY);
        self.cat_d.fill(f32::INFINITY);
        self.oid.fill(0);

        let now = tinyquad::date::now;
        let mut marks = [now(); 8];
        if self.basis.cardinal {
            self.copy_world(s.lighting);
        } else {
            self.splat_world(s.lighting);
        }
        marks[1] = now();
        self.splat_dynamic(s);
        marks[2] = now();
        self.splat_water(s.water);
        marks[3] = now();
        self.draw_bolt(s.weather);
        self.composite(s);
        marks[4] = now();
        self.draw_particles(s);
        marks[5] = now();
        self.bloom();
        marks[6] = now();
        self.finish();
        marks[7] = now();
        self.frame_ms = (marks[7] - marks[0]) * 1000.0;
        #[cfg(test)]
        for (k, m) in self.min_times.iter_mut().enumerate() {
            *m = m.min(marks[k + 1] - marks[k]);
        }
    }

    #[inline(always)]
    fn plot(&mut self, sx: f32, sy: f32, d: f32, shade: Shade, oid: u8, size: i32) {
        let (x0, y0) = if size == 1 {
            (sx.floor() as i32, sy.floor() as i32)
        } else {
            ((sx - 0.5).floor() as i32, (sy - 0.5).floor() as i32)
        };
        for y in y0..y0 + size {
            if y as u32 >= SH as u32 {
                continue;
            }
            for x in x0..x0 + size {
                if x as u32 >= SW as u32 {
                    continue;
                }
                let i = y as usize * SW + x as usize;
                if d < self.depth[i] {
                    self.depth[i] = d;
                    self.shade[i] = shade;
                    self.oid[i] = oid;
                }
            }
        }
    }

    fn splat_world(&mut self, lighting: &Lighting) {
        let b = self.basis;
        let size = if b.cardinal { 1 } else { 2 };
        let c = self.cam;
        for chunk in &lighting.chunks {
            // cull chunks whose projected box misses the screen
            let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for k in 0..8 {
                let p = vec3(
                    if k & 1 == 0 { chunk.min.x } else { chunk.max.x },
                    if k & 2 == 0 { chunk.min.y } else { chunk.max.y },
                    if k & 4 == 0 { chunk.min.z } else { chunk.max.z },
                );
                let (x, y, _) = b.project(p - c);
                lo = lo.min(vec3(x, y, 0.0));
                hi = hi.max(vec3(x, y, 0.0));
            }
            if hi.x < -2.0 || hi.y < -2.0 || lo.x > SW as f32 + 2.0 || lo.y > SH as f32 + 2.0 {
                continue;
            }
            let o = vec3(0.5, 0.5, 0.5) - c;
            for v in &chunk.voxels {
                let rel = vec3(
                    f32::from(v.pos[0]),
                    f32::from(v.pos[1]),
                    f32::from(v.pos[2]),
                ) + o;
                let (x, y, d) = b.project(rel);
                if v.shade.flags & F_GLASS != 0 {
                    self.plot_glass(x, y, d, v.shade, size);
                } else {
                    self.plot(x, y, d, v.shade, 0, size);
                }
            }
        }
    }

    /// Cardinal views: project the whole world once into an image, then
    /// copy the window under the camera.
    fn copy_world(&mut self, lighting: &Lighting) {
        let b = self.basis;
        let us = b.u * b.stretch;
        let key = (
            b.r.x.round() as i32 + 2 * b.r.z.round() as i32,
            us.y.round() as i32,
            us.x.round() as i32 + 2 * us.z.round() as i32,
        );
        if self.image.as_ref().is_none_or(|img| img.key != key) {
            self.image = Some(Self::build_image(lighting, &b, key));
        }
        let img = self.image.as_ref().unwrap();
        let c = self.cam;
        let ox = (c.dot(b.r) - HW - img.rmin).round() as i64;
        let oy = (img.umax - HH - c.dot(us)).round() as i64;
        let dshift = -c.dot(b.f);
        for y in 0..SH {
            let iy = oy + y as i64;
            let row = y * SW;
            if iy < 0 || iy >= img.h as i64 {
                self.depth[row..row + SW].fill(f32::INFINITY);
                self.glass_d[row..row + SW].fill(f32::INFINITY);
                continue;
            }
            for x in 0..SW {
                let ix = ox + x as i64;
                let i = row + x;
                if ix < 0 || ix >= img.w as i64 {
                    self.depth[i] = f32::INFINITY;
                    self.glass_d[i] = f32::INFINITY;
                    continue;
                }
                let j = iy as usize * img.w + ix as usize;
                self.depth[i] = img.depth[j] + dshift;
                self.shade[i] = img.shade[j];
                self.glass_d[i] = img.glass_d[j] + dshift;
                self.glass[i] = img.glass[j];
            }
        }
    }

    fn build_image(lighting: &Lighting, b: &Basis, key: (i32, i32, i32)) -> WorldImage {
        let us = b.u * b.stretch;
        let (mut rmin, mut rmax, mut umin, mut umax) = (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        );
        for k in 0..8 {
            let p = vec3(
                if k & 1 == 0 { 0.0 } else { W as f32 },
                if k & 2 == 0 { 0.0 } else { H as f32 },
                if k & 4 == 0 { 0.0 } else { D as f32 },
            );
            rmin = rmin.min(p.dot(b.r));
            rmax = rmax.max(p.dot(b.r));
            umin = umin.min(p.dot(us));
            umax = umax.max(p.dot(us));
        }
        let (w, h) = (
            (rmax - rmin).round() as usize + 1,
            (umax - umin).round() as usize + 1,
        );
        let mut img = WorldImage {
            key,
            w,
            h,
            rmin,
            umax,
            depth: vec![f32::INFINITY; w * h],
            shade: vec![Shade::default(); w * h],
            glass_d: vec![f32::INFINITY; w * h],
            glass: vec![Shade::default(); w * h],
        };
        for chunk in &lighting.chunks {
            for v in &chunk.voxels {
                let p = vec3(
                    f32::from(v.pos[0]) + 0.5,
                    f32::from(v.pos[1]) + 0.5,
                    f32::from(v.pos[2]) + 0.5,
                );
                let ix = (p.dot(b.r) - rmin).floor() as usize;
                let iy = (umax - p.dot(us)).floor() as usize;
                if ix >= w || iy >= h {
                    continue;
                }
                let j = iy * w + ix;
                let d = p.dot(b.f);
                if v.shade.flags & F_GLASS != 0 {
                    if d < img.glass_d[j] {
                        img.glass_d[j] = d;
                        img.glass[j] = v.shade;
                    }
                } else if d < img.depth[j] {
                    img.depth[j] = d;
                    img.shade[j] = v.shade;
                }
            }
        }
        img
    }

    fn plot_glass(&mut self, sx: f32, sy: f32, d: f32, shade: Shade, size: i32) {
        let (x0, y0) = if size == 1 {
            (sx.floor() as i32, sy.floor() as i32)
        } else {
            ((sx - 0.5).floor() as i32, (sy - 0.5).floor() as i32)
        };
        for y in y0..y0 + size {
            for x in x0..x0 + size {
                if (x as u32) < SW as u32 && (y as u32) < SH as u32 {
                    let i = y as usize * SW + x as usize;
                    if d < self.glass_d[i] {
                        self.glass_d[i] = d;
                        self.glass[i] = shade;
                    }
                }
            }
        }
    }

    fn env_at(s: &Scene, p: Vec3) -> Env {
        let covered = s.world.height_at(p.x as i32, p.z as i32) as f32 > p.y + 2.0;
        let (lamp, other) = s.lighting.grid_at(p);
        let lamp_pos = s.world.lights[0].pos;
        Env {
            sky: if covered { 0.3 } else { 1.0 },
            lamp,
            other,
            lamp_dir: (lamp_pos - p).normalize_or_zero(),
        }
    }

    /// Splats a model whose local axes are (fwd, up, lateral) at `origin`.
    #[allow(clippy::too_many_arguments)]
    fn splat_model(
        &mut self,
        model: &Model,
        origin: Vec3,
        fwd: Vec3,
        lat: Vec3,
        env: &Env,
        oid: u8,
        flags: u8,
        cat: bool,
    ) {
        let b = self.basis;
        let size = if b.cardinal { 1 } else { 2 };
        let c = self.cam;
        for v in &model.voxels {
            let (a, y, l) = (
                f32::from(v.p[0]) + 0.5,
                f32::from(v.p[1]) + 0.5,
                f32::from(v.p[2]) + 0.5,
            );
            let world = origin + fwd * a + Vec3::Y * y + lat * l;
            let (sx, sy, d) = b.project(world - c);
            let shade = if v.emissive {
                Shade {
                    amb: [0; 3],
                    lamp: [0; 3],
                    other: light::enc_rgb_hdr(v.color),
                    flags: flags | F_EMISSIVE,
                    spec: 0,
                    _pad: 0,
                }
            } else {
                let n = (fwd * f32::from(v.n[0])
                    + Vec3::Y * f32::from(v.n[1])
                    + lat * f32::from(v.n[2]))
                .normalize_or_zero();
                let ndl = 0.3 + 0.7 * n.dot(env.lamp_dir).max(0.0);
                let up = 0.6 + 0.4 * n.y.max(0.0);
                let amb = [
                    v.color[0] * (light::SKY[0] * env.sky * up + light::FILL[0]),
                    v.color[1] * (light::SKY[1] * env.sky * up + light::FILL[1]),
                    v.color[2] * (light::SKY[2] * env.sky * up + light::FILL[2]),
                ];
                Shade {
                    amb: light::enc_rgb_ldr(amb),
                    lamp: light::enc_rgb_hdr(std::array::from_fn(|k| {
                        v.color[k] * env.lamp[k] * ndl
                    })),
                    other: light::enc_rgb_hdr(std::array::from_fn(|k| {
                        v.color[k] * env.other[k] * 0.8
                    })),
                    flags: flags | if n.y > 0.5 { F_TOP } else { 0 },
                    spec: 0,
                    _pad: 0,
                }
            };
            self.plot(sx, sy, d, shade, oid, size);
            if cat {
                let (x, y) = (sx.floor() as i32, sy.floor() as i32);
                if (x as u32) < SW as u32 && (y as u32) < SH as u32 {
                    let i = y as usize * SW + x as usize;
                    self.cat_d[i] = self.cat_d[i].min(d);
                }
            }
        }
    }

    fn splat_dynamic(&mut self, s: &Scene) {
        let cat = s.cat;
        let fwd = cat.facing;
        let lat = vec3(-fwd.z, 0.0, fwd.x);
        let env = Self::env_at(s, cat.center());
        let mut origin = cat.pos;
        if cat.swimming {
            origin.y -= 1.0;
        }
        self.splat_model(
            s.cat_model,
            origin,
            fwd,
            lat,
            &env,
            1,
            F_DYNAMIC | F_CAT,
            true,
        );
        for (i, (p, m)) in s.props.iter().zip(s.prop_models).enumerate() {
            let env = Self::env_at(s, p.center());
            self.splat_model(
                m,
                p.pos,
                Vec3::X,
                Vec3::Z,
                &env,
                2 + i as u8,
                F_DYNAMIC,
                false,
            );
        }
        if let Some((pos, model)) = s.fish {
            let env = Self::env_at(s, pos);
            let spin = (s.time * 1.5).sin();
            let (fwd, lat) = if spin > 0.0 {
                (Vec3::X, Vec3::Z)
            } else {
                (-Vec3::X, -Vec3::Z)
            };
            let bob = vec3(0.0, 2.0 + (s.time * 3.0).sin() * 1.5, 0.0);
            self.splat_model(
                model,
                pos + bob - fwd * 5.0 - lat,
                fwd,
                lat,
                &env,
                250,
                F_DYNAMIC,
                false,
            );
        }
    }

    fn splat_water(&mut self, water: &Water) {
        let b = self.basis;
        let c = self.cam;
        let size = if b.cardinal { 1 } else { 2 };
        let plot = |r: &mut Renderer, p: Vec3, cell: u32| {
            let (sx, sy, d) = b.project(p - c);
            let (x0, y0) = if size == 1 {
                (sx.floor() as i32, sy.floor() as i32)
            } else {
                ((sx - 0.5).floor() as i32, (sy - 0.5).floor() as i32)
            };
            for y in y0..y0 + size {
                for x in x0..x0 + size {
                    if (x as u32) < SW as u32 && (y as u32) < SH as u32 {
                        let i = y as usize * SW + x as usize;
                        if d < r.wf[i] {
                            r.wf[i] = d;
                            r.wcell[i] = cell;
                        }
                        if d > r.wb[i] {
                            r.wb[i] = d;
                        }
                    }
                }
            }
        };
        for &ci in &water.cells {
            let i = ci as usize;
            let (x, z) = ((ci as i32 % W) as f32, (ci as i32 / W) as f32);
            let h = water.h[i];
            plot(self, vec3(x + 0.5, h - 0.5, z + 0.5), ci + 1);
            // cut faces where the canal meets the edge of the world
            let edge_x = if x == 0.0 {
                Some(0.0)
            } else if x == (W - 1) as f32 {
                Some(W as f32)
            } else {
                None
            };
            let edge_z = if z == 0.0 {
                Some(0.0)
            } else if z == (D - 1) as f32 {
                Some(D as f32)
            } else {
                None
            };
            if edge_x.is_some() || edge_z.is_some() {
                let mut y = water.floor[i];
                while y < h - 1.0 {
                    let px = edge_x.unwrap_or(x + 0.5);
                    let pz = edge_z.unwrap_or(z + 0.5);
                    plot(self, vec3(px, y + 0.5, pz), 0);
                    y += 1.0;
                }
            }
        }
    }

    fn draw_bolt(&mut self, weather: &Weather) {
        self.bolt.fill(false);
        let Some((seed, age, xfrac)) = weather.bolt else {
            return;
        };
        if age > 0.3 {
            return;
        }
        let mut x = xfrac * SW as f32;
        let bottom = (SH as f32 * 0.55) as i32;
        let mut branch: Option<(f32, i32)> = None;
        for y in 0..bottom {
            let r = hash3(seed as i32, y, 1);
            x += ((r % 5) as f32 - 2.0) * 0.8;
            self.mark_bolt(x as i32, y);
            if branch.is_none() && r.is_multiple_of(23) && y > 10 {
                branch = Some((x, y));
            }
        }
        if let Some((mut bx, by)) = branch {
            let dir = if hash3(seed as i32, 7, 7).is_multiple_of(2) {
                1.2
            } else {
                -1.2
            };
            for y in by..(by + 30).min(bottom) {
                bx += dir + ((hash3(seed as i32, y, 3) % 3) as f32 - 1.0);
                self.mark_bolt(bx as i32, y);
            }
        }
    }

    fn mark_bolt(&mut self, x: i32, y: i32) {
        if (x as u32) < SW as u32 && (y as u32) < SH as u32 {
            self.bolt[y as usize * SW + x as usize] = true;
        }
    }

    fn composite(&mut self, s: &Scene) {
        let bg = self.background_setup(s);
        let b = self.basis;
        let c = self.cam;
        let t = &self.table;
        let flash = s.weather.flash;
        let topness = b.topness().clamp(0.0, 1.0);
        let lamp_pos = s.world.lights[0].pos;
        let amb_scale = 1.0 + flash * 5.0;
        let lamp_scale = s.lamp;
        let other_scale = s.panel;
        let fog_col = mix([0.055, 0.07, 0.11], [0.5, 0.52, 0.65], flash * 0.6);
        let view = -b.f;

        // depth range of the world along the view, for fog
        let (mut dmin, mut dmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for k in 0..8 {
            let p = vec3(
                if k & 1 == 0 { 0.0 } else { W as f32 },
                if k & 2 == 0 { 0.0 } else { H as f32 },
                if k & 4 == 0 { 0.0 } else { D as f32 },
            );
            let d = (p - c).dot(b.f);
            dmin = dmin.min(d);
            dmax = dmax.max(d);
        }
        let fog_span = 1.0 / (dmax - dmin).max(1.0);

        // shadow casters: the cat and the floating props
        let mut casters: Vec<(Vec3, Vec3, u8)> = Vec::with_capacity(8);
        casters.push((
            s.cat.pos - vec3(cat::HALF - 1.0, 0.0, cat::HALF - 1.0),
            s.cat.pos + vec3(cat::HALF - 1.0, cat::HEIGHT, cat::HALF - 1.0),
            1,
        ));
        for (i, p) in s.props.iter().enumerate() {
            casters.push((p.pos, p.max(), 2 + i as u8));
        }

        // screen box where a caster can shadow anything (its box + reach)
        let (mut sx0, mut sy0, mut sx1, mut sy1) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        for &(lo, hi, _) in &casters {
            for k in 0..8 {
                let reach = Vec3::splat(70.0);
                let (lo, hi) = (lo - reach, hi + reach);
                let p = vec3(
                    if k & 1 == 0 { lo.x } else { hi.x },
                    if k & 2 == 0 { lo.y } else { hi.y },
                    if k & 4 == 0 { lo.z } else { hi.z },
                );
                let (x, y, _) = b.project(p - c);
                sx0 = sx0.min(x);
                sy0 = sy0.min(y);
                sx1 = sx1.max(x);
                sy1 = sy1.max(y);
            }
        }

        // the lamp's light cone, as seen on screen
        let (lsx, lsy, ld) = b.project(lamp_pos - c);
        let down = vec3(0.0, -1.0, 0.0);
        let axis2 = (down.dot(b.r), -down.dot(b.u) * b.stretch);
        let axis_len = (axis2.0 * axis2.0 + axis2.1 * axis2.1).sqrt();
        let beam_len = (lamp_pos.y - GROUND as f32) * b.stretch;
        let beam_k = 0.16 * lamp_scale * (0.5 + 0.5 * s.weather.intensity);

        for y in 0..SH {
            for x in 0..SW {
                let i = y * SW + x;
                let d = self.depth[i];
                let mut col: Rgb;
                if d.is_finite() {
                    let sh = self.shade[i];
                    let amb = [
                        f32::from(sh.amb[0]) / 255.0,
                        f32::from(sh.amb[1]) / 255.0,
                        f32::from(sh.amb[2]) / 255.0,
                    ];
                    let lamp = [
                        t[sh.lamp[0] as usize],
                        t[sh.lamp[1] as usize],
                        t[sh.lamp[2] as usize],
                    ];
                    let other = [
                        t[sh.other[0] as usize],
                        t[sh.other[1] as usize],
                        t[sh.other[2] as usize],
                    ];
                    let ft = ((d - dmin) * fog_span).clamp(0.0, 1.0);
                    let fog = ft * (0.45 + 0.55 * ft);
                    if sh.flags & F_EMISSIVE != 0 {
                        col =
                            std::array::from_fn(|k| lamp[k] * lamp_scale + other[k] * other_scale);
                        col = mix(col, fog_col, fog * 0.25);
                    } else {
                        let mut lamp_vis = lamp_scale;
                        let p = b.unproject(c, x, y, d);
                        let (fx, fy) = (x as f32, y as f32);
                        if lamp[0] + lamp[1] > 0.02
                            && fx >= sx0
                            && fx <= sx1
                            && fy >= sy0
                            && fy <= sy1
                        {
                            let oid = self.oid[i];
                            for &(lo, hi, id) in &casters {
                                let mid = (lo + hi) * 0.5;
                                if id != oid
                                    && (p - mid).length_squared() < 70.0 * 70.0
                                    && segment_hits_box(p, lamp_pos, lo, hi)
                                {
                                    lamp_vis *= 0.18;
                                    break;
                                }
                            }
                        }
                        col = std::array::from_fn(|k| {
                            amb[k] * amb_scale + lamp[k] * lamp_vis + other[k] * other_scale
                        });
                        if sh.flags & F_TOP != 0 {
                            // wet top edges catch light; in top view, wet
                            // ground mirrors the lamp
                            let above = if y > 0 {
                                self.depth[i - SW]
                            } else {
                                f32::INFINITY
                            };
                            let rim = (above - d > 3.0) as u8 as f32 * (1.0 - topness);
                            let wet = if sh.flags & F_WET != 0 { 1.0 } else { 0.35 };
                            let spec = t[sh.spec as usize] * lamp_vis;
                            let glint = if sh.flags & F_WET != 0 && topness > 0.3 {
                                let l = (lamp_pos - p).normalize_or_zero();
                                let h = (l + view).normalize_or_zero();
                                let sparkle = noise3(p.x as i32, (s.time * 4.0) as i32, p.z as i32);
                                let n =
                                    h.y.max(0.0).powi(24) * if sparkle > 0.82 { 2.0 } else { 0.25 };
                                spec * n * 1.4 * topness
                            } else {
                                0.0
                            };
                            for (k, v) in col.iter_mut().enumerate() {
                                *v += rim
                                    * (0.035 + lamp[k] * lamp_vis * 0.35 + other[k] * 0.2)
                                    * wet
                                    + glint * [1.0, 0.85, 0.6][k];
                            }
                        }
                        // silhouette edges darken (pixel-art outline)
                        let mut edge = false;
                        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            if (nx as u32) < SW as u32 && (ny as u32) < SH as u32 {
                                let nd = self.depth[ny as usize * SW + nx as usize];
                                if nd - d > 6.0 {
                                    edge = true;
                                }
                            }
                        }
                        if edge && sh.flags & F_CAT != 0 {
                            col = std::array::from_fn(|k| {
                                col[k] * 1.15 + [0.05, 0.045, 0.06][k] + lamp[k] * lamp_vis * 0.25
                            });
                        } else if edge && sh.flags & F_DYNAMIC != 0 {
                            col = col.map(|v| v * 0.7);
                        } else if edge {
                            col = col.map(|v| v * 0.85);
                        }
                        col = mix(col, fog_col, fog * 0.6);
                    }
                } else {
                    col = self.background(&bg, x, y, flash);
                }

                // glass panes in front: tint + faint reflection of their light
                let gd = self.glass_d[i];
                if gd < d && gd < self.wf[i] {
                    let g = self.glass[i];
                    let glow: Rgb = std::array::from_fn(|k| {
                        f32::from(g.amb[k]) / 255.0 * amb_scale
                            + t[g.lamp[k] as usize] * lamp_scale
                            + t[g.other[k] as usize] * other_scale
                    });
                    col = std::array::from_fn(|k| {
                        col[k] * [0.66, 0.78, 0.84][k] + glow[k] * 0.35 + 0.012
                    });
                }

                // water volume in front of the surface
                let wf = self.wf[i];
                if wf < d {
                    let back = if d.is_finite() { d } else { self.wb[i] };
                    let thick = (back - wf).max(0.0) + 1.0;
                    let absorb = 1.0 - (-thick * 0.05).exp();
                    let p = b.unproject(c, x, y, wf);
                    let (lamp, other) = s.lighting.grid_at(p);
                    let lit: Rgb =
                        std::array::from_fn(|k| lamp[k] * lamp_scale + other[k] * other_scale);
                    let deep: Rgb = std::array::from_fn(|k| {
                        [0.03, 0.065, 0.10][k] * amb_scale + lit[k] * [0.12, 0.2, 0.24][k]
                    });
                    col = std::array::from_fn(|k| {
                        col[k] * (1.0 - absorb) * [0.55, 0.8, 0.85][k] + deep[k] * absorb
                    });
                    let cell = self.wcell[i];
                    if cell > 0 {
                        let (wx, wz) = ((cell - 1) as i32 % W, (cell - 1) as i32 / W);
                        let (gx, gz) = s.water.slope(wx, wz);
                        let n = vec3(-gx * 2.0, 1.0, -gz * 2.0).normalize();
                        let l = (lamp_pos - p).normalize_or_zero();
                        let h = (l + view).normalize_or_zero();
                        let nh = n.dot(h).max(0.0);
                        let nh8 = {
                            let a = nh * nh;
                            let b = a * a;
                            b * b
                        };
                        let spec =
                            nh8 * nh8 * nh8 * nh8 * nh8 * (lamp[0] + lamp[1]) * lamp_scale * 0.8;
                        let fres = (1.0 - n.dot(view).abs()).powi(3) * 0.15 + 0.03;
                        let ripple = (gx.abs() + gz.abs()).min(0.6);
                        for (k, v) in col.iter_mut().enumerate() {
                            *v += spec * [1.0, 0.8, 0.55][k]
                                + fres * [0.25, 0.32, 0.5][k] * amb_scale
                                + ripple * lit[k] * 0.25;
                            if topness < 0.5 {
                                // the waterline catches whatever light is around
                                *v += lit[k] * 0.22
                                    + [0.05, 0.075, 0.1][k] * amb_scale
                                    + ripple * 0.15;
                            }
                        }
                    }
                }

                // light cone of the street lamp in the falling rain
                if beam_k > 0.0
                    && (x as f32 - lsx).abs() < 80.0
                    && (y as f32 - lsy) > -50.0
                    && (y as f32 - lsy) < beam_len + 10.0
                    && (d > ld - 20.0 || !d.is_finite())
                {
                    let (vx, vy) = (x as f32 + 0.5 - lsx, y as f32 + 0.5 - lsy);
                    let dist = (vx * vx + vy * vy).sqrt();
                    let along = if axis_len > 0.05 {
                        (vx * axis2.0 + vy * axis2.1) / axis_len
                    } else {
                        0.0
                    };
                    let perp = (dist * dist - along * along).max(0.0).sqrt();
                    let side = if along > 0.0 && along < beam_len {
                        let width = along * 0.62 * axis_len + (1.0 - axis_len) * 40.0 + 2.0;
                        (1.0 - smoothstep(width * 0.6, width, perp))
                            * (1.0 - along / beam_len).sqrt()
                    } else {
                        0.0
                    };
                    let disc = (1.0 - axis_len) * (1.0 - smoothstep(4.0, 34.0, dist)) * 0.45;
                    let k = (side * axis_len + disc) * beam_k;
                    col[0] += k * 1.0;
                    col[1] += k * 0.72;
                    col[2] += k * 0.42;
                }

                // x-ray outline of the cat when something hides it
                let cd = self.cat_d[i];
                if cd.is_finite() && cd > d + 1.5 {
                    col = mix(col, [0.95, 0.62, 0.3], 0.28);
                }
                self.hdr[i] = col;
            }
        }
    }

    /// Per-frame setup for `background`: where each pixel's ray meets the
    /// far end of the world is affine in screen space, and the skyline only
    /// depends on the column.
    fn background_setup(&mut self, s: &Scene) -> Option<Background> {
        let b = self.basis;
        let c = self.cam;
        let along_f = b.f.dot(b.fh);
        if along_f < 0.05 {
            return None;
        }
        let far = [
            vec3(0.0, 0.0, 0.0),
            vec3(W as f32, 0.0, D as f32),
            vec3(W as f32, 0.0, 0.0),
            vec3(0.0, 0.0, D as f32),
        ]
        .iter()
        .map(|p| p.dot(b.fh))
        .fold(f32::NEG_INFINITY, f32::max);
        let hit = |x: usize, y: usize| {
            let base = b.unproject(c, x, y, 0.0);
            base + b.f * ((far - base.dot(b.fh)) / along_f)
        };
        let (h00, h10, h01) = (hit(0, 0), hit(1, 0), hit(0, 1));
        let shift = c.dot(b.r);
        for (layer, (par, base_h)) in [(0.15f32, 28.0f32), (0.3, 12.0)].into_iter().enumerate() {
            for (x, slot) in self.skyline[layer].iter_mut().enumerate() {
                let u = (x as f32 + shift * par + layer as f32 * 977.0) as i32;
                let block = u.div_euclid(9 + layer as i32 * 4);
                *slot = (
                    u,
                    GROUND as f32 + base_h + (hash3(block, layer as i32, 5) % 40) as f32,
                );
            }
        }
        // the cloud row and column advance linearly too
        let along0 = h00.dot(b.r) * 0.9 + s.time * 2.0;
        Some(Background {
            y0: h00.y,
            ydx: h10.y - h00.y,
            ydy: h01.y - h00.y,
            a0: along0,
            adx: (h10 - h00).dot(b.r) * 0.9,
            ady: (h01 - h00).dot(b.r) * 0.9,
        })
    }

    fn background(&self, bg: &Option<Background>, x: usize, y: usize, flash: f32) -> Rgb {
        let void = [0.012, 0.015, 0.025];
        let Some(bg) = bg else { return void };
        let (fx, fy) = (x as f32, y as f32);
        let world_y = bg.y0 + bg.ydx * fx + bg.ydy * fy;
        if world_y < 0.0 {
            return mix([0.03, 0.035, 0.055], void, smoothstep(0.0, 30.0, -world_y));
        }
        let mut sky = mix(
            [0.07, 0.085, 0.14],
            [0.02, 0.025, 0.05],
            smoothstep(10.0, 200.0, world_y),
        );
        let cloud = {
            let u =
                ((bg.a0 + bg.adx * fx + bg.ady * fy) as i32).rem_euclid(CLOUD_W as i32) as usize;
            let v = (world_y as usize).min(CLOUD_H - 1);
            self.clouds[v * CLOUD_W + u]
        };
        sky = mix(sky, [0.12, 0.13, 0.19], cloud * 0.5);
        sky = mix(sky, [0.55, 0.55, 0.75], flash * (0.35 + cloud * 0.5));
        // two layers of distant skyline with parallax
        for (layer, col) in [[0.035f32, 0.045, 0.075], [0.025, 0.032, 0.055]]
            .into_iter()
            .enumerate()
        {
            let (u, height) = self.skyline[layer][x];
            if world_y < height {
                sky = col;
                let wy = world_y as i32;
                if u.rem_euclid(3) != 0
                    && wy.rem_euclid(4) != 0
                    && hash3(u.div_euclid(3), wy.div_euclid(4), 11 + layer as i32)
                        .is_multiple_of(17)
                {
                    sky = [0.35, 0.28, 0.12];
                }
                sky = mix(sky, [0.25, 0.25, 0.35], flash * 0.4);
            }
        }
        if self.bolt[y * SW + x] {
            sky = [2.2, 2.2, 2.6];
        }
        sky
    }

    fn draw_particles(&mut self, s: &Scene) {
        let b = self.basis;
        let c = self.cam;
        let w = s.weather;
        let streak = 0.016 * b.stretch;
        for d in &w.drops[..w.active] {
            let (x0, y0, dep) = b.project(d.p - c);
            if !(-4.0..SW as f32 + 4.0).contains(&x0) || !(-4.0..SH as f32 + 4.0).contains(&y0) {
                continue;
            }
            let (x1, y1, _) = b.project(d.p - d.v * streak - c);
            let (lamp, other) = s.lighting.grid_at(d.p);
            let col: Rgb = std::array::from_fn(|k| {
                [0.05, 0.06, 0.085][k] * (1.0 + w.flash * 4.0)
                    + (lamp[k] * s.lamp + other[k] * s.panel) * 0.32
            });
            self.line(x0, y0, x1, y1, dep, col);
        }
        for p in &w.particles {
            let (x, y, dep) = b.project(p.p - c);
            let (lamp, other) = s.lighting.grid_at(p.p);
            let lit: Rgb = std::array::from_fn(|k| lamp[k] * s.lamp + other[k] * s.panel);
            let col = match p.kind {
                Kind::Spray => std::array::from_fn(|k| 0.09 + lit[k] * 0.4),
                Kind::Drip | Kind::Stream => {
                    std::array::from_fn(|k| [0.08, 0.1, 0.13][k] + lit[k] * 0.5)
                }
                Kind::Sparkle => [1.4 * p.life + 0.3, 1.2 * p.life + 0.2, 0.5],
            };
            if p.kind == Kind::Drip {
                self.line(x, y, x, y - 2.0, dep, col);
            } else {
                self.dot(x, y, dep, col);
            }
        }
    }

    #[inline]
    fn dot(&mut self, x: f32, y: f32, dep: f32, col: Rgb) {
        let (xi, yi) = (x.floor() as i32, y.floor() as i32);
        if (xi as u32) < SW as u32 && (yi as u32) < SH as u32 {
            let i = yi as usize * SW + xi as usize;
            if dep < self.depth[i] && dep < self.wf[i] + 0.5 {
                let h = &mut self.hdr[i];
                h[0] += col[0];
                h[1] += col[1];
                h[2] += col[2];
            }
        }
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, dep: f32, col: Rgb) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1.0, 8.0) as i32;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let fade = 1.0 - t * 0.6;
            self.dot(lerp(x0, x1, t), lerp(y0, y1, t), dep, col.map(|v| v * fade));
        }
    }

    /// Glow around bright pixels: bright-pass at half resolution, blurred
    /// with a few separable passes, added back.
    fn bloom(&mut self) {
        // bright-pass into a quarter resolution buffer (4x4 blocks)
        let (w4, h4) = (SW / 4, SH / 4);
        for y in 0..h4 {
            for x in 0..w4 {
                let mut acc = [0.0; 3];
                for dy in 0..4 {
                    for dx in 0..4 {
                        let p = self.hdr[(y * 4 + dy) * SW + x * 4 + dx];
                        let l = p[0] * 0.3 + p[1] * 0.5 + p[2] * 0.2;
                        if l > 0.75 {
                            let k = (l - 0.75) / l * (0.25 / 4.0);
                            for c in 0..3 {
                                acc[c] += p[c] * k;
                            }
                        }
                    }
                }
                self.half[y * w4 + x] = acc;
            }
        }
        blur(&self.half, &mut self.half_tmp, w4, h4, 1, 0);
        blur(&self.half_tmp, &mut self.half, w4, h4, 0, 1);
        // bilinear upsample and add
        for y in 0..SH {
            let fy = ((y as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (h4 - 1) as f32);
            let y0 = fy as usize;
            let ty = fy - y0 as f32;
            let y1 = (y0 + 1).min(h4 - 1);
            for x in 0..SW {
                let fx = ((x as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (w4 - 1) as f32);
                let x0 = fx as usize;
                let tx = fx - x0 as f32;
                let x1 = (x0 + 1).min(w4 - 1);
                let (a, b) = (self.half[y0 * w4 + x0], self.half[y0 * w4 + x1]);
                let (c, d) = (self.half[y1 * w4 + x0], self.half[y1 * w4 + x1]);
                let p = &mut self.hdr[y * SW + x];
                for k in 0..3 {
                    let top = a[k] + (b[k] - a[k]) * tx;
                    let bottom = c[k] + (d[k] - c[k]) * tx;
                    p[k] += (top + (bottom - top) * ty) * 1.7;
                }
            }
        }
    }

    fn finish(&mut self) {
        let tone = &self.tone;
        let mut out = self.rgba.as_chunks_mut::<4>().0.iter_mut();
        for y in 0..SH {
            let bayer = &BAYER[(y & 3) * 4..(y & 3) * 4 + 4];
            for x in 0..SW {
                let px = out.next().unwrap();
                let dither = bayer[x & 3];
                let p = self.hdr[y * SW + x];
                for c in 0..3 {
                    let v = (p[c] * TONE_SCALE).clamp(0.0, (TONE_LEN - 1) as f32) as usize;
                    // tone[] holds the curve times LEVELS; dither picks the level
                    px[c] =
                        (((tone[v] + dither) as u32) as f32 * (255.0 / LEVELS)).min(255.0) as u8;
                }
                px[3] = 255;
            }
        }
    }
}

fn blur(src: &[Rgb], dst: &mut [Rgb], w: usize, h: usize, dx: usize, dy: usize) {
    for y in 0..h {
        for x in 0..w {
            let at = |o: isize| {
                let xx = (x as isize + o * dx as isize).clamp(0, w as isize - 1) as usize;
                let yy = (y as isize + o * dy as isize).clamp(0, h as isize - 1) as usize;
                src[yy * w + xx]
            };
            let (a, b, c, d, e) = (at(-2), at(-1), at(0), at(1), at(2));
            dst[y * w + x] = std::array::from_fn(|k| {
                (a[k] + e[k]) * 0.0625 + (b[k] + d[k]) * 0.25 + c[k] * 0.375
            });
        }
    }
}

/// Does the segment from `p` to `q` pass through the box (excluding the
/// start point being inside)?
fn segment_hits_box(p: Vec3, q: Vec3, lo: Vec3, hi: Vec3) -> bool {
    // quick reject against the box's bounding sphere
    let c = (lo + hi) * 0.5;
    let r2 = (hi - lo).length_squared() * 0.25;
    let d = q - p;
    let t = ((c - p).dot(d) / d.length_squared()).clamp(0.0, 1.0);
    if (p + d * t - c).length_squared() > r2 {
        return false;
    }
    if p.cmpge(lo).all() && p.cmple(hi).all() {
        return false;
    }
    let mut t0: f32 = 0.0;
    let mut t1: f32 = 1.0;
    for a in 0..3 {
        if d[a].abs() < 1e-6 {
            if p[a] < lo[a] || p[a] > hi[a] {
                return false;
            }
        } else {
            let inv = 1.0 / d[a];
            let (mut ta, mut tb) = ((lo[a] - p[a]) * inv, (hi[a] - p[a]) * inv);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                return false;
            }
        }
    }
    t0 > 0.02
}

/// Value noise that wraps every `period` units along x.
fn tiled_noise(x: f32, y: f32, period: f32) -> f32 {
    let p = period.round() as i32;
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let n = |a: i32, b: i32| noise3(a.rem_euclid(p), b, 17);
    lerp(
        lerp(n(xi, yi), n(xi + 1, yi), sx),
        lerp(n(xi, yi + 1), n(xi + 1, yi + 1), sx),
        sy,
    )
}
