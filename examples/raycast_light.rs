//! A point light, three draggable objects (cube, sphere, cone) and a floor -
//! shaded with real per-pixel ray casting, done on the CPU.
//!
//! Every frame the scene is ray traced at 200x150 and the image is uploaded
//! into a texture drawn fullscreen with nearest filtering, so the pixels stay
//! crisp 4x4 blocks. Each pixel casts a primary ray at the scene and a shadow
//! ray at the light: the floor shows the shadows cast by the objects, the
//! objects shadow each other, and everything brightens or darkens as the
//! light is moved.
//!
//! Controls: **drag the light** (the bright dot) or **drag any object**
//! across the floor and watch the shadow rays re-cast.

use glam::{Mat4, Vec3};
use tinyquad::*;

/// Internal render resolution; upscaled 4x to the 800x600 window.
const RW: usize = 200;
const RH: usize = 150;
const HALF_EXTENT: f32 = 3.9;
const CAMERA_DIST: f32 = 12.0;
const CAMERA_YAW: f32 = -55.0_f32.to_radians();
const CAMERA_PITCH: f32 = 42.0_f32.to_radians();
const LIGHT_Z: f32 = 2.6;
const LIGHT_INTENSITY: f32 = 4.2;
const AMBIENT: f32 = 0.16;

/// Ray-primitive hits return the distance along the ray plus the surface
/// normal at the hit.
type Hit = (f32, Vec3);

pub struct Stage {
    ctx: Box<dyn RenderingBackend>,
    blit_pipeline: Pipeline,
    blit_bind: Bindings,
    line_pipeline: Pipeline,
    rays_bind: Bindings,
    texture: TextureId,
    light: Vec3,
    sphere_c: Vec3,
    cube_c: Vec3,
    cone_c: Vec3,
    drag: Option<(Drag, f32, f32)>,
}

#[derive(Clone, Copy)]
enum Drag {
    Light,
    Sphere,
    Cube,
    Cone,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 4],
}

impl Vertex {
    fn new(pos: [f32; 3], color: [f32; 4]) -> Vertex {
        Vertex { pos, color }
    }
}

impl Default for Stage {
    fn default() -> Self {
        Self::new()
    }
}

