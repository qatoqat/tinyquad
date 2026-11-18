#![doc = include_str!("../README.md")]
#![expect(
    clippy::missing_safety_doc,
    reason = "most public `unsafe fn`s are thin wrappers over raw platform/GL bindings"
)]

pub mod conf;
mod event;
pub mod fs;
pub mod graphics;
pub mod native;
use std::ops::{Index, IndexMut};

#[cfg(feature = "log-impl")]
pub mod log;

pub use event::*;

pub use graphics::*;

mod default_icon;

pub use native::gl;

/// Storage for backend resources (buffers, shaders, pipelines, render
/// passes), addressed by the opaque `usize` inside the public id types.
///
/// This is a generational slab: values live in a `Vec`, so the lookups done
/// on every bind/draw are a bounds check plus a generation compare instead
/// of a `HashMap` probe, and slots of removed resources are recycled so the
/// table stays as small as the peak number of live resources.
///
/// An id packs the slot index (low bits) together with the generation of
/// the slot at allocation time (high bits). Removing a resource bumps the
/// slot's generation, so an id is never handed out twice and a stale id
/// keeps panicking exactly like a missing `HashMap` key did - it can never
/// silently alias a newer resource. A slot whose generation counter is
/// exhausted is retired instead of wrapping around. Resources that are never
/// removed get the ids 0, 1, 2, .. as before.
#[derive(Clone)]
pub(crate) struct ResourceManager<T> {
    slots: Vec<Slot<T>>,
    /// Indices of vacant slots that can be handed out again.
    free: Vec<usize>,
}

#[derive(Clone)]
struct Slot<T> {
    generation: usize,
    resource: Option<T>,
}

impl<T> ResourceManager<T> {
    /// Bits of an id holding the slot index: 4 billion live resources on
    /// 64-bit targets, 4 million (and 1024 generations per slot) on 32-bit
    /// ones such as wasm32.
    const INDEX_BITS: u32 = if usize::BITS >= 64 { 32 } else { 22 };
    const INDEX_MASK: usize = (1 << Self::INDEX_BITS) - 1;
    const MAX_GENERATION: usize = usize::MAX >> Self::INDEX_BITS;

    pub fn add(&mut self, resource: T) -> usize {
        let index = self.free.pop().unwrap_or_else(|| {
            assert!(
                self.slots.len() <= Self::INDEX_MASK,
                "too many live resources"
            );
            self.slots.push(Slot {
                generation: 0,
                resource: None,
            });
            self.slots.len() - 1
        });
        let slot = &mut self.slots[index];
        slot.resource = Some(resource);
        index | (slot.generation << Self::INDEX_BITS)
    }

    pub fn remove(&mut self, id: usize) -> T {
        let index = id & Self::INDEX_MASK;
        let slot = self
            .slots
            .get_mut(index)
            .filter(|slot| slot.generation == id >> Self::INDEX_BITS);
        // Let it crash if the resource is not found
        let slot = slot.unwrap_or_else(|| Self::missing(id));
        let resource = slot.resource.take().unwrap_or_else(|| Self::missing(id));
        if slot.generation < Self::MAX_GENERATION {
            slot.generation += 1;
            self.free.push(index);
        }
        resource
    }

    #[inline]
    fn get(&self, id: usize) -> Option<&T> {
        let slot = self.slots.get(id & Self::INDEX_MASK)?;
        if slot.generation == id >> Self::INDEX_BITS {
            slot.resource.as_ref()
        } else {
            None
        }
    }

    #[inline]
    fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        let slot = self.slots.get_mut(id & Self::INDEX_MASK)?;
        if slot.generation == id >> Self::INDEX_BITS {
            slot.resource.as_mut()
        } else {
            None
        }
    }

    #[cold]
    #[inline(never)]
    fn missing(id: usize) -> ! {
        panic!("resource {id:#x} does not exist (never created or already deleted)")
    }
}

impl<T> Default for ResourceManager<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Index<usize> for ResourceManager<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: usize) -> &Self::Output {
        self.get(id).unwrap_or_else(|| Self::missing(id))
    }
}

impl<T> IndexMut<usize> for ResourceManager<T> {
    #[inline]
    fn index_mut(&mut self, id: usize) -> &mut Self::Output {
        self.get_mut(id).unwrap_or_else(|| Self::missing(id))
    }
}

#[cfg(test)]
mod resource_manager_tests {
    use super::ResourceManager;

    type Rm = ResourceManager<&'static str>;

