//! Android build of the classic triangle example.
//!
//! Android apps built with tinyquad are shared libraries loaded by the Java
//! activity through `System.loadLibrary` (see `java/MainActivity.java`), so
//! this example has no `main` - it is declared as a `cdylib` in Cargo.toml
//! and exports the `quad_main` symbol the activity calls through JNI once
//! the rendering surface is ready.
//!
//! Build an installable APK with `extras/android/build_apk.sh`.

use tinyquad::*;

#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}

struct Stage {
    pipeline: Pipeline,
    bindings: Bindings,
    ctx: Box<dyn RenderingBackend>,
}

impl Stage {
    pub fn new() -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        #[rustfmt::skip]
        let vertices: [Vertex; 3] = [
            Vertex { pos : [ -0.5, -0.5 ], color: [1., 0., 0., 1.] },
            Vertex { pos : [  0.5, -0.5 ], color: [0., 1., 0., 1.] },
            Vertex { pos : [  0.0,  0.5 ], color: [0., 0., 1., 1.] },
        ];
        let vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&vertices),
        );

        let indices: [u16; 3] = [0, 1, 2];
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&indices),
        );

        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![],
        };

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

        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_color", VertexFormat::Float4),
            ],
            shader,
            PipelineParams::default(),
        );

        Stage {
            pipeline,
            bindings,
            ctx,
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        self.ctx.begin_default_pass(Default::default());

        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx.draw(0, 3, 1);
        self.ctx.end_render_pass();

        self.ctx.commit_frame();
    }
}

/// Called by tinyquad's Java activity (`MainActivity.onCreate` -> JNI) after
/// the app surface exists. Mirrors `src/native/android/mod_inject.rs`.
#[unsafe(no_mangle)]
pub extern "C" fn quad_main() {
    tinyquad::start(conf::Conf::default(), move || Box::new(Stage::new()));
}

mod shader {
    use tinyquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec2 in_pos;
    attribute vec4 in_color;

    varying lowp vec4 color;

    void main() {
        gl_Position = vec4(in_pos, 0, 1);
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
        float2 in_pos   [[attribute(0)]];
        float4 in_color [[attribute(1)]];
    };

    struct RasterizerData
    {
        float4 position [[position]];
        float4 color [[user(locn0)]];
    };

    vertex RasterizerData vertexShader(Vertex v [[stage_in]])
    {
        RasterizerData out;

        out.position = float4(v.in_pos.xy, 0.0, 1.0);
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
            uniforms: UniformBlockLayout { uniforms: vec![] },
        }
    }
}
