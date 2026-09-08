//! A Blender-style "Cartesian" 3D view: the default 2x2x2 cube sitting on a
//! unit grid, with the X/Y/Z axes drawn through the origin and labeled.
//!
//! The camera is orthographic, so the cube keeps its right angles on screen -
//! the plain grid-paper look, just from a 3/4 viewpoint. Like in Blender the
//! Z axis points up and the grid lies in the XY plane.
//!
//! Labels are tiny hand-drawn glyphs: after rendering the 3D scene, the axis
//! end points are projected to screen space and the glyphs are drawn as small
//! colored quads with a separate 2D pipeline.

use glam::{Mat4, Vec3, Vec4};
use miniquad::*;

const GRID_EXTENT: f32 = 3.0;
const GRID_COLOR: [f32; 4] = [0.30, 0.30, 0.30, 1.0];
const AXIS_X: [f32; 4] = [0.80, 0.30, 0.30, 1.0];
const AXIS_Y: [f32; 4] = [0.30, 0.80, 0.30, 1.0];
const AXIS_Z: [f32; 4] = [0.30, 0.50, 0.90, 1.0];
const EDGE_COLOR: [f32; 4] = [0.12, 0.12, 0.14, 1.0];
const BG_COLOR: (f32, f32, f32, f32) = (0.22, 0.22, 0.24, 1.0);

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

const CUBE_HALF: f32 = 1.0;

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

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    line_pipeline: Pipeline,
    cube_pipeline: Pipeline,
    text_pipeline: Pipeline,
    grid_bindings: Bindings,
    axes_bindings: Bindings,
    edges_bindings: Bindings,
    cube_bindings: Bindings,
    text_bindings: Bindings,
    labels: Vec<(char, [f32; 4], [f32; 3])>,
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

        // Allocate the dynamic text buffer up front: buffer_update never
        // grows a buffer, it only overwrites existing contents.
        const TEXT_MAX_VERTS: usize = 1024;
        let text_verts = vec![Vertex::new([0.; 3], [0.; 4]); TEXT_MAX_VERTS];
        let text_bindings = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Dynamic,
                BufferSource::slice(&text_verts),
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
            text_bindings,
            labels: vec![
                ('X', AXIS_X, [GRID_EXTENT + 0.45, 0.1, 0.]),
                ('Y', AXIS_Y, [0.1, GRID_EXTENT + 0.45, 0.]),
                ('Z', AXIS_Z, [0.1, 0., GRID_EXTENT + 0.45]),
            ],
        }
    }

    fn view_projection(&self, width: f32, height: f32) -> Mat4 {
        // Orthographic 3/4 view from the (+X, -Y, +Z) octant, Z up: the
        // +X axis comes toward the lower right, +Y recedes to the upper
        // right and +Z points up, so the frame reads right-handed.
        let eye = Vec3::new(4.5, -4.5, 3.4);
        let view = view_matrix(eye, Vec3::ZERO);

        let half = 4.4;
        let aspect = width / height;
        let proj = glam::camera::rh::proj::opengl::orthographic(
            -half * aspect,
            half * aspect,
            -half,
            half,
            0.1,
            100.0,
        );
        proj * view
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
            let (sx, sy) = match Self::project(mvp, *world) {
                Some(p) => (p.0 * width, p.1 * height),
                None => continue,
            };
            let glyph = glyph(*ch);
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
                        verts.push(Vertex::new([x0 + dx, y0 + dy, 0.], *color));
                    }
                }
            }
        }
        verts
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        let (width, height) = window::screen_size();
        let mvp = self.view_projection(width, height);
        // y is flipped on purpose: label coordinates are in screen space
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

        // labels on top, in screen space
        let label_verts = self.label_vertices(&mvp, width, height);
        if !label_verts.is_empty() {
            self.ctx.buffer_update(
                self.text_bindings.vertex_buffers[0],
                BufferSource::slice(&label_verts),
            );
            self.ctx.apply_pipeline(&self.text_pipeline);
            self.ctx.apply_bindings(&self.text_bindings);
            self.ctx
                .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp: ortho_2d }));
            self.ctx.draw(0, label_verts.len() as _, 1);
        }

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }
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

fn main() {
    miniquad::start(conf::Conf::default(), move || Box::new(Stage::new()));
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
