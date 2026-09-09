//! Tests for the pure data types of the graphics API: layouts, formats and defaults.
//!
//! None of these need a rendering context, so they run on any target with a test harness.

// Everything in tinyquad::graphics is re-exported at the crate root.
use tinyquad::*;

#[test]
fn uniform_type_byte_size() {
    assert_eq!(UniformType::Float1.size(), 4);
    assert_eq!(UniformType::Float2.size(), 8);
    assert_eq!(UniformType::Float3.size(), 12);
    assert_eq!(UniformType::Float4.size(), 16);
    assert_eq!(UniformType::Int1.size(), 4);
    assert_eq!(UniformType::Int2.size(), 8);
    assert_eq!(UniformType::Int3.size(), 12);
    assert_eq!(UniformType::Int4.size(), 16);
    assert_eq!(UniformType::Mat4.size(), 64);
}

#[test]
fn uniform_desc_defaults_to_single_element() {
    let desc = UniformDesc::new("time", UniformType::Float1);
    assert_eq!(desc.name, "time");
    assert_eq!(desc.array_count, 1);

    let array = UniformDesc::new("lights", UniformType::Float4).array(8);
    assert_eq!(array.name, "lights");
    assert_eq!(array.uniform_type, UniformType::Float4);
    assert_eq!(array.array_count, 8);
}

#[test]
fn vertex_format_components_and_bytes() {
    assert_eq!(VertexFormat::Float1.components(), 1);
    assert_eq!(VertexFormat::Float4.components(), 4);
    assert_eq!(VertexFormat::Mat4.components(), 16);

    assert_eq!(VertexFormat::Float3.size_bytes(), 12);
    assert_eq!(VertexFormat::Byte4.size_bytes(), 4);
    assert_eq!(VertexFormat::Short2.size_bytes(), 4);
    assert_eq!(VertexFormat::Int4.size_bytes(), 16);
    assert_eq!(VertexFormat::Mat4.size_bytes(), 64);
}

#[test]
fn vertex_attribute_defaults_to_interleaved_float_passing() {
    let attr = VertexAttribute::new("position", VertexFormat::Float3);
    assert_eq!(attr.buffer_index, 0);
    assert!(attr.gl_pass_as_float);

    let attr = VertexAttribute::with_buffer("color", VertexFormat::Byte4, 1);
    assert_eq!(attr.buffer_index, 1);
}

#[test]
fn buffer_layout_default_is_per_vertex_with_auto_stride() {
    let layout = BufferLayout::default();
    assert_eq!(layout.stride, 0);
    assert_eq!(layout.step_func, VertexStep::PerVertex);
    assert_eq!(layout.step_rate, 1);
}

#[test]
fn texture_format_size_matches_bytes_per_pixel() {
    assert_eq!(TextureFormat::RGB8.size(4, 4), 3 * 16);
    assert_eq!(TextureFormat::RGBA8.size(4, 4), 4 * 16);
    assert_eq!(TextureFormat::RGBA16F.size(4, 4), 8 * 16);
    assert_eq!(TextureFormat::Depth.size(4, 4), 2 * 16);
    assert_eq!(TextureFormat::Depth32.size(4, 4), 4 * 16);
    assert_eq!(TextureFormat::Alpha.size(4, 4), 16);
}

#[test]
fn texture_params_default() {
    let params = TextureParams::default();
    assert_eq!(params.kind, TextureKind::Texture2D);
    assert_eq!(params.format, TextureFormat::RGBA8);
    assert_eq!(params.wrap, TextureWrap::Clamp);
    assert_eq!(params.min_filter, FilterMode::Linear);
    assert_eq!(params.mag_filter, FilterMode::Linear);
    assert_eq!(params.mipmap_filter, MipmapFilterMode::None);
    assert_eq!(params.width, 0);
    assert_eq!(params.height, 0);
    assert!(!params.allocate_mipmaps);
    assert_eq!(params.sample_count, 1);
}

#[test]
fn pipeline_params_default_is_unlit_triangle_rendering() {
    let params = PipelineParams::default();
    assert_eq!(params.cull_face, CullFace::Nothing);
    assert_eq!(params.front_face_order, FrontFaceOrder::CounterClockwise);
    assert_eq!(params.depth_test, Comparison::Always);
    assert!(!params.depth_write);
    assert_eq!(params.depth_write_offset, None);
    assert_eq!(params.color_blend, None);
    assert_eq!(params.alpha_blend, None);
    assert_eq!(params.stencil_test, None);
    assert_eq!(params.color_write, (true, true, true, true));
    assert_eq!(params.primitive_type, PrimitiveType::Triangles);
}

#[test]
fn pass_action_defaults_to_black_clear() {
    match PassAction::default() {
        PassAction::Clear {
            color,
            depth,
            stencil,
        } => {
            assert_eq!(color, Some((0.0, 0.0, 0.0, 0.0)));
            assert_eq!(depth, Some(1.0));
            assert_eq!(stencil, None);
        }
        PassAction::Nothing => panic!("default PassAction should clear"),
    }
}

#[test]
fn pass_action_clear_color_only_sets_color_and_depth() {
    match PassAction::clear_color(0.25, 0.5, 0.75, 1.0) {
        PassAction::Clear {
            color,
            depth,
            stencil,
        } => {
            assert_eq!(color, Some((0.25, 0.5, 0.75, 1.0)));
            assert_eq!(depth, Some(1.0));
            assert_eq!(stencil, None);
        }
        PassAction::Nothing => panic!("clear_color should clear"),
    }
}

#[test]
fn blend_state_new_stores_the_alpha_blending_recipe() {
    let state = BlendState::new(
        Equation::Add,
        BlendFactor::Value(BlendValue::SourceAlpha),
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    );
    // The struct fields are private; equality is the observable contract.
    assert_eq!(
        state,
        BlendState::new(
            Equation::Add,
            BlendFactor::Value(BlendValue::SourceAlpha),
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha)
        )
    );
    assert_ne!(
        state,
        BlendState::new(
            Equation::ReverseSubtract,
            BlendFactor::Zero,
            BlendFactor::One
        )
    );
}

#[test]
fn features_default_assumes_desktop_gl_capabilities() {
    let features = Features::default();
    assert!(features.instancing);
    assert!(features.resolve_attachments);
}

#[test]
fn resource_limit_constants() {
    assert_eq!(MAX_VERTEX_ATTRIBUTES, 16);
    assert_eq!(MAX_SHADERSTAGE_IMAGES, 12);
}

#[test]
fn shader_error_display_is_human_readable() {
    let err = ShaderError::LinkError("undefined main".to_string());
    let msg = err.to_string();
    assert!(msg.contains("Link shader error"), "unexpected: {msg}");
    assert!(msg.contains("undefined main"), "unexpected: {msg}");
}

#[test]
fn shader_type_display() {
    assert_eq!(ShaderType::Vertex.to_string(), "Vertex");
    assert_eq!(ShaderType::Fragment.to_string(), "Fragment");
}