impl Stage {
    pub fn new() -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let texture = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Empty,
            TextureParams {
                width: RW as u32,
                height: RH as u32,
                format: TextureFormat::RGBA8,
                min_filter: FilterMode::Nearest,
                mag_filter: FilterMode::Nearest,
                ..Default::default()
            },
        );

        // fullscreen blit of the ray-traced image
        #[rustfmt::skip]
        let blit_verts: &[f32] = &[
            /* pos */ -1.0, -1.0, 0.0, /* uv */ 0.0, 0.0,
            /* pos */  1.0, -1.0, 0.0, /* uv */ 1.0, 0.0,
            /* pos */  1.0,  1.0, 0.0, /* uv */ 1.0, 1.0,
            /* pos */ -1.0,  1.0, 0.0, /* uv */ 0.0, 1.0,
        ];
        let blit_bind = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(blit_verts),
            )],
            index_buffer: ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&[0u16, 1, 2, 0, 2, 3]),
            ),
            images: vec![texture],
        };
        let blit_shader_id = ctx
            .new_shader(
                match ctx.info().backend {
                    Backend::OpenGl => ShaderSource::Glsl {
                        vertex: blit_shader::BLIT_VERTEX,
                        fragment: blit_shader::BLIT_FRAGMENT,
                    },
                    Backend::Metal => ShaderSource::Msl {
                        program: blit_shader::METAL,
                    },
                },
                blit_shader::meta(),
            )
            .unwrap();
        let blit_pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float3),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
            ],
            blit_shader_id,
            PipelineParams::default(),
        );

        // ray lines drawn on top with the same camera as the CPU renderer
        let line_shader_id = ctx
            .new_shader(
                match ctx.info().backend {
                    Backend::OpenGl => ShaderSource::Glsl {
                        vertex: shader::VERTEX,
                        fragment: shader::FRAGMENT,
                    },
                    Backend::Metal => ShaderSource::Msl {
                        program: shader::METAL,
                    },
                },
                shader::meta(),
            )
            .unwrap();
        let line_pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float3),
                VertexAttribute::new("in_color", VertexFormat::Float4),
            ],
            line_shader_id,
            PipelineParams {
                primitive_type: PrimitiveType::Lines,
                ..Default::default()
            },
        );
        // exactly 3 rays x 2 endpoints; pre-sized because buffer_update
        // never grows a buffer
        let rays = vec![Vertex::new([0.; 3], [0.; 4]); 6];
        let rays_bind = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Dynamic,
                BufferSource::slice(&rays),
            )],
            index_buffer: ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&[0u16, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1]),
            ),
            images: vec![],
        };

        Stage {
            ctx,
            blit_pipeline,
            blit_bind,
            line_pipeline,
            rays_bind,
            texture,
            light: Vec3::new(-0.6, -1.2, LIGHT_Z),
            sphere_c: Vec3::new(0.5, 0.7, 0.55),
            cube_c: Vec3::new(-1.3, 0.3, 0.5),
            cone_c: Vec3::new(1.5, -0.7, 0.0),
            drag: None,
        }
    }

    fn camera(&self) -> (Vec3, Vec3, Vec3, Vec3) {
        let eye = Vec3::new(
            CAMERA_DIST * CAMERA_PITCH.cos() * CAMERA_YAW.cos(),
            CAMERA_DIST * CAMERA_PITCH.cos() * CAMERA_YAW.sin(),
            CAMERA_DIST * CAMERA_PITCH.sin(),
        );
        let forward = (Vec3::ZERO - eye).normalize();
        let right = forward.cross(Vec3::Z).normalize();
        let up = right.cross(forward);
        (eye, right, up, forward)
    }

    /// World point on the horizontal plane at `plane_z` under the mouse.
    fn floor_point(&self, mx: f32, my: f32, plane_z: f32) -> Vec3 {
        let (w, h) = window::screen_size();
        let (eye, right, up, forward) = self.camera();
        let ndx = (mx / w * 2.0 - 1.0) * HALF_EXTENT * (w / h);
        let ndy = (1.0 - my / h * 2.0) * HALF_EXTENT;
        let o = eye + right * ndx + up * ndy;
        let t = (plane_z - o.z) / forward.z;
        o + forward * t
    }

    fn render(&self) -> Vec<u8> {
        let (w, h) = (RW, RH);
        let mut pixels = vec![0u8; w * h * 4];
        let (eye, right, up, forward) = self.camera();
        let aspect = w as f32 / h as f32;

        for py in 0..h {
            for px in 0..w {
                let ndx = ((px as f32 + 0.5) / w as f32 * 2.0 - 1.0) * HALF_EXTENT * aspect;
                let ndy = (1.0 - (py as f32 + 0.5) / h as f32 * 2.0) * HALF_EXTENT;
                let origin = eye + right * ndx + up * ndy;
                let dir = forward;

                let (color, _t) = self.trace(origin, dir);
                let i = (py * w + px) * 4;
                pixels[i] = (color.x.min(1.0) * 255.0) as u8;
                pixels[i + 1] = (color.y.min(1.0) * 255.0) as u8;
                pixels[i + 2] = (color.z.min(1.0) * 255.0) as u8;
                pixels[i + 3] = 255;
            }
        }

        // stamp the light source so it can be found and dragged
        let rel = self.light - eye;
        let sx = (rel.dot(right) / (HALF_EXTENT * aspect) + 1.0) / 2.0;
        let sy = 1.0 - (rel.dot(up) / HALF_EXTENT + 1.0) / 2.0;
        let cx = (sx * w as f32) as i32;
        let cy = (sy * h as f32) as i32;
        for dy in -4..=4 {
            for dx in -4..=4 {
                let d2 = (dx * dx + dy * dy) as f32;
                if d2 > 16.0 {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                    continue;
                }
                let i = (y as usize * w + x as usize) * 4;
                let core = d2 <= 4.0;
                pixels[i] = if core { 255 } else { 246 };
                pixels[i + 1] = if core { 250 } else { 238 };
                pixels[i + 2] = if core { 210 } else { 150 };
                pixels[i + 3] = 255;
            }
        }

        pixels
    }

    /// Primary ray: nearest hit among floor, sphere, cube and cone, shaded
    /// with one shadow ray toward the light.
    fn trace(&self, origin: Vec3, dir: Vec3) -> (Vec3, f32) {
        // floor (z = 0) with a checker pattern
        let mut best_t = f32::INFINITY;
        let mut best_n = Vec3::Z;
        let mut best_color = [0.3, 0.3, 0.3, 1.0];
        if dir.z.abs() > 1e-6 {
            let t = -origin.z / dir.z;
            if t > 1e-4 {
                let p = origin + dir * t;
                let checker = ((p.x / 0.75).floor() + (p.y / 0.75).floor()) as i32 % 2;
                let g = if checker == 0 { 0.34 } else { 0.27 };
                best_t = t;
                best_n = Vec3::Z;
                best_color = [g, g, g + 0.015, 1.0];
            }
        }

        if let Some((t, n)) = hit_sphere(origin, dir, self.sphere_c, 0.55) {
            if t < best_t {
                best_t = t;
                best_n = n;
                best_color = [0.30, 0.72, 0.38, 1.0];
            }
        }
        if let Some((t, n)) = hit_cube(origin, dir, self.cube_c, Vec3::new(0.5, 0.5, 0.5)) {
            if t < best_t {
                best_t = t;
                best_n = n;
                best_color = [0.45, 0.58, 0.78, 1.0];
            }
        }
        if let Some((t, n)) = hit_cone(origin, dir, self.cone_c, 0.6, 1.4) {
            if t < best_t {
                best_t = t;
                best_n = n;
                best_color = [0.88, 0.48, 0.22, 1.0];
            }
        }

        if best_t.is_infinite() {
            return (Vec3::ZERO, best_t);
        }

        let p = origin + dir * best_t;
        let to_light = self.light - p;
        let dist = to_light.length();
        let ldir = to_light / dist;
        let lambert = best_n.dot(ldir).max(0.0);
        let shadowed = self.occluded(p + best_n * 1e-3, ldir, dist);

        let strength = if shadowed {
            0.0
        } else {
            LIGHT_INTENSITY * lambert / (1.0 + 0.10 * dist * dist)
        };

        let c = Vec3::new(best_color[0], best_color[1], best_color[2]) * (AMBIENT + strength)
            + Vec3::new(1.0, 0.95, 0.8) * (strength * 0.08);
        (c, best_t)
    }

    /// Shadow ray: does anything block the straight line to the light?
    fn occluded(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> bool {
        if let Some((t, _)) = hit_sphere(origin, dir, self.sphere_c, 0.55) {
            if t < max_dist {
                return true;
            }
        }
        if let Some((t, _)) = hit_cube(origin, dir, self.cube_c, Vec3::new(0.5, 0.5, 0.5)) {
            if t < max_dist {
                return true;
            }
        }
        if let Some((t, _)) = hit_cone(origin, dir, self.cone_c, 0.6, 1.4) {
            if t < max_dist {
                return true;
            }
        }
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn view_from(eye: Vec3, right: Vec3, up: Vec3, forward: Vec3) -> Mat4 {
    Mat4::from_cols_array_2d(&[
        [right.x, up.x, -forward.x, 0.0],
        [right.y, up.y, -forward.y, 0.0],
        [right.z, up.z, -forward.z, 0.0],
        [-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.0],
    ])
}

fn hit_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<Hit> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if t < 1e-4 {
        return None;
    }
    let p = origin + dir * t;
    Some((t, (p - center) / radius))
}

fn hit_cube(origin: Vec3, dir: Vec3, center: Vec3, half: Vec3) -> Option<Hit> {
    let o = [origin.x, origin.y, origin.z];
    let d = [dir.x, dir.y, dir.z];
    let c = [center.x, center.y, center.z];
    let h = [half.x, half.y, half.z];
    let mut tmin = -f32::INFINITY;
    let mut tmax = f32::INFINITY;
    let mut axis = 0;
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < c[i] - h[i] || o[i] > c[i] + h[i] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[i];
        let (mut t0, mut t1) = ((c[i] - h[i] - o[i]) * inv, (c[i] + h[i] - o[i]) * inv);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        if t0 > tmin {
            tmin = t0;
            axis = i;
        }
        tmax = tmax.min(t1);
        if tmax < tmin.max(1e-4) {
            return None;
        }
    }
    if tmin < 1e-4 {
        return None;
    }
    let mut n = [0.0f32; 3];
    n[axis] = if d[axis] > 0.0 { -1.0 } else { 1.0 };
    Some((tmin, Vec3::from_array(n)))
}

