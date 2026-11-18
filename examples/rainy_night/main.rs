//! Rainy night: a playable cat in a flooded pixel-art street at night.
//!
//! The world is a voxel diorama (one voxel = one pixel; the cat is 24
//! voxels long) drawn orthographically from four Fez-style side views or
//! from straight above. Light from the street lamp, the bus-stop advert,
//! the billboard and lit windows is traced through the voxel structure
//! (shadows, tinted glass, dry shade under roofs); rain falls as real 3D
//! drops, the canal and puddles ripple, boxes float, lightning flashes and
//! thunder rolls. All sound is synthesized.
//!
//! Controls:
//! - **A / D** (or arrows): walk; in top-down view **W / S** too
//! - **Space**: jump
//! - **Shift**: sprint while moving, charge a high jump while standing
//! - **Ctrl** (or **S** in side view): drop through the platform below
//! - **Q / E**: rotate the camera 90 degrees
//! - **Tab / T**: toggle top-down / side view
//! - **Esc**: quit (desktop)
//!
//! Side views play by Fez rules: anything that *looks* like a floor on
//! screen is one, whatever its depth. The bus-stop roof and the billboard
//! line up only when looking along Z, which is the way to the fish.

mod audio;
mod cat;
mod game;
mod light;
mod model;
mod props;
mod rain;
mod render;
mod util;
mod water;
mod world;

use game::{Controls, Game};
use render::{Renderer, SH, SW};
use tinyquad::*;

const STEP: f64 = 1.0 / 120.0;
const SCALE: i32 = 3;

#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    bindings: Bindings,
    texture: TextureId,
    game: Game,
    renderer: Renderer,
    audio: audio::Audio,
    held: Vec<bool>,
    pressed: Vec<KeyCode>,
    last: f64,
    acc: f64,
}

impl Stage {
    fn new() -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();
        let renderer = Renderer::new();
        let texture = ctx.new_texture_from_rgba8(SW as u16, SH as u16, &renderer.rgba);
        ctx.texture_set_filter(texture, FilterMode::Nearest, MipmapFilterMode::None);

