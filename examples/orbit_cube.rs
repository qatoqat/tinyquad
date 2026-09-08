//! The axis-colored Cartesian cube with a Blender-style viewport camera.
//!
//! Controls:
//! - **left drag**: orbit around the world Z axis (elevation clamped near
//!   the poles so the Z-up view can not flip)
//! - **scroll**: zoom
//! - **1 / Ctrl+1**: front / back view
//! - **3 / Ctrl+3**: right / left view
//! - **7 / Ctrl+7**: top / bottom view
//! - **9**: jump to the opposite side
//! - **5**: toggle orthographic / perspective projection
//!
//! A little navigation compass in the top right corner mirrors the axes
//! orientation, like the one Blender draws.

use glam::{Mat4, Vec3, Vec4};
use miniquad::*;

const GRID_EXTENT: f32 = 3.0;
const GRID_COLOR: [f32; 4] = [0.30, 0.30, 0.30, 1.0];
const AXIS_X: [f32; 4] = [0.80, 0.30, 0.30, 1.0];
const AXIS_Y: [f32; 4] = [0.30, 0.80, 0.30, 1.0];
const AXIS_Z: [f32; 4] = [0.30, 0.50, 0.90, 1.0];
const EDGE_COLOR: [f32; 4] = [0.12, 0.12, 0.14, 1.0];
const BG_COLOR: (f32, f32, f32, f32) = (0.22, 0.22, 0.24, 1.0);

const CUBE_HALF: f32 = 1.0;

/// Camera orbit state, radians.
const START_YAW: f32 = -45.0_f32.to_radians();
const START_PITCH: f32 = 28.0_f32.to_radians();
const CAMERA_DIST: f32 = 7.2;
/// Elevation clamp: past ~85 degrees the Z-up view matrix degenerates.
const PITCH_LIMIT: f32 = 85.0_f32.to_radians();
const DRAG_SENSITIVITY: f32 = 0.008;
/// Ortho half extent at zoom 1.0; also scales the perspective distance.
const BASE_HALF_EXTENT: f32 = 4.4;
const PERSPECTIVE_FOV: f32 = 30.0_f32.to_radians();

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

/// Faces are painted with their axis color; the positive side of each axis
/// uses the full color and the negative side a dimmed version of it.
const fn dim(color: [f32; 4]) -> [f32; 4] {
    [color[0] * 0.45, color[1] * 0.45, color[2] * 0.45, color[3]]
}

fn cube_faces() -> Vec<Vertex> {
    let corner = |x: f32, y: f32, z: f32| [x * CUBE_HALF, y * CUBE_HALF, z * CUBE_HALF];
    let mut v = Vec::new();
    for (corners, color) in CUBE_FACES {
        let pts: [[f32; 3]; 4] = corners.map(|p| corner(p[0], p[1], p[2]));
        // expand the quad into two triangles: draw() consumes consecutive
        // index triples, there is no quad primitive
        for &(a, b, c) in &[(0usize, 1usize, 2usize), (0, 2, 3)] {
            for p in [pts[a], pts[b], pts[c]] {
                v.push(Vertex::new(p, *color));
            }
        }
    }
    v
}

