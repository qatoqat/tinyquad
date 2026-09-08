//! A 3D orthographic scene that plays like a 2D game.
//!
//! The world is fully three-dimensional (a ground grid, a few pillars of
//! different heights and the orange player cube), but the camera starts
//! straight above, so everything reads as flat 2D. Press **Space** to
//! toggle between the top-down and the side view - the transition is
//! animated and reveals the third dimension.
//!
//! Controls:
//! - **WASD / arrow keys**: move the orange cube along the world X/Y axes
//! - **Space**: toggle top-down / side view

use glam::{Mat4, Vec3};
use miniquad::*;

const GRID_EXTENT: f32 = 3.0;
const GRID_COLOR: [f32; 4] = [0.30, 0.30, 0.30, 1.0];
const AXIS_X: [f32; 4] = [0.80, 0.30, 0.30, 1.0];
const AXIS_Y: [f32; 4] = [0.30, 0.80, 0.30, 1.0];
const AXIS_Z: [f32; 4] = [0.30, 0.50, 0.90, 1.0];
const BG_COLOR: (f32, f32, f32, f32) = (0.22, 0.22, 0.24, 1.0);
const ENTITY_COLOR: [f32; 4] = [1.00, 0.65, 0.20, 1.0];

const MOVE_SPEED: f32 = 0.045; // world units per frame at ~60 fps
const MOVE_LIMIT: f32 = 2.5;
const ENTITY_HALF: f32 = 0.4;
/// Elevation clamp: past ~85 degrees the Z-up view matrix degenerates.
const PITCH_LIMIT: f32 = 85.0_f32.to_radians();
const CAMERA_DIST: f32 = 8.5;
const BASE_HALF_EXTENT: f32 = 4.6;
/// Camera animation speed, radians per frame at ~60 fps.
const VIEW_TWEEN: f32 = 0.06;

// Both views look down the -Y->+Y direction and only differ in pitch:
// the top-down view is the side view tilted up to the elevation clamp.
const SIDE_VIEW: (f32, f32) = (-90.0_f32.to_radians(), 0.0);
const TOP_VIEW: (f32, f32) = (-90.0_f32.to_radians(), PITCH_LIMIT);

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

fn push_line(v: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], color: [f32; 4]) {
    v.push(Vertex::new(a, color));
    v.push(Vertex::new(b, color));
}

/// One axis-aligned box, each face shaded by how much it faces the usual
/// top/front camera so boxes read as solids.
fn box_vertices(center: [f32; 3], half: [f32; 3], color: [f32; 4]) -> Vec<Vertex> {
    // (normal, tangent u, tangent v, shade)
    type Face = ([f32; 3], [f32; 3], [f32; 3], f32);
    const FACES: &[Face] = &[
        ([0., 0., 1.], [1., 0., 0.], [0., 1., 0.], 1.00), // +Z
        ([0., 0., -1.], [1., 0., 0.], [0., 1., 0.], 0.50), // -Z
        ([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], 0.85), // +X
        ([-1., 0., 0.], [0., 1., 0.], [0., 0., 1.], 0.70), // -X
        ([0., 1., 0.], [1., 0., 0.], [0., 0., 1.], 0.92), // +Y
        ([0., -1., 0.], [1., 0., 0.], [0., 0., 1.], 0.65), // -Y
    ];

    let mut v = Vec::new();
    for (n, u, t, shade) in FACES {
        let shade = |c: f32| c * shade;
        let color = [shade(color[0]), shade(color[1]), shade(color[2]), 1.0];
        let fc = [
            center[0] + n[0] * half[0],
            center[1] + n[1] * half[1],
            center[2] + n[2] * half[2],
        ];
        let hu = u[0] * half[0] + u[1] * half[1] + u[2] * half[2];
        let hv = t[0] * half[0] + t[1] * half[1] + t[2] * half[2];
        let corners = [
            [
                fc[0] - u[0] * hu - t[0] * hv,
                fc[1] - u[1] * hu - t[1] * hv,
                fc[2] - u[2] * hu - t[2] * hv,
            ],
            [
                fc[0] + u[0] * hu - t[0] * hv,
                fc[1] + u[1] * hu - t[1] * hv,
                fc[2] + u[2] * hu - t[2] * hv,
            ],
            [
                fc[0] + u[0] * hu + t[0] * hv,
                fc[1] + u[1] * hu + t[1] * hv,
                fc[2] + u[2] * hu + t[2] * hv,
            ],
            [
                fc[0] - u[0] * hu + t[0] * hv,
                fc[1] - u[1] * hu + t[1] * hv,
                fc[2] - u[2] * hu + t[2] * hv,
            ],
        ];
        // expand the quad into two triangles: draw() consumes consecutive
        // index triples, there is no quad primitive
        for &(a, b, c) in &[(0usize, 1usize, 2usize), (0, 2, 3)] {
            for p in [corners[a], corners[b], corners[c]] {
                v.push(Vertex::new(p, color));
            }
        }
    }
    v
}

