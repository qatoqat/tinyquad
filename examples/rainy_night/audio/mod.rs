//! Procedural audio for the rainy night demo: rain, storm drain, thunder and a few
//! one-shots, synthesized in `synth.rs` and played through a tiny per-platform backend
//! (ALSA / waveOut / AudioQueue / WebAudio). No assets, no audio crates.
//!
//! Threading model: the game thread never touches the synth. It only drops the latest
//! [`Ambience`] and one-shot events into a small mailbox; whoever renders audio (backend
//! thread, or the browser calling `rn_audio_render`) empties the mailbox and then renders
//! with the synth lock held, once per buffer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

mod synth;

#[cfg(target_os = "linux")]
#[path = "alsa.rs"]
mod backend;
#[cfg(target_os = "windows")]
#[path = "winmm.rs"]
mod backend;
#[cfg(target_os = "macos")]
#[path = "audioqueue.rs"]
mod backend;
#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
mod backend;
#[cfg(not(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    target_arch = "wasm32"
)))]
#[path = "null.rs"]
mod backend;

/// Continuous ambience targets. The synth smooths every field (~70 ms), so they can be
/// set to new values every frame without clicks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ambience {
    /// 0..1 overall rain intensity (heavy rain at 1).
    pub rain: f32,
    /// 0..1 listener is under/near the bus-stop roof: pattering on a thin roof.
    pub roof: f32,
    /// 0..1 storm-drain gurgle + water pouring in (caller attenuates by distance).
    pub drain: f32,
    /// -1..1
    pub drain_pan: f32,
    /// 0..1 low-pass everything (cat underwater / inside).
    pub muffle: f32,
    /// 0..1
    pub master: f32,
}

#[derive(Clone, Copy)]
enum Event {
    Thunder(f32, f32),
    Splash(f32, f32),
    Drip(f32, f32),
    Step(f32, f32),
}

/// What the game thread writes and the audio side reads. Held only for a few instructions
/// on either side, so the game never waits for DSP.
struct Mailbox {
    ambience: Ambience,
    events: [Event; 32],
    len: usize,
}

static MAILBOX: Mutex<Mailbox> = Mutex::new(Mailbox {
    ambience: Ambience {
        rain: 0.0,
        roof: 0.0,
        drain: 0.0,
        drain_pan: 0.0,
        muffle: 0.0,
        master: 0.0,
    },
    events: [Event::Step(0.0, 0.0); 32],
    len: 0,
});

static SYNTH: Mutex<Option<synth::Synth>> = Mutex::new(None);

/// There is one synth, so only one backend may pull from it at a time.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// A poisoned lock only means another thread panicked mid-update; the data is plain
/// numbers, so keep going rather than propagate the panic.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// (Re)creates the synth for the given output rate. Called by backends before they start
/// pulling audio. Events queued while nothing was playing are dropped so they do not all
/// fire at once.
fn init(sample_rate: f32) {
    *lock(&SYNTH) = Some(synth::Synth::new(sample_rate));
    lock(&MAILBOX).len = 0;
}

/// Fills `out` (interleaved stereo) with the next frames. Called by backends.
fn render(out: &mut [f32]) {
    let mut synth = lock(&SYNTH);
    let Some(synth) = synth.as_mut() else {
        out.fill(0.0);
        return;
    };
    let (ambience, events, len) = {
        let mut mail = lock(&MAILBOX);
        let len = std::mem::take(&mut mail.len);
        (mail.ambience, mail.events, len)
    };
    synth.set_ambience(ambience);
    for event in &events[..len] {
        match *event {
            Event::Thunder(strength, distance) => synth.thunder(strength, distance),
            Event::Splash(size, pan) => synth.splash(size, pan),
            Event::Drip(volume, pan) => synth.drip(volume, pan),
            Event::Step(volume, pan) => synth.step(volume, pan),
        }
    }
    synth.render(out);
}

/// Clamps to 0..1; NaN becomes 0.
fn unit(x: f32) -> f32 {
    if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) }
}

/// Clamps to -1..1; NaN becomes 0.
fn pan(x: f32) -> f32 {
    if x.is_nan() { 0.0 } else { x.clamp(-1.0, 1.0) }
}

pub struct Audio {
    /// Keeps the output stream alive; dropping it stops playback.
    backend: Option<backend::Backend>,
}

impl Audio {
    /// Starts the platform backend. Never panics; on any failure audio is silently disabled.
    pub fn new() -> Audio {
        // A second live `Audio` gets no backend of its own; its calls still reach the synth.
        let backend = if RUNNING.swap(true, Ordering::AcqRel) {
            None
        } else {
            let backend = backend::start();
            RUNNING.store(backend.is_some(), Ordering::Release);
            backend
        };
        Audio { backend }
    }

    /// No backend: the caller pulls audio itself with [`Audio::render`] (offline bounce,
    /// tests, or feeding some other audio API).
    #[allow(dead_code)]
    pub fn offline(sample_rate: f32) -> Audio {
        init(sample_rate);
        Audio { backend: None }
    }

    /// Renders the next `out.len() / 2` interleaved stereo frames. Only meaningful for an
    /// [`Audio::offline`] instance (otherwise it would steal audio from the backend).
    #[allow(dead_code)]
    pub fn render(&mut self, out: &mut [f32]) {
        render(out);
    }

    /// Continuous ambience targets; called every frame (cheap: stores the targets; the
    /// synth smooths them, no zipper noise).
    pub fn set_ambience(&mut self, a: Ambience) {
        lock(&MAILBOX).ambience = Ambience {
            rain: unit(a.rain),
            roof: unit(a.roof),
            drain: unit(a.drain),
            drain_pan: pan(a.drain_pan),
            muffle: unit(a.muffle),
            master: unit(a.master),
        };
    }

    /// Thunder clap+rumble. strength 0..1, distance 0..1 (0 = overhead: sharp crack,
    /// 1 = far: soft low roll). The caller already delays it after the lightning flash.
    pub fn thunder(&mut self, strength: f32, distance: f32) {
        self.push(Event::Thunder(unit(strength), unit(distance)));
    }

    /// Something falls in water (cat, box). size 0..1, pan -1..1.
    pub fn splash(&mut self, size: f32, pan_: f32) {
        self.push(Event::Splash(unit(size), pan(pan_)));
    }

    /// A single heavy drip into a puddle (roof edge). volume 0..1, pan -1..1.
    pub fn drip(&mut self, volume: f32, pan_: f32) {
        self.push(Event::Drip(unit(volume), pan(pan_)));
    }

    /// Small soft footstep/land thud on wet ground. volume 0..1, pan -1..1.
    pub fn step(&mut self, volume: f32, pan_: f32) {
        self.push(Event::Step(unit(volume), pan(pan_)));
    }

    /// Queues a one-shot; if the queue is full (nothing is pulling audio) it is dropped.
    fn push(&mut self, event: Event) {
        let mut mail = lock(&MAILBOX);
        if mail.len < mail.events.len() {
            let i = mail.len;
            mail.events[i] = event;
            mail.len += 1;
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        // Stop the stream first (joins the render thread), then free the slot.
        if self.backend.take().is_some() {
            RUNNING.store(false, Ordering::Release);
        }
    }
}

impl Default for Audio {
    fn default() -> Audio {
        Audio::new()
    }
}