// (corners as unit-sign triplets, face color) for each of the 6 faces.
const CUBE_FACES: &[([[f32; 3]; 4], [f32; 4])] = &[
    // top (+Z)
    (
        [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
        AXIS_Z,
    ),
    // bottom (-Z)
    (
        [
            [-1., -1., -1.],
            [1., -1., -1.],
            [1., 1., -1.],
            [-1., 1., -1.],
        ],
        dim(AXIS_Z),
    ),
    // +X
    (
        [[1., -1., -1.], [1., -1., 1.], [1., 1., 1.], [1., 1., -1.]],
        AXIS_X,
    ),
    // -X
    (
        [
            [-1., -1., -1.],
            [-1., -1., 1.],
            [-1., 1., 1.],
            [-1., 1., -1.],
        ],
        dim(AXIS_X),
    ),
    // +Y
    (
        [[1., 1., -1.], [1., 1., 1.], [-1., 1., 1.], [-1., 1., -1.]],
        AXIS_Y,
    ),
    // -Y
    (
        [
            [1., -1., -1.],
            [1., -1., 1.],
            [-1., -1., 1.],
            [-1., -1., -1.],
        ],
        dim(AXIS_Y),
    ),
];

fn cube_edges() -> Vec<Vertex> {
    const C: f32 = CUBE_HALF * 1.002; // tiny offset so edges win the depth test
    let c = [
        [-C, -C, -C],
        [C, -C, -C],
        [C, C, -C],
        [-C, C, -C],
        [-C, -C, C],
        [C, -C, C],
        [C, C, C],
        [-C, C, C],
    ];
    let pairs = [
        (0usize, 1usize),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut v = Vec::new();
    for (a, b) in pairs {
        push_line(&mut v, c[a], c[b], EDGE_COLOR);
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
    push_line(&mut axes, [0., 0., -1.], [0., 0., 3.], AXIS_Z);
    (grid, axes)
}

/// 5x5 pixel glyphs for the axis labels.
fn glyph(ch: char) -> &'static [&'static str] {
    match ch {
        'X' => &["#...#", ".#.#.", "..#..", ".#.#.", "#...#"],
        'Y' => &["#...#", ".#.#.", "..#..", "..#..", "..#.."],
        _ => &["#####", "...#.", "..#..", ".#...", "#####"], // Z
    }
}

/// Emit one glyph as little colored quads centered on (sx, sy) pixels.
fn push_glyph(verts: &mut Vec<Vertex>, ch: char, color: [f32; 4], sx: f32, sy: f32, pixel: f32) {
    let glyph = glyph(ch);
    let rows = glyph.len() as f32;
    let cols = glyph[0].len() as f32;
    for (row, line) in glyph.iter().enumerate() {
        for (col, c) in line.chars().enumerate() {
            if c != '#' {
                continue;
            }
            let x0 = sx + (col as f32 - cols / 2.0) * pixel;
            let y0 = sy - (rows / 2.0 - row as f32) * pixel;
            for (dx, dy) in [
                (0., 0.),
                (pixel, 0.),
                (pixel, pixel),
                (0., 0.),
                (pixel, pixel),
                (0., pixel),
            ] {
                verts.push(Vertex::new([x0 + dx, y0 + dy, 0.], color));
            }
        }
    }
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    line_pipeline: Pipeline,
    cube_pipeline: Pipeline,
    text_pipeline: Pipeline,
    grid_bindings: Bindings,
    axes_bindings: Bindings,
    edges_bindings: Bindings,
    cube_bindings: Bindings,
    overlay_bindings: Bindings,
    compass_bindings: Bindings,
    labels: Vec<(char, [f32; 4], [f32; 3])>,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    perspective: bool,
    dragging: bool,
    last_mouse: (f32, f32),
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
                // test against the cube so lines behind it are hidden, but
                // never write depth: the wireframe must not occlude itself
                // or the faces drawn after it
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
            // without this the faces would paint in buffer order and back
            // faces (drawn later) would cover front ones
            PipelineParams {
                depth_write: true,
                depth_test: Comparison::LessOrEqual,
                ..Default::default()
            },
        );
        // overlays (labels, compass glyphs) always draw on top
        let text_pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float3),
                VertexAttribute::new("in_color", VertexFormat::Float4),
            ],
            shader,
            PipelineParams::default(),
        );

        // One identity index buffer is enough for every draw: vertices are
        // stored in draw order, so index i simply picks vertex i.
        let identity: Vec<u16> = (0..1024).collect();
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&identity),
        );

        let (grid, axes) = grid_and_axes();
        let edges = cube_edges();
        let faces = cube_faces();

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
        let edges_bindings = mk_bindings(&mut ctx, &edges);
        let cube_bindings = mk_bindings(&mut ctx, &faces);

        // Allocate the dynamic overlay buffers up front: buffer_update never
        // grows a buffer, it only overwrites existing contents.
        const OVERLAY_MAX_VERTS: usize = 2048;
        const COMPASS_MAX_LINES: usize = 128;
        let overlay_verts = vec![Vertex::new([0.; 3], [0.; 4]); OVERLAY_MAX_VERTS];
        let compass_verts = vec![Vertex::new([0.; 3], [0.; 4]); COMPASS_MAX_LINES];
        let overlay_bindings = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Dynamic,
                BufferSource::slice(&overlay_verts),
            )],
            index_buffer,
            images: vec![],
        };
        let compass_bindings = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Dynamic,
                BufferSource::slice(&compass_verts),
            )],
            index_buffer,
            images: vec![],
        };

        Stage {
            ctx,
            line_pipeline,
            cube_pipeline,
            text_pipeline,
            grid_bindings,
            axes_bindings,
            edges_bindings,
            cube_bindings,
            overlay_bindings,
            compass_bindings,
            labels: vec![
                ('X', AXIS_X, [GRID_EXTENT + 0.45, 0.1, 0.]),
                ('Y', AXIS_Y, [0.1, GRID_EXTENT + 0.45, 0.]),
                ('Z', AXIS_Z, [0.1, 0., GRID_EXTENT + 0.45]),
            ],
            yaw: START_YAW,
            pitch: START_PITCH,
            zoom: 1.0,
            perspective: false,
            dragging: false,
            last_mouse: (0., 0.),
        }
    }

    fn eye_position(&self) -> Vec3 {
        let dist = CAMERA_DIST * self.zoom;
        Vec3::new(
            dist * self.pitch.cos() * self.yaw.cos(),
            dist * self.pitch.cos() * self.yaw.sin(),
            dist * self.pitch.sin(),
        )
    }

    fn projection(&self, width: f32, height: f32) -> Mat4 {
        let aspect = width / height;
        if self.perspective {
            glam::camera::rh::proj::opengl::perspective(PERSPECTIVE_FOV, aspect, 0.1, 100.0)
        } else {
            let half = BASE_HALF_EXTENT * self.zoom;
            glam::camera::rh::proj::opengl::orthographic(
                -half * aspect,
                half * aspect,
                -half,
                half,
                0.1,
                100.0,
            )
        }
    }

    /// Project a world point to [0, 1]x[0, 1] screen coordinates (origin at
    /// the bottom left). Returns None for points behind the camera.
    fn project(mvp: &Mat4, p: [f32; 3]) -> Option<(f32, f32)> {
        let clip = *mvp * Vec4::new(p[0], p[1], p[2], 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        Some((ndc_x * 0.5 + 0.5, 1.0 - (ndc_y * 0.5 + 0.5)))
    }

    /// Turn each label glyph into little colored quads in pixel coordinates.
    fn label_vertices(&self, mvp: &Mat4, width: f32, height: f32) -> Vec<Vertex> {
        let mut verts = Vec::new();
        let pixel = (height / 160.0).max(2.0);

        for (ch, color, world) in &self.labels {
            if let Some((sx, sy)) = Self::project(mvp, *world) {
                push_glyph(&mut verts, *ch, *color, sx * width, sy * height, pixel);
            }
        }
        verts
    }

    /// The Blender-style navigation compass: a circle with the world axes
    /// drawn from its center, oriented as they point on screen. Positive
    /// directions get their glyph at the tip, negative directions a dimmed
    /// line. Returns the line vertices; glyph quads are appended to
    /// `glyph_verts`.
    fn compass(
        &self,
        view: &Mat4,
        width: f32,
        height: f32,
        glyph_verts: &mut Vec<Vertex>,
    ) -> Vec<Vertex> {
        // The first three rows of the view matrix are the screen-space
        // right/up/depth directions of the world axes. y is negated because
        // pixel coordinates grow downward.
        let axes_on_screen = |axis: Vec3| {
            Vec3::new(
                view.x_axis.x * axis.x + view.y_axis.x * axis.y + view.z_axis.x * axis.z,
                -(view.x_axis.y * axis.x + view.y_axis.y * axis.y + view.z_axis.y * axis.z),
                0.0,
            )
        };

        let pixel = (height / 160.0).max(2.0);
        let pivot = (width - 80.0, 84.0);
        let radius = 36.0;

        let mut verts = Vec::new();

        // surrounding circle
        let segments = 32;
        for i in 0..segments {
            let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            push_line(
                &mut verts,
                [pivot.0 + a0.cos() * radius, pivot.1 + a0.sin() * radius, 0.],
                [pivot.0 + a1.cos() * radius, pivot.1 + a1.sin() * radius, 0.],
                [0.55, 0.55, 0.58, 1.0],
            );
        }

        for (axis, ch, color) in [
            (Vec3::X, 'X', AXIS_X),
            (Vec3::Y, 'Y', AXIS_Y),
            (Vec3::Z, 'Z', AXIS_Z),
        ] {
            let dir = axes_on_screen(axis);
            push_line(
                &mut verts,
                [pivot.0, pivot.1, 0.],
                [pivot.0 + dir.x * radius, pivot.1 + dir.y * radius, 0.],
                color,
            );
            push_line(
                &mut verts,
                [pivot.0, pivot.1, 0.],
                [pivot.0 - dir.x * radius, pivot.1 - dir.y * radius, 0.],
                dim(color),
            );
            let tip_scale = radius * 1.3;
            push_glyph(
                glyph_verts,
                ch,
                color,
                pivot.0 + dir.x * tip_scale,
                pivot.1 + dir.y * tip_scale,
                pixel * 0.75,
            );
        }

        verts
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        let (width, height) = window::screen_size();
        let view = view_matrix(self.eye_position(), Vec3::ZERO);
        let mvp = self.projection(width, height) * view;
        // y is flipped on purpose: overlay coordinates are in screen space
        // (y down), while opengl::orthographic defaults to y up.
        let ortho_2d =
            glam::camera::rh::proj::opengl::orthographic(0.0, width, height, 0.0, -1.0, 1.0);

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
        self.ctx.apply_bindings(&self.cube_bindings);
        self.ctx
            .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp }));
        // 6 faces x 6 vertices (two triangles each)
        self.ctx.draw(0, 36, 1);

        // wireframe over the shaded faces, depth-tested against them
        self.ctx.apply_pipeline(&self.line_pipeline);
        self.ctx.apply_bindings(&self.edges_bindings);
        self.ctx.draw(0, 24, 1);

        // 2D overlay: scene labels plus the compass glyphs in one buffer,
        // the compass lines in their own
        let mut overlay = self.label_vertices(&mvp, width, height);
        let compass_lines = self.compass(&view, width, height, &mut overlay);

        if !overlay.is_empty() {
            self.ctx.buffer_update(
                self.overlay_bindings.vertex_buffers[0],
                BufferSource::slice(&overlay),
            );
            self.ctx.apply_pipeline(&self.text_pipeline);
            self.ctx.apply_bindings(&self.overlay_bindings);
            self.ctx
                .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp: ortho_2d }));
            self.ctx.draw(0, overlay.len() as _, 1);
        }

        if !compass_lines.is_empty() {
            self.ctx.buffer_update(
                self.compass_bindings.vertex_buffers[0],
                BufferSource::slice(&compass_lines),
            );
            self.ctx.apply_pipeline(&self.line_pipeline);
            self.ctx.apply_bindings(&self.compass_bindings);
            self.ctx
                .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp: ortho_2d }));
            self.ctx.draw(0, compass_lines.len() as _, 1);
        }

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left {
            self.dragging = true;
            self.last_mouse = (x, y);
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        if button == MouseButton::Left {
            self.dragging = false;
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        if !self.dragging {
            return;
        }
        let (last_x, last_y) = self.last_mouse;
        self.yaw += (x - last_x) * DRAG_SENSITIVITY;
        // dragging up tilts the camera toward the top of the object
        self.pitch -= (y - last_y) * DRAG_SENSITIVITY;
        self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
        self.last_mouse = (x, y);
    }

    fn mouse_wheel_event(&mut self, _x: f32, y: f32) {
        self.zoom = (self.zoom * (1.0 - 0.1 * y)).clamp(0.25, 4.0);
    }

    fn key_down_event(&mut self, keycode: KeyCode, mods: KeyMods, repeat: bool) {
        if repeat {
            return;
        }
        let set_view = |stage: &mut Stage, yaw_deg: f32, pitch_deg: f32| {
            stage.yaw = yaw_deg.to_radians();
            stage.pitch = pitch_deg.to_radians();
        };
        match keycode {
            // front / back
            KeyCode::Key1 => set_view(self, if mods.ctrl { 90. } else { -90. }, 0.),
            // right / left
            KeyCode::Key3 => set_view(self, if mods.ctrl { 180. } else { 0. }, 0.),
            // top / bottom
            KeyCode::Key7 => set_view(
                self,
                self.yaw.to_degrees(),
                if mods.ctrl {
                    -PITCH_LIMIT.to_degrees()
                } else {
                    PITCH_LIMIT.to_degrees()
                },
            ),
            // opposite side
            KeyCode::Key9 => {
                self.yaw += std::f32::consts::PI;
                self.pitch = -self.pitch;
            }
            // toggle orthographic / perspective
            KeyCode::Key5 => self.perspective = !self.perspective,
            _ => {}
        }
    }
}

fn main() {
    miniquad::start(conf::Conf::default(), move || Box::new(Stage::new()));
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