fn grid_and_axes() -> (Vec<Vertex>, Vec<Vertex>) {
    let mut grid = Vec::new();
    let e = GRID_EXTENT;
    for i in -3i32..=3 {
        let f = i as f32;
        if i == 0 {
            continue; // the axes take the center lines
        }
        push_line(&mut grid, [f, -e, 0.], [f, e, 0.], GRID_COLOR);
        push_line(&mut grid, [-e, f, 0.], [e, f, 0.], GRID_COLOR);
    }

    let mut axes = Vec::new();
    push_line(&mut axes, [-e, 0., 0.], [e, 0., 0.], AXIS_X);
    push_line(&mut axes, [0., -e, 0.], [0., e, 0.], AXIS_Y);
    push_line(&mut axes, [0., 0., -1.], [0., 0., 2.5], AXIS_Z);
    (grid, axes)
}

/// A few pillars of different heights: squares from the top, but the side
/// view reveals that the world has depth.
fn pillars() -> Vec<Vertex> {
    let mut v = Vec::new();
    for (x, y, height, color) in [
        (-1.5f32, -1.0f32, 2.2f32, [0.55, 0.60, 0.68, 1.0]),
        (1.2, 0.8, 1.4, [0.48, 0.55, 0.62, 1.0]),
        (0.8, -1.6, 0.8, [0.60, 0.54, 0.48, 1.0]),
    ] {
        let h = height / 2.0;
        v.extend(box_vertices([x, y, h], [0.35, 0.35, h], color));
    }
    v
}

fn move_toward(current: &mut f32, target: f32, max_delta: f32) {
    let diff = target - *current;
    if diff.abs() <= max_delta {
        *current = target;
    } else {
        *current += diff.signum() * max_delta;
    }
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    line_pipeline: Pipeline,
    cube_pipeline: Pipeline,
    grid_bindings: Bindings,
    axes_bindings: Bindings,
    pillars_bindings: Bindings,
    entity_bindings: Bindings,
    pos: [f32; 2],
    yaw: f32,
    pitch: f32,
    target_yaw: f32,
    target_pitch: f32,
    top_view: bool,
    /// keyed by the full u16 range of KeyCode discriminants
    keys: [bool; u16::MAX as usize + 1],
}