/// Vertical cone: base disk of radius `radius` at `base` (z = 0 relative),
/// apex at `base + (0, 0, height)`.
fn hit_cone(origin: Vec3, dir: Vec3, base: Vec3, radius: f32, height: f32) -> Option<Hit> {
    let ox = origin.x - base.x;
    let oy = origin.y - base.y;
    let oz = origin.z - base.z;
    let k = radius / height;
    let a = dir.x * dir.x + dir.y * dir.y - k * k * dir.z * dir.z;
    let b = 2.0 * (ox * dir.x + oy * dir.y)
        + 2.0 * radius * radius * (1.0 - oz / height) * (dir.z / height);
    let c = ox * ox + oy * oy - radius * radius * (oz / height - 1.0) * (oz / height - 1.0);

    let mut best = f32::INFINITY;
    if a.abs() > 1e-9 {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let sq = disc.sqrt();
            for t in [(-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a)] {
                if t < 1e-4 || t > best {
                    continue;
                }
                let z = oz + t * dir.z;
                if z >= 0.0 && z <= height {
                    best = t;
                }
            }
        }
    } else if b.abs() > 1e-9 {
        let t = -c / b;
        if t > 1e-4 && t < best {
            let z = oz + t * dir.z;
            if z >= 0.0 && z <= height {
                best = t;
            }
        }
    }

    // base disk
    if dir.z.abs() > 1e-9 {
        let t = -oz / dir.z;
        if t > 1e-4 && t < best {
            let px = ox + t * dir.x;
            let py = oy + t * dir.y;
            if px * px + py * py <= radius * radius {
                best = t;
            }
        }
    }

    if best.is_infinite() {
        return None;
    }
    let p = origin + dir * best;
    // lateral surface normal around the axis; the base disk faces down
    if (p.z - base.z).abs() < 1e-5 && (p.x - base.x).hypot(p.y - base.y) <= radius {
        return Some((best, Vec3::new(0.0, 0.0, -1.0)));
    }
    let rho = radius * (1.0 - (p.z - base.z) / height);
    let n = Vec3::new(p.x - base.x, p.y - base.y, rho * k).normalize();
    Some((best, n))
}