        #[rustfmt::skip]
        let vertices = [
            Vertex { pos: [-1.0, -1.0], uv: [0.0, 1.0] },
            Vertex { pos: [1.0, -1.0], uv: [1.0, 1.0] },
            Vertex { pos: [1.0, 1.0], uv: [1.0, 0.0] },
            Vertex { pos: [-1.0, 1.0], uv: [0.0, 0.0] },
        ];
        let vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&vertices),
        );
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&[0u16, 1, 2, 0, 2, 3]),
        );
        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![texture],
        };

        let source = match ctx.info().backend {
            Backend::OpenGl => ShaderSource::Glsl {
                vertex: shader::VERTEX,
                fragment: shader::FRAGMENT,
            },
            Backend::Metal => ShaderSource::Msl {
                program: shader::METAL,
            },
        };
        let shader = ctx.new_shader(source, shader::meta()).expect("blit shader");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
            ],
            shader,
            PipelineParams::default(),
        );

        Stage {
            ctx,
            pipeline,
            bindings,
            texture,
            game: Game::new(),
            renderer,
            audio: audio::Audio::new(),
            held: vec![false; 1 << 16],
            pressed: Vec::new(),
            last: date::now(),
            acc: 0.0,
        }
    }

    fn down(&self, keys: &[KeyCode]) -> bool {
        keys.iter().any(|&k| self.held[k as usize])
    }

    fn was_pressed(&self, keys: &[KeyCode]) -> bool {
        self.pressed.iter().any(|k| keys.contains(k))
    }

    fn controls(&self) -> Controls {
        use KeyCode::*;
        let mut c = Controls::default();
        c.cat.left = self.down(&[A, Left]);
        c.cat.right = self.down(&[D, Right]);
        c.cat.up = self.down(&[W, Up]);
        c.cat.down = self.down(&[S, Down]);
        c.cat.shift = self.down(&[LeftShift, RightShift]);
        c.cat.ctrl = self.down(&[LeftControl, RightControl]);
        c.cat.jump_pressed = self.was_pressed(&[Space]);
        c.cat.drop_pressed = self.was_pressed(&[LeftControl, RightControl]);
        c.down_pressed = self.was_pressed(&[S, Down]);
        c.rotate = i32::from(self.was_pressed(&[E])) - i32::from(self.was_pressed(&[Q]));
        c.toggle_top = self.was_pressed(&[Tab, T]);
        c
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {
        let now = date::now();
        self.acc = (self.acc + (now - self.last)).min(0.25);
        self.last = now;
        let mut controls = self.controls();
        while self.acc >= STEP {
            self.acc -= STEP;
            self.game.step(&controls, STEP as f32);
            // edge-triggered keys fire on the first step only
            controls.cat.jump_pressed = false;
            controls.cat.drop_pressed = false;
            controls.down_pressed = false;
            controls.rotate = 0;
            controls.toggle_top = false;
            self.pressed.clear();
        }
        self.game.sound(&mut self.audio);
    }

    fn draw(&mut self) {
        let scene = self.game.scene();
        self.renderer.render(&scene);
        self.ctx.texture_update(self.texture, &self.renderer.rgba);

        let (w, h) = window::screen_size();
        let fit = (w / SW as f32).min(h / SH as f32);
        let scale = if fit >= 1.0 { fit.floor() } else { fit };
        let (vw, vh) = (SW as f32 * scale, SH as f32 * scale);
        self.ctx
            .begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        self.ctx.apply_viewport(
            ((w - vw) * 0.5) as i32,
            ((h - vh) * 0.5) as i32,
            vw as i32,
            vh as i32,
        );
        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx.draw(0, 6, 1);
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _mods: KeyMods, repeat: bool) {
        self.held[keycode as usize] = true;
        if !repeat {
            self.pressed.push(keycode);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if keycode == KeyCode::Escape {
            window::quit();
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _mods: KeyMods) {
        self.held[keycode as usize] = false;
    }

    fn window_minimized_event(&mut self) {
        self.held.fill(false);
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "rainy night".to_owned(),
        window_width: SW as i32 * SCALE,
        window_height: SH as i32 * SCALE,
        ..Default::default()
    };
    tinyquad::start(conf, || Box::new(Stage::new()));
}

mod shader {
    use tinyquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec2 in_pos;
    attribute vec2 in_uv;
    varying lowp vec2 uv;
    void main() {
        gl_Position = vec4(in_pos, 0.0, 1.0);
        uv = in_uv;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 uv;
    uniform sampler2D tex;
    void main() {
        gl_FragColor = texture2D(tex, uv);
    }"#;

    pub const METAL: &str = r#"
    #include <metal_stdlib>
    using namespace metal;

    struct Vertex {
        float2 in_pos [[attribute(0)]];
        float2 in_uv  [[attribute(1)]];
    };

    struct RasterizerData {
        float4 position [[position]];
        float2 uv [[user(locn0)]];
    };

    vertex RasterizerData vertexShader(Vertex v [[stage_in]]) {
        RasterizerData out;
        out.position = float4(v.in_pos, 0.0, 1.0);
        out.uv = v.in_uv;
        return out;
    }

    fragment float4 fragmentShader(RasterizerData in [[stage_in]],
                                   texture2d<float> tex [[texture(0)]],
                                   sampler texSmplr [[sampler(0)]]) {
        return tex.sample(texSmplr, in.uv);
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout { uniforms: vec![] },
        }
    }
}

/// Headless snapshots for development: `RN_SHOTS=/tmp/rn cargo test --release
/// --example rainy_night -- --ignored --nocapture` steps the game through a
/// scripted walk and writes PPM frames.
#[cfg(test)]
mod tests {
    use super::*;

    fn save(path: &str, rgba: &[u8]) {
        let mut out = format!("P6 {SW} {SH} 255\n").into_bytes();
        for px in rgba.chunks(4) {
            out.extend_from_slice(&px[..3]);
        }
        std::fs::write(path, out).unwrap();
    }

    /// Runs `script` ("<steps>:<keys>;..." with keys l r u d j s c q e t)
    /// and returns the game.
    fn play(game: &mut Game, script: &str) {
        for cmd in script.split(';') {
            let (n, keys) = cmd.split_once(':').unwrap();
            let mut c = Controls::default();
            c.cat.left = keys.contains('l');
            c.cat.right = keys.contains('r');
            c.cat.up = keys.contains('u');
            c.cat.down = keys.contains('d');
            c.cat.shift = keys.contains('s');
            c.cat.ctrl = keys.contains('c');
            for i in 0..n.parse::<usize>().unwrap() {
                let first = i == 0;
                c.cat.jump_pressed = first && keys.contains('j');
                c.cat.drop_pressed = first && keys.contains('c');
                c.down_pressed = first && keys.contains('d');
                c.rotate = if first {
                    i32::from(keys.contains('e')) - i32::from(keys.contains('q'))
                } else {
                    0
                };
                c.toggle_top = first && keys.contains('t');
                game.step(&c, STEP as f32);
            }
        }
    }

    /// The Fez shortcut: trash bin -> charged jump onto the bus-stop roof ->
    /// walk right across to the billboard, which only lines up with the roof
    /// when looking along Z, and take the fish.
    #[test]
    fn fez_bridge_reaches_the_fish() {
        let mut game = Game::new();
        play(&mut game, "30:;46:r;1:j;40:;84:s;1:j;20:;30:r;40:");
        assert_eq!(
            game.cat_pos().y,
            world::BRIDGE_TOP as f32,
            "on the bus-stop roof"
        );
        play(&mut game, "220:r;30:");
        assert!(game.cat_pos().z < 40.0, "depth snapped onto the billboard");
        assert!(game.fish_taken(), "fish collected");
    }

    /// From the side the billboard is far away in depth: walking off the
    /// roof's side falls down instead.
    #[test]
    fn bridge_is_not_connected_from_the_side() {
        let mut game = Game::new();
        play(
            &mut game,
            "30:;46:r;1:j;40:;84:s;1:j;20:;30:r;40:;1:e;60:;240:l;60:",
        );
        assert!(
            game.cat_pos().y < world::BRIDGE_TOP as f32,
            "fell off the roof"
        );
        assert!(!game.fish_taken());
    }

    /// Mashing every key in every view never leaves the cat inside a wall.
    #[test]
    fn cat_never_ends_up_inside_solid() {
        let mut game = Game::new();
        let mut rng = util::Rng::new(7);
        let keys = [
            "l", "r", "u", "d", "j", "s", "c", "lj", "rj", "rs", "ls", "e", "q", "t", "rsj", "",
        ];
        for _ in 0..400 {
            let k = keys[rng.below(keys.len() as u32) as usize];
            play(&mut game, &format!("{}:{k}", 5 + rng.below(40)));
            assert!(
                game.cat_is_free(),
                "cat stuck in solid at {:?}",
                game.cat_pos()
            );
        }
    }

    #[test]
    #[ignore]
    fn snapshots() {
        let dir = std::env::var("RN_SHOTS").unwrap_or_else(|_| "/tmp".into());
        let script = std::env::var("RN_SCRIPT").unwrap_or_else(|_| "60:;shot".into());
        let mut game = Game::new();
        let mut renderer = Renderer::new();
        let mut shot = 0;
        let t0 = std::time::Instant::now();
        for cmd in script.split(';') {
            if cmd == "bench" {
                renderer.min_times = [f64::MAX; 7];
                for _ in 0..40 {
                    renderer.render(&game.scene());
                }
                let names = [
                    "world",
                    "dynamic",
                    "water",
                    "composite",
                    "particles",
                    "bloom",
                    "finish",
                ];
                let total: f64 = renderer.min_times.iter().sum::<f64>() * 1000.0;
                eprintln!(
                    "bench (min of 40): {:?} total {total:?}",
                    names
                        .iter()
                        .zip(renderer.min_times.map(|t| (t * 1e5).round() / 100.0))
                        .collect::<Vec<_>>()
                );
                continue;
            }
            if cmd == "shot" {
                let t = std::time::Instant::now();
                renderer.render(&game.scene());
                eprintln!("render {:?}", t.elapsed());
                save(&format!("{dir}/shot{shot:02}.ppm"), &renderer.rgba);
                shot += 1;
                continue;
            }
            // "<steps>:<keys>" with keys from: l r u d j(ump) s(hift) c(trl) q e t
            let (n, keys) = cmd.split_once(':').unwrap();
            let mut c = Controls::default();
            c.cat.left = keys.contains('l');
            c.cat.right = keys.contains('r');
            c.cat.up = keys.contains('u');
            c.cat.down = keys.contains('d');
            c.cat.shift = keys.contains('s');
            c.cat.ctrl = keys.contains('c');
            for i in 0..n.parse::<usize>().unwrap() {
                let first = i == 0;
                c.cat.jump_pressed = first && keys.contains('j');
                c.cat.drop_pressed = first && keys.contains('c');
                c.down_pressed = first && keys.contains('d');
                c.rotate = if first {
                    i32::from(keys.contains('e')) - i32::from(keys.contains('q'))
                } else {
                    0
                };
                c.toggle_top = first && keys.contains('t');
                game.step(&c, STEP as f32);
            }
            eprintln!("{cmd}: {}", game.debug());
        }
        eprintln!("total {:?}", t0.elapsed());
    }
}