    #[test]
    fn ids_are_sequential_without_removals() {
        let mut rm = Rm::default();
        assert_eq!([rm.add("a"), rm.add("b"), rm.add("c")], [0, 1, 2]);
        assert_eq!((rm[0], rm[1], rm[2]), ("a", "b", "c"));
        rm[1] = "B";
        assert_eq!(rm[1], "B");
    }

    #[test]
    fn slots_are_recycled_but_ids_are_not() {
        let mut rm = Rm::default();
        let a = rm.add("a");
        let b = rm.add("b");
        assert_eq!(rm.remove(a), "a");

        let c = rm.add("c");
        assert_ne!(c, a, "an id must never be handed out twice");
        assert_eq!(c & Rm::INDEX_MASK, a, "the vacant slot is reused");
        assert_eq!(rm.slots.len(), 2);
        assert_eq!((rm[b], rm[c]), ("b", "c"));
        assert!(rm.get(a).is_none(), "stale id must not alias the new one");
    }

    #[test]
    #[should_panic(expected = "does not exist")]
    fn stale_id_panics_on_index() {
        let mut rm = Rm::default();
        let a = rm.add("a");
        rm.remove(a);
        rm.add("b");
        let _ = rm[a];
    }

    #[test]
    #[should_panic(expected = "does not exist")]
    fn double_remove_panics() {
        let mut rm = Rm::default();
        let a = rm.add("a");
        rm.remove(a);
        rm.remove(a);
    }

    #[test]
    #[should_panic(expected = "does not exist")]
    fn unknown_id_panics() {
        let _ = Rm::default()[7];
    }

    #[test]
    fn exhausted_slot_is_retired() {
        let mut rm = Rm::default();
        let a = rm.add("a");
        rm.slots[0].generation = Rm::MAX_GENERATION;
        let last = a | (Rm::MAX_GENERATION << Rm::INDEX_BITS);
        assert_eq!(rm.remove(last), "a");
        // The slot cannot get a fresh generation, so it is never reused.
        assert_eq!(rm.add("b") & Rm::INDEX_MASK, 1);
        assert!(rm.get(last).is_none());
    }
}

pub mod date {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn now() -> f64 {
        use std::time::SystemTime;

        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|e| panic!("{e}"));
        time.as_secs_f64()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn now() -> f64 {
        use crate::native;

        unsafe { native::wasm::now() }
    }
}

pub type Context = dyn RenderingBackend;

use std::sync::{Mutex, OnceLock};

static NATIVE_DISPLAY: OnceLock<Mutex<native::NativeDisplayData>> = OnceLock::new();

fn set_display(display: native::NativeDisplayData) {
    NATIVE_DISPLAY
        .set(Mutex::new(display))
        .unwrap_or_else(|_| panic!("NATIVE_DISPLAY already set"));
}
/// This for now is Android specific since the process can continue running but the display
/// is restarted. We support reinitializing the display.
#[cfg(target_os = "android")]
fn set_or_replace_display(display: native::NativeDisplayData) {
    if let Some(m) = NATIVE_DISPLAY.get() {
        // Replace existing display
        *m.lock().unwrap() = display;
    } else {
        // First time initialization
        set_display(display);
    }
}
fn native_display() -> &'static Mutex<native::NativeDisplayData> {
    NATIVE_DISPLAY
        .get()
        .expect("Backend has not initialized NATIVE_DISPLAY yet.")
}

/// Window and associated to window rendering context related functions.
/// in macroquad <= 0.3, it was ctx.screen_size(). Now it is window::screen_size()
pub mod window {
    use super::*;

    /// The same as
    /// ```ignore
    /// if metal {
    ///    Box::new(MetalContext::new())
    /// } else {
    ///   Box::new(GlContext::new())
    /// };
    /// ```
    /// but under #[cfg] gate to avoid MetalContext on non-apple platforms
    pub fn new_rendering_backend() -> Box<dyn RenderingBackend> {
        #[cfg(target_vendor = "apple")]
        {
            if window::apple_gfx_api() == conf::AppleGfxApi::Metal {
                Box::new(MetalContext::new())
            } else {
                Box::new(GlContext::new())
            }
        }
        #[cfg(not(target_vendor = "apple"))]
        Box::new(GlContext::new())
    }

    /// The current framebuffer size in pixels
    /// NOTE: [High DPI Rendering](../conf/index.html#high-dpi-rendering)
    pub fn screen_size() -> (f32, f32) {
        let d = native_display().lock().unwrap();
        (d.screen_width as f32, d.screen_height as f32)
    }

    /// The dpi scaling factor (window pixels to framebuffer pixels)
    /// NOTE: [High DPI Rendering](../conf/index.html#high-dpi-rendering)
    pub fn dpi_scale() -> f32 {
        let d = native_display().lock().unwrap();
        d.dpi_scale
    }