impl EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        let (width, height) = window::screen_size();
        let pixels = self.render();
        self.ctx.texture_update(self.texture, &pixels);

        let (eye, right, up, forward) = self.camera();
        let aspect = width / height;
        let view = view_from(eye, right, up, forward);
        let proj = glam::camera::rh::proj::opengl::orthographic(
            -HALF_EXTENT * aspect,
            HALF_EXTENT * aspect,
            -HALF_EXTENT,
            HALF_EXTENT,
            0.1,
            100.0,
        );
        let mvp = proj * view;

        self.ctx.begin_default_pass(PassAction::Clear {
            color: Some((0.10, 0.10, 0.12, 1.0)),
            depth: Some(1.),
            stencil: None,
        });

        // the ray-traced image
        self.ctx.apply_pipeline(&self.blit_pipeline);
        self.ctx.apply_bindings(&self.blit_bind);
        self.ctx.draw(0, 6, 1);

        // explicit rays from the light to each object
        let rays = {
            let targets = [
                self.sphere_c,
                self.cube_c,
                Vec3::new(self.cone_c.x, self.cone_c.y, 0.7),
            ];
            let mut verts = Vec::new();
            for target in targets {
                for p in [self.light, target] {
                    let rel = p - eye;
                    let x = rel.dot(right) / (HALF_EXTENT * aspect);
                    let y = rel.dot(up) / HALF_EXTENT;
                    verts.push(Vertex::new([x, y, 0.0], [0.85, 0.80, 0.45, 1.0]));
                }
            }
            verts
        };
        if !rays.is_empty() {
            self.ctx
                .buffer_update(self.rays_bind.vertex_buffers[0], BufferSource::slice(&rays));
            self.ctx.apply_pipeline(&self.line_pipeline);
            self.ctx.apply_bindings(&self.rays_bind);
            self.ctx
                .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp }));
            self.ctx.draw(0, rays.len() as _, 1);
        }

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button != MouseButton::Left {
            return;
        }
        // try the light first (it is drawn on top), then the object footprints
        let p_light = self.floor_point(x, y, self.light.z);
        let d_light = (p_light - self.light).length();
        if d_light < 0.5 {
            self.drag = Some((
                Drag::Light,
                self.light.x - p_light.x,
                self.light.y - p_light.y,
            ));
            return;
        }
        let on_floor = self.floor_point(x, y, 0.0);
        let candidates: [(Drag, Vec3, f32); 3] = [
            (Drag::Sphere, self.sphere_c, 0.55 + 0.15),
            (Drag::Cube, self.cube_c, 0.5 + 0.15),
            (Drag::Cone, self.cone_c, 0.6 + 0.15),
        ];
        for (which, c, reach) in candidates {
            let dx = on_floor.x - c.x;
            let dy = on_floor.y - c.y;
            if dx * dx + dy * dy <= reach * reach {
                self.drag = Some((which, c.x - on_floor.x, c.y - on_floor.y));
                return;
            }
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        if button == MouseButton::Left {
            self.drag = None;
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let Some((what, ox, oy)) = self.drag else {
            return;
        };
        let plane_z = match what {
            Drag::Light => self.light.z,
            _ => 0.0,
        };
        let p = self.floor_point(x, y, plane_z);
        let nx = (p.x + ox).clamp(-3.2, 3.2);
        let ny = (p.y + oy).clamp(-3.2, 3.2);
        match what {
            Drag::Light => self.light = Vec3::new(nx, ny, self.light.z),
            Drag::Sphere => self.sphere_c = Vec3::new(nx, ny, self.sphere_c.z),
            Drag::Cube => self.cube_c = Vec3::new(nx, ny, self.cube_c.z),
            Drag::Cone => self.cone_c = Vec3::new(nx, ny, self.cone_c.z),
        }
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "raycast light".to_owned(),
        window_resizable: false,
        window_width: 800,
        window_height: 600,
        ..Default::default()
    };

    tinyquad::start(conf, move || Box::new(Stage::new()));
}

