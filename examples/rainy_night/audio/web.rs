//! wasm32: the browser drives us. `web/audio.js` (a tinyquad JS plugin) creates the
//! AudioContext on the first user gesture, tells us its sample rate with `rn_audio_init`
//! and then pulls interleaved stereo frames with `rn_audio_render` from its audio callback.

use std::sync::Mutex;

/// Must match `version` in web/audio.js (gl.js compares them at startup).
const PLUGIN_VERSION: u32 = 1;
/// Largest block the JS side may ask for (ScriptProcessorNode tops out at 16384 frames).
const MAX_FRAMES: usize = 16384;

static OUT: Mutex<[f32; 2 * MAX_FRAMES]> = Mutex::new([0.0; 2 * MAX_FRAMES]);

pub struct Backend;

pub fn start() -> Option<Backend> {
    // Nothing to do until audio.js calls `rn_audio_init` after a user gesture.
    Some(Backend)
}

#[unsafe(no_mangle)]
pub extern "C" fn rn_audio_crate_version() -> u32 {
    PLUGIN_VERSION
}

/// Called by audio.js once the AudioContext exists (again if it is ever recreated).
#[unsafe(no_mangle)]
pub extern "C" fn rn_audio_init(sample_rate: f32) {
    super::init(sample_rate);
}

/// Renders `frames` (clamped to `MAX_FRAMES`) and returns a pointer to that many
/// interleaved stereo f32 frames inside wasm memory, valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn rn_audio_render(frames: u32) -> *const f32 {
    let mut out = super::lock(&OUT);
    let frames = (frames as usize).min(MAX_FRAMES);
    super::render(&mut out[..2 * frames]);
    // The buffer is a static, so the pointer stays valid after the guard is released.
    out.as_ptr()
}