    /// True when high_dpi was requested and actually running in a high-dpi scenario
    /// NOTE: [High DPI Rendering](../conf/index.html#high-dpi-rendering)
    pub fn high_dpi() -> bool {
        let d = native_display().lock().unwrap();
        d.high_dpi
    }

    pub fn blocking_event_loop() -> bool {
        let d = native_display().lock().unwrap();
        d.blocking_event_loop
    }

    /// This function simply quits the application without
    /// giving the user a chance to intervene. Usually this might
    /// be called when the user clicks the 'Ok' button in a 'Really Quit?'
    /// dialog box
    /// Window might not be actually closed right away (exit(0) might not
    /// happen in the order_quit implmentation) and execution might continue for some time after
    /// But the window is going to be inevitably closed at some point.
    pub fn order_quit() {
        let mut d = native_display().lock().unwrap();
        d.quit_ordered = true;
    }

    /// Shortcut for `order_quit`. Will add a legacy attribute at some point.
    pub fn quit() {
        order_quit()
    }

    /// Calling request_quit() will trigger "quit_requested_event" event , giving
    /// the user code a chance to intervene and cancel the pending quit process
    /// (for instance to show a 'Really Quit?' dialog box).
    /// If the event handler callback does nothing, the application will be quit as usual.
    /// To prevent this, call the function "cancel_quit()"" from inside the event handler.
    pub fn request_quit() {
        let mut d = native_display().lock().unwrap();
        d.quit_requested = true;
    }

    /// Cancels a pending quit request, either initiated
    /// by the user clicking the window close button, or programmatically
    /// by calling "request_quit()". The only place where calling this
    /// function makes sense is from inside the event handler callback when
    /// the "quit_requested_event" event has been received
    pub fn cancel_quit() {
        let mut d = native_display().lock().unwrap();
        d.quit_requested = false;
    }
    /// Capture mouse cursor to the current window
    /// On WASM this will automatically hide cursor
    /// On desktop this will bound cursor to windows border
    /// NOTICE: on desktop cursor will not be automatically released after window lost focus
    ///         so set_cursor_grab(false) on window's focus lost is recommended.
    /// TODO: implement window focus events
    pub fn set_cursor_grab(grab: bool) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::SetCursorGrab(grab));
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetCursorGrab(grab))
                .unwrap();
        }
    }

    /// With `conf.platform.blocking_event_loop`, `schedule_update` called from an
    /// event handler makes draw()/update() functions to be called without waiting
    /// for a next event.
    ///
    /// Does nothing without `conf.platform.blocking_event_loop`.
    pub fn schedule_update() {
        #[cfg(all(target_os = "android", not(target_arch = "wasm32")))]
        {
            let d = native_display().lock().unwrap();
            (d.native_requests)(native::Request::ScheduleUpdate);
        }

        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        {
            let d = native_display().lock().unwrap();
            d.native_requests
                .send(native::Request::ScheduleUpdate)
                .unwrap();
        }

        #[cfg(target_arch = "wasm32")]
        unsafe {
            native::wasm::sapp_schedule_update();
        }
    }

    /// Show or hide the mouse cursor
    pub fn show_mouse(shown: bool) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::ShowMouse(shown));
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::ShowMouse(shown))
                .unwrap();
        }
    }

    /// Set the mouse cursor icon.
    pub fn set_mouse_cursor(cursor_icon: CursorIcon) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::SetMouseCursor(cursor_icon));
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetMouseCursor(cursor_icon))
                .unwrap();
        }
    }

    /// Set the application's window size.
    pub fn set_window_size(new_width: u32, new_height: u32) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::SetWindowSize {
                new_width,
                new_height,
            });
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetWindowSize {
                    new_width,
                    new_height,
                })
                .unwrap();
        }
    }

    pub fn set_window_position(new_x: u32, new_y: u32) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::SetWindowPosition { new_x, new_y });
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetWindowPosition { new_x, new_y })
                .unwrap();
        }
    }

    /// Get the position of the window.
    /// TODO: implement for other platforms
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn get_window_position() -> (u32, u32) {
        let d = native_display().lock().unwrap();
        d.screen_position
    }

    pub fn set_fullscreen(fullscreen: bool) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::SetFullscreen(fullscreen));
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetFullscreen(fullscreen))
                .unwrap();
        }
    }

    /// Get current OS clipboard value
    pub fn clipboard_get() -> Option<String> {
        let mut d = native_display().lock().unwrap();
        d.clipboard.get()
    }

    /// Save value to OS clipboard
    pub fn clipboard_set(data: &str) {
        let mut d = native_display().lock().unwrap();
        d.clipboard.set(data)
    }
    pub fn dropped_file_count() -> usize {
        let d = native_display().lock().unwrap();
        d.dropped_files.bytes.len()
    }
    pub fn dropped_file_bytes(index: usize) -> Option<Vec<u8>> {
        let d = native_display().lock().unwrap();
        d.dropped_files.bytes.get(index).cloned()
    }
    pub fn dropped_file_path(index: usize) -> Option<std::path::PathBuf> {
        let d = native_display().lock().unwrap();
        d.dropped_files.paths.get(index).cloned()
    }

    /// Show/hide onscreen keyboard.
    /// Works on Android and iOS.
    pub fn show_keyboard(show: bool) {
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            (d.native_requests)(native::Request::ShowKeyboard(show));
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::ShowKeyboard(show))
                .unwrap();
        }
    }

    /// Set the position of the IME candidate window.
    /// The position is in window client coordinates (pixels).
    /// This should be called when the text cursor moves to keep the IME
    /// candidate window near the insertion point.
    pub fn set_ime_position(x: i32, y: i32) {
        // `d` is unused on Android, where the IME request is not sent.
        #[cfg_attr(target_os = "android", allow(unused_variables))]
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            let _ = (x, y); // IME position not applicable on Android
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetImePosition { x, y })
                .unwrap();
        }
    }

    /// Enable or disable IME (Input Method Editor) for the window.
    /// When enabled, the IME will process keyboard input for CJK text input.
    /// When disabled, keyboard events are sent directly to the application,
    /// which is useful for game controls (e.g., WASD movement).
    ///
    /// # Arguments
    /// * `enabled` - `true` to enable IME (for text input), `false` to disable (for game controls)
    pub fn set_ime_enabled(enabled: bool) {
        // `d` is unused on Android, where the IME request is not sent.
        #[cfg_attr(target_os = "android", allow(unused_variables))]
        let d = native_display().lock().unwrap();
        #[cfg(target_os = "android")]
        {
            let _ = enabled; // IME control not applicable on Android
        }

        #[cfg(not(target_os = "android"))]
        {
            d.native_requests
                .send(native::Request::SetImeEnabled(enabled))
                .unwrap();
        }
    }

    #[cfg(target_vendor = "apple")]
    pub fn apple_gfx_api() -> crate::conf::AppleGfxApi {
        let d = native_display().lock().unwrap();
        d.gfx_api
    }
    #[cfg(target_vendor = "apple")]
    pub fn apple_view() -> crate::native::apple::frameworks::ObjcId {
        let d = native_display().lock().unwrap();
        d.view
    }
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    pub fn apple_view_ctrl() -> crate::native::apple::frameworks::ObjcId {
        let d = native_display().lock().unwrap();
        d.view_ctrl
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq)]
pub enum CursorIcon {
    Default,
    Help,
    Pointer,
    Wait,
    Crosshair,
    Text,
    Move,
    NotAllowed,
    EWResize,
    NSResize,
    NESWResize,
    NWSEResize,
}