mod shader {
    use tinyquad::*;

    #[repr(C)]
    pub struct Uniforms {
        pub mvp: glam::Mat4,
    }

    pub const VERTEX: &str = r#"#version 100
    attribute vec3 in_pos;
    attribute vec4 in_color;

    uniform mat4 mvp;

    varying lowp vec4 color;

    void main() {
        gl_Position = mvp * vec4(in_pos, 1.0);
        color = in_color;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec4 color;

    void main() {
        gl_FragColor = color;
    }"#;

    pub const METAL: &str = r#"
    #include <metal_stdlib>

    using namespace metal;

    struct Vertex
    {
        float3 in_pos [[attribute(0)]];
        float4 in_color [[attribute(1)]];
    };

    struct RasterizerData
    {
        float4 position [[position]];
        float4 color [[user(locn0)]];
    };

    vertex RasterizerData vertexShader(Vertex v [[stage_in]],
                                      constant float4x4& mvp [[buffer(0)]])
    {
        RasterizerData out;

        out.position = mvp * float4(v.in_pos, 1.0);
        out.color = v.in_color;

        return out;
    }

    fragment float4 fragmentShader(RasterizerData in [[stage_in]])
    {
        return in.color;
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec![],
            uniforms: UniformBlockLayout {
                uniforms: vec![UniformDesc::new("mvp", UniformType::Mat4)],
            },
        }
    }
}

mod blit_shader {
    use tinyquad::*;

    pub const BLIT_VERTEX: &str = r#"#version 100
    attribute vec3 in_pos;
    attribute vec2 in_uv;

    varying lowp vec2 uv;

    void main() {
        gl_Position = vec4(in_pos, 1.0);
        uv = in_uv;
    }"#;

    pub const BLIT_FRAGMENT: &str = r#"#version 100
    varying lowp vec2 uv;

    uniform sampler2D tex;

    void main() {
        gl_FragColor = texture2D(tex, uv);
    }"#;

    pub const METAL: &str = r#"
    #include <metal_stdlib>

    using namespace metal;

    struct Vertex
    {
        float3 in_pos [[attribute(0)]];
        float2 in_uv  [[attribute(1)]];
    };

    struct RasterizerData
    {
        float4 position [[position]];
        float2 uv [[user(locn0)]];
    };

    vertex RasterizerData vertexShader(Vertex v [[stage_in]])
    {
        RasterizerData out;

        out.position = float4(v.in_pos, 1.0);
        out.uv = v.in_uv;

        return out;
    }

    fragment float4 fragmentShader(RasterizerData in [[stage_in]],
                                   texture2d<float> tex [[texture(0)]],
                                   sampler texSmplr [[sampler(0)]])
    {
        return tex.sample(texSmplr, in.uv);
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout { uniforms: vec![] },
        }
    }
}