impl Stage {
    pub fn new() -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let shader = ctx
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
            shader,
            PipelineParams {
                primitive_type: PrimitiveType::Lines,
                depth_test: Comparison::LessOrEqual,
                depth_write: false,
                ..Default::default()
            },
        );
        let cube_pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float3),
                VertexAttribute::new("in_color", VertexFormat::Float4),
            ],
            shader,
            PipelineParams {
                depth_write: true,
                depth_test: Comparison::LessOrEqual,
                ..Default::default()
            },
        );

        let identity: Vec<u16> = (0..1024).collect();
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&identity),
        );

        let (grid, axes) = grid_and_axes();
        let pillars = pillars();
        let entity = box_vertices([0., 0., ENTITY_HALF], [ENTITY_HALF; 3], ENTITY_COLOR);

        let mk_bindings = |ctx: &mut Box<dyn RenderingBackend>, verts: &[Vertex]| Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(verts),
            )],
            index_buffer,
            images: vec![],
        };

        let grid_bindings = mk_bindings(&mut ctx, &grid);
        let axes_bindings = mk_bindings(&mut ctx, &axes);
        let pillars_bindings = mk_bindings(&mut ctx, &pillars);

        // the player cube moves, so its vertices live in a dynamic buffer
        let entity_bindings = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Dynamic,
                BufferSource::slice(&entity),
            )],
            index_buffer,
            images: vec![],
        };

        // start in the top-down view: it looks like a flat 2D game
        Stage {
            ctx,
            line_pipeline,
            cube_pipeline,
            grid_bindings,
            axes_bindings,
            pillars_bindings,
            entity_bindings,
            pos: [0.3, -0.6],
            yaw: TOP_VIEW.0,
            pitch: TOP_VIEW.1,
            target_yaw: TOP_VIEW.0,
            target_pitch: TOP_VIEW.1,
            top_view: true,
            keys: [false; u16::MAX as usize + 1],
        }
    }

    fn view_matrix(&self) -> Mat4 {
        view_matrix(
            Vec3::new(
                CAMERA_DIST * self.pitch.cos() * self.yaw.cos(),
                CAMERA_DIST * self.pitch.cos() * self.yaw.sin(),
                CAMERA_DIST * self.pitch.sin(),
            ),
            Vec3::ZERO,
        )
    }

    fn projection(&self, width: f32, height: f32) -> Mat4 {
        let aspect = width / height;
        let half = BASE_HALF_EXTENT;
        glam::camera::rh::proj::opengl::orthographic(
            -half * aspect,
            half * aspect,
            -half,
            half,
            0.1,
            100.0,
        )
    }

    fn update_entity_vertices(&mut self) {
        let verts = box_vertices(
            [self.pos[0], self.pos[1], ENTITY_HALF],
            [ENTITY_HALF; 3],
            ENTITY_COLOR,
        );
        self.ctx.buffer_update(
            self.entity_bindings.vertex_buffers[0],
            BufferSource::slice(&verts),
        );
    }

    fn held(&self, key: KeyCode) -> bool {
        self.keys[key as usize]
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {
        // world-plane movement: +X right, +Y "up" in the top-down view
        let mut dx = 0.0;
        let mut dy = 0.0;
        if self.held(KeyCode::W) || self.held(KeyCode::Up) {
            dy += MOVE_SPEED;
        }
        if self.held(KeyCode::S) || self.held(KeyCode::Down) {
            dy -= MOVE_SPEED;
        }
        if self.held(KeyCode::D) || self.held(KeyCode::Right) {
            dx += MOVE_SPEED;
        }
        if self.held(KeyCode::A) || self.held(KeyCode::Left) {
            dx -= MOVE_SPEED;
        }
        self.pos[0] = (self.pos[0] + dx).clamp(-MOVE_LIMIT, MOVE_LIMIT);
        self.pos[1] = (self.pos[1] + dy).clamp(-MOVE_LIMIT, MOVE_LIMIT);
        self.update_entity_vertices();

        // animate the camera toward the active view
        move_toward(&mut self.yaw, self.target_yaw, VIEW_TWEEN);
        move_toward(&mut self.pitch, self.target_pitch, VIEW_TWEEN);
    }

    fn draw(&mut self) {
        let (width, height) = window::screen_size();
        let mvp = self.projection(width, height) * self.view_matrix();

        self.ctx.begin_default_pass(PassAction::Clear {
            color: Some(BG_COLOR),
            depth: Some(1.),
            stencil: None,
        });

        self.ctx.apply_pipeline(&self.line_pipeline);
        self.ctx.apply_bindings(&self.grid_bindings);
        self.ctx
            .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp }));
        self.ctx.draw(0, 48, 1);

        self.ctx.apply_bindings(&self.axes_bindings);
        self.ctx.draw(0, 6, 1);

        self.ctx.apply_pipeline(&self.cube_pipeline);
        self.ctx.apply_bindings(&self.pillars_bindings);
        self.ctx
            .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp }));
        self.ctx.draw(0, 36 * 3, 1);

        self.ctx.apply_bindings(&self.entity_bindings);
        self.ctx.draw(0, 36, 1);

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _mods: KeyMods, repeat: bool) {
        self.keys[keycode as usize] = true;
        if !repeat && keycode == KeyCode::Space {
            // toggle top-down <-> side view
            self.top_view = !self.top_view;
            let (yaw, pitch) = if self.top_view { TOP_VIEW } else { SIDE_VIEW };
            self.target_yaw = yaw;
            self.target_pitch = pitch;
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _mods: KeyMods) {
        self.keys[keycode as usize] = false;
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "view toggle".to_owned(),
        ..Default::default()
    };

    miniquad::start(conf, move || Box::new(Stage::new()));
}

/// View matrix for a right-handed, Z-up Cartesian world.
///
/// The backends end in a y-up clip space with the camera looking down -Z.
/// This builds the world-to-view rotation/translation explicitly instead of
/// hiding the convention inside a look-at helper:
///
///   forward = normalize(target - eye)
///   right   = normalize(forward x world_up), world_up = +Z
///   up      = right x forward
///
/// so the world axes keep their right-handed relation on screen: +Z up,
/// depth increasing away from the eye, and no axis flipping between the
/// Cartesian scene and what the rasterizer receives.
fn view_matrix(eye: Vec3, target: Vec3) -> Mat4 {
    let forward = (target - eye).normalize();
    let right = forward.cross(Vec3::Z).normalize();
    let up = right.cross(forward);
    Mat4::from_cols_array_2d(&[
        [right.x, up.x, -forward.x, 0.0],
        [right.y, up.y, -forward.y, 0.0],
        [right.z, up.z, -forward.z, 0.0],
        [-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.0],
    ])
}

mod shader {
    use miniquad::*;

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
        float3 in_pos   [[attribute(0)]];
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
