//! Tests for context-creation configuration defaults.
//!
//! These run without a window or GPU context: [`tinyquad::conf`] types are plain data.

use tinyquad::conf::*;

#[test]
fn conf_desktop_defaults() {
    let conf = Conf::default();

    assert_eq!(conf.window_title, "");
    assert_eq!(conf.window_width, 800);
    assert_eq!(conf.window_height, 600);
    assert!(!conf.high_dpi);
    assert!(!conf.fullscreen);
    assert_eq!(conf.sample_count, 1);
    assert!(conf.window_resizable);
    assert!(conf.icon.is_some());
}

#[test]
fn platform_defaults() {
    let platform = Platform::default();

    assert_eq!(platform.linux_x11_gl, LinuxX11Gl::GLXWithEGLFallback);
    assert_eq!(platform.linux_backend, LinuxBackend::X11Only);
    assert_eq!(platform.webgl_version, WebGLVersion::WebGL1);
    assert_eq!(platform.apple_gfx_api, AppleGfxApi::OpenGl);
    assert_eq!(platform.swap_interval, None);
    assert_eq!(platform.sleep_interval_ms, None);
    assert!(!platform.blocking_event_loop);
    assert!(!platform.framebuffer_alpha);
    assert_eq!(
        platform.wayland_decorations,
        WaylandDecorations::ServerWithLibDecorFallback
    );
    assert_eq!(platform.linux_wm_class, "tinyquad-application");
    assert!(platform.android_panic_hook);
}

#[test]
fn platform_enums_derive_default_from_first_documented_variant() {
    // The #[default] attribute documents which variant each enum resolves to;
    // a change here is a user-visible behavior change of Conf::default().
    assert_eq!(LinuxX11Gl::default(), LinuxX11Gl::GLXWithEGLFallback);
    assert_eq!(LinuxBackend::default(), LinuxBackend::X11Only);
    assert_eq!(AppleGfxApi::default(), AppleGfxApi::OpenGl);
    assert_eq!(WebGLVersion::default(), WebGLVersion::WebGL1);
    assert_eq!(
        WaylandDecorations::default(),
        WaylandDecorations::ServerWithLibDecorFallback
    );
}

#[test]
fn tinyquad_logo_icon_has_documented_dimensions() {
    let icon = Icon::tinyquad_logo();

    assert_eq!(icon.small.len(), 16 * 16 * 4);
    assert_eq!(icon.medium.len(), 32 * 32 * 4);
    assert_eq!(icon.big.len(), 64 * 64 * 4);
}
