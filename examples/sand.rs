//! Falling-sand toy.
//!
//! Hold the **left mouse button** to pour sand; grains fall, pile up and
//! slide off slopes.
//!
//! The simulation grid is 200x150 cells, each cell drawn as an exact 4x4
//! pixel block (800x600 window, not resizable), so every grain lands on the
//! pixel grid with no half-pixel seams: quad edges sit on integer pixel
//! boundaries and the overlay projection maps one unit to one pixel.

use tinyquad::*;

const CELL: f32 = 4.0;
const WIN_W: i32 = 800;
const WIN_H: i32 = 600;
const GRID_W: usize = WIN_W as usize / CELL as usize;
const GRID_H: usize = WIN_H as usize / CELL as usize;

/// Palette entry 0 stays unused (it would mean "empty").
const PALETTE_SIZE: usize = 256;

/// Warm sand color; each palette entry is the same hue at a different
/// brightness so grains get a natural speckled look.
const SAND_BASE: [f32; 3] = [0.91, 0.76, 0.42];

fn build_palette() -> Vec<[f32; 4]> {
    (0..PALETTE_SIZE)
        .map(|i| {
            let v = 0.72 + i as f32 / (PALETTE_SIZE - 1) as f32 * 0.38;
            [SAND_BASE[0] * v, SAND_BASE[1] * v, SAND_BASE[2] * v, 1.0]
        })
        .collect()
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    fn new(pos: [f32; 2], color: [f32; 4]) -> Vertex {
        Vertex { pos, color }
    }
}

/// One quad = CELL x CELL pixels, corners on integer pixel boundaries.
fn push_cell(v: &mut Vec<Vertex>, gx: usize, gy: usize, color: [f32; 4]) {
    let x0 = gx as f32 * CELL;
    let y0 = gy as f32 * CELL;
    let x1 = x0 + CELL;
    let y1 = y0 + CELL;
    for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y0), (x1, y1), (x0, y1)] {
        v.push(Vertex::new([x, y], color));
    }
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    sand_bindings: Bindings,
    palette: Vec<[f32; 4]>,
    /// 0 = empty, otherwise palette index (1..PALETTE_SIZE).
    grid: Vec<u8>,
    mouse: (f32, f32),
    pouring: bool,
}

impl Stage {
    pub fn new() -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: shader::VERTEX,
                    fragment: shader::FRAGMENT,
                },
                shader::meta(),
            )
            .unwrap();

        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_color", VertexFormat::Float4),
            ],
            shader,
            PipelineParams::default(),
        );

        // worst case: every cell filled
        const MAX_VERTS: usize = GRID_W * GRID_H * 6;
        let empty = vec![Vertex::new([0., 0.], [0.; 4]); MAX_VERTS];
        // u32 identity indices: with more than 65535 vertices u16 would
        // overflow, and draw() reads `count` indices from this buffer
        let identity: Vec<u32> = (0..MAX_VERTS as u32).collect();
        let sand_bindings = Bindings {
            vertex_buffers: vec![ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Dynamic,
                BufferSource::slice(&empty),
            )],
            index_buffer: ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&identity),
            ),
            images: vec![],
        };

        Stage {
            ctx,
            pipeline,
            sand_bindings,
            palette: build_palette(),
            grid: vec![0; GRID_W * GRID_H],
            mouse: (WIN_W as f32 / 2., WIN_H as f32 / 2.),
            pouring: false,
        }
    }

    fn cell(&self, x: usize, y: usize) -> u8 {
        self.grid[y * GRID_W + x]
    }

    fn set_cell(&mut self, x: usize, y: usize, v: u8) {
        self.grid[y * GRID_W + x] = v;
    }

    /// One cellular-automaton step, scanning bottom-up so a grain that just
    /// moved down is not moved twice in the same step.
    fn step(&mut self) {
        for y in (0..GRID_H - 1).rev() {
            for x in 0..GRID_W {
                let v = self.cell(x, y);
                if v == 0 || self.cell(x, y + 1) != 0 {
                    continue;
                }
                // fall straight down
                self.set_cell(x, y + 1, v);
                self.set_cell(x, y, 0);
            }
        }
        // diagonal slides, in a separate pass over the already-fallen state:
        // a grain only slides if the cell below its diagonal is free too
        for y in (0..GRID_H - 1).rev() {
            let lean_left = quad_rand::gen_range(0, 2) == 0;
            for x in 0..GRID_W {
                let v = self.cell(x, y);
                if v == 0 || self.cell(x, y + 1) != 0 {
                    continue;
                }
                let dirs: [i32; 2] = if lean_left { [-1, 1] } else { [1, -1] };
                for dx in dirs {
                    let nx = x as i32 + dx;
                    if nx < 0 || nx >= GRID_W as i32 {
                        continue;
                    }
                    let nx = nx as usize;
                    if self.cell(nx, y + 1) == 0 {
                        self.set_cell(nx, y + 1, v);
                        self.set_cell(x, y, 0);
                        break;
                    }
                }
            }
        }
    }

    /// Pour a few grains around the cursor.
    fn pour(&mut self, px: f32, py: f32) {
        let cx = (px / CELL) as i32;
        let cy = (py / CELL) as i32;
        for _ in 0..3 {
            let dx = quad_rand::gen_range(-2, 3);
            let dy = quad_rand::gen_range(-1, 3);
            let x = cx + dx;
            let y = cy + dy;
            if x < 0 || y < 0 || x >= GRID_W as i32 || y >= GRID_H as i32 {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            if self.cell(x, y) == 0 {
                let color = quad_rand::gen_range(1, PALETTE_SIZE as u32) as u8;
                self.set_cell(x, y, color);
            }
        }
    }

    fn sand_vertices(&self) -> Vec<Vertex> {
        let mut verts = Vec::new();
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let v = self.cell(x, y);
                if v != 0 {
                    push_cell(&mut verts, x, y, self.palette[v as usize]);
                }
            }
        }
        verts
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {
        if self.pouring {
            let (x, y) = self.mouse;
            self.pour(x, y);
        }
        self.step();
    }

    fn draw(&mut self) {
        let (width, height) = window::screen_size();
        // y-down pixel coordinates: one unit = one pixel, row 0 at the top
        let ortho_2d =
            glam::camera::rh::proj::opengl::orthographic(0.0, width, height, 0.0, -1.0, 1.0);

        let sand = self.sand_vertices();
        self.ctx.buffer_update(
            self.sand_bindings.vertex_buffers[0],
            BufferSource::slice(&sand),
        );

        self.ctx.begin_default_pass(PassAction::Clear {
            color: Some((0.10, 0.10, 0.12, 1.0)),
            depth: Some(1.),
            stencil: None,
        });

        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.sand_bindings);
        self.ctx
            .apply_uniforms(UniformsSource::table(&shader::Uniforms { mvp: ortho_2d }));
        self.ctx.draw(0, sand.len() as _, 1);

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left {
            self.pouring = true;
            self.mouse = (x, y);
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        if button == MouseButton::Left {
            self.pouring = false;
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.mouse = (x, y);
    }
}

fn main() {
    // the grid maps 1:1 onto the 800x600 pixel grid - do not allow resizing
    let conf = conf::Conf {
        window_title: "sand".to_owned(),
        window_resizable: false,
        window_width: WIN_W,
        window_height: WIN_H,
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
    attribute vec2 in_pos;
    attribute vec4 in_color;

    uniform mat4 mvp;

    varying lowp vec4 color;

    void main() {
        gl_Position = mvp * vec4(in_pos, 0.0, 1.0);
        color = in_color;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec4 color;

    void main() {
        gl_FragColor = color;
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
