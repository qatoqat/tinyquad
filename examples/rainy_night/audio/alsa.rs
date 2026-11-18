//! Linux: ALSA, loaded at runtime with `dlopen` so the binary neither links against
//! libasound nor fails to start on machines without it. A thread renders ~10 ms blocks and
//! hands them to the blocking `snd_pcm_writei`; ALSA's own buffer (~50 ms) is the latency.

use std::ffi::{CStr, c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

const SAMPLE_RATE: u32 = 48000;
const LATENCY_US: c_uint = 50_000;
/// Frames rendered per write (10.7 ms).
const BLOCK: usize = 512;

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_S16_LE: c_int = 2;
const SND_PCM_FORMAT_FLOAT_LE: c_int = 14;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;

type ErrorHandler = unsafe extern "C" fn(*const c_char, c_int, *const c_char, c_int, *const c_char);

/// The handful of libasound entry points we need.
struct Alsa {
    open: unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int,
    set_params:
        unsafe extern "C" fn(*mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int,
    writei: unsafe extern "C" fn(*mut c_void, *const c_void, c_ulong) -> c_long,
    recover: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int,
    drop: unsafe extern "C" fn(*mut c_void) -> c_int,
    close: unsafe extern "C" fn(*mut c_void) -> c_int,
    set_error_handler: unsafe extern "C" fn(Option<ErrorHandler>) -> c_int,
}

impl Alsa {
    fn load() -> Option<Alsa> {
        // SAFETY: plain dlopen/dlsym; every symbol is checked for null before it is
        // turned into a function pointer with the signature from <alsa/asoundlib.h>.
        unsafe {
            let mut lib = libc::dlopen(c"libasound.so.2".as_ptr(), libc::RTLD_NOW);
            if lib.is_null() {
                lib = libc::dlopen(c"libasound.so".as_ptr(), libc::RTLD_NOW);
            }
            if lib.is_null() {
                return None;
            }
            macro_rules! sym {
                ($name:literal) => {
                    symbol(lib, $name)?
                };
            }
            Some(Alsa {
                open: sym!(c"snd_pcm_open"),
                set_params: sym!(c"snd_pcm_set_params"),
                writei: sym!(c"snd_pcm_writei"),
                recover: sym!(c"snd_pcm_recover"),
                drop: sym!(c"snd_pcm_drop"),
                close: sym!(c"snd_pcm_close"),
                set_error_handler: sym!(c"snd_lib_error_set_handler"),
            })
        }
    }
}

/// Looks up `name` and reinterprets it as the function pointer type `F`.
///
/// # Safety
/// `F` must be the `extern "C" fn` type matching the C declaration of the symbol.
unsafe fn symbol<F: Copy>(lib: *mut c_void, name: &CStr) -> Option<F> {
    let ptr = unsafe { libc::dlsym(lib, name.as_ptr()) };
    const { assert!(size_of::<F>() == size_of::<*mut c_void>()) };
    // SAFETY: `F` is pointer-sized (checked above) and, per the contract, a fn pointer.
    (!ptr.is_null()).then(|| unsafe { std::mem::transmute_copy(&ptr) })
}

/// Replaces ALSA's default handler, which prints to stderr when e.g. there is no sound
/// card. (The real handler type is variadic; the extra printf arguments are simply ignored,
/// which is fine in the C calling conventions of every Linux target.)
unsafe extern "C" fn quiet(
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
) {
}

pub struct Backend {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

pub fn start() -> Option<Backend> {
    let alsa = Alsa::load()?;
    super::init(SAMPLE_RATE as f32);
    let stop = Arc::new(AtomicBool::new(false));
    let thread = std::thread::Builder::new()
        .name("rn-audio".into())
        .spawn({
            let stop = stop.clone();
            // Opening the device happens on the thread too: with PulseAudio/PipeWire
            // behind "default" it can take a while and must not stall the first frame.
            move || run(&alsa, &stop, c"default")
        })
        .ok()?;
    Some(Backend {
        stop,
        thread: Some(thread),
    })
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(alsa: &Alsa, stop: &AtomicBool, device: &CStr) {
    // SAFETY: by-the-book use of the ALSA PCM API; `pcm` is only used between a successful
    // open and the close below, and buffers outlive the calls that read them.
    unsafe {
        (alsa.set_error_handler)(Some(quiet));
        let mut pcm = std::ptr::null_mut();
        if (alsa.open)(&mut pcm, device.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0) < 0 {
            return;
        }
        // Prefer float; fall back to 16-bit for plugins/devices that refuse it. Stereo,
        // 48 kHz, let alsa-lib resample if the hardware runs at another rate.
        let configure = |format| {
            (alsa.set_params)(
                pcm,
                format,
                SND_PCM_ACCESS_RW_INTERLEAVED,
                2,
                SAMPLE_RATE,
                1,
                LATENCY_US,
            ) >= 0
        };
        let float = configure(SND_PCM_FORMAT_FLOAT_LE);
        if float || configure(SND_PCM_FORMAT_S16_LE) {
            let mut mix = [0.0f32; 2 * BLOCK];
            let mut pcm16 = [0i16; 2 * BLOCK];
            'stream: while !stop.load(Ordering::Relaxed) {
                super::render(&mut mix);
                let data: *const c_void = if float {
                    mix.as_ptr().cast()
                } else {
                    for (o, s) in pcm16.iter_mut().zip(&mix) {
                        *o = (s * 32767.0) as i16;
                    }
                    pcm16.as_ptr().cast()
                };
                let mut done = 0;
                while done < BLOCK {
                    let frame_bytes = if float { 8 } else { 4 };
                    let n = (alsa.writei)(
                        pcm,
                        data.byte_add(done * frame_bytes),
                        (BLOCK - done) as c_ulong,
                    );
                    if n >= 0 {
                        done += n as usize;
                    } else if (alsa.recover)(pcm, n as c_int, 1) < 0 {
                        // Not an underrun/suspend we can recover from (device unplugged...).
                        break 'stream;
                    }
                }
            }
            (alsa.drop)(pcm);
        }
        (alsa.close)(pcm);
    }
}