/// Start tinyquad.
pub fn start<F>(conf: conf::Conf, f: F)
where
    F: 'static + FnOnce() -> Box<dyn EventHandler>,
{
    #[cfg(target_os = "linux")]
    {
        let mut f = Some(f);
        let f = &mut f;
        match conf.platform.linux_backend {
            conf::LinuxBackend::X11Only => {
                native::linux_x11::run(&conf, f).expect("X11 backend failed")
            }
            conf::LinuxBackend::WaylandOnly => {
                native::linux_wayland::run(&conf, f).expect("Wayland backend failed")
            }
            conf::LinuxBackend::X11WithWaylandFallback => {
                if let Err(err) = native::linux_x11::run(&conf, f) {
                    eprintln!("{err:?}");
                    eprintln!("Failed to initialize through X11! Trying wayland instead");
                    native::linux_wayland::run(&conf, f);
                }
            }
            conf::LinuxBackend::WaylandWithX11Fallback => {
                if native::linux_wayland::run(&conf, f).is_none() {
                    eprintln!("Failed to initialize through wayland! Trying X11 instead");
                    native::linux_x11::run(&conf, f).unwrap()
                }
            }
        }
    }

    #[cfg(target_os = "android")]
    unsafe {
        native::android::run(conf, f);
    }

    #[cfg(target_arch = "wasm32")]
    {
        native::wasm::run(&conf, f);
    }

    #[cfg(target_os = "windows")]
    {
        native::windows::run(&conf, f);
    }

    #[cfg(target_os = "macos")]
    unsafe {
        native::macos::run(conf, f);
    }

    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    unsafe {
        native::ios::run(conf, f);
    }
}
