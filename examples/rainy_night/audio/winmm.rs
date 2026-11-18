//! Windows: waveOut (winmm), the oldest and simplest PCM API and still present everywhere.
//! A thread keeps a few small buffers rotating: waveOut signals an event whenever it has
//! finished one, we refill every finished buffer and queue it again.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Media::Audio::{
    CALLBACK_EVENT, WAVE_FORMAT_PCM, WAVE_MAPPER, WAVEFORMATEX, WAVEHDR, WHDR_DONE, waveOutClose,
    waveOutOpen, waveOutPrepareHeader, waveOutReset, waveOutUnprepareHeader, waveOutWrite,
};
use windows_sys::Win32::Media::MMSYSERR_NOERROR;
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

const SAMPLE_RATE: u32 = 48000;
/// Frames per buffer (21 ms). Four are queued, so about 85 ms are in flight.
const FRAMES: usize = 1024;
const BUFFERS: usize = 4;
const HDR_SIZE: u32 = size_of::<WAVEHDR>() as u32;

pub struct Backend {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

pub fn start() -> Option<Backend> {
    super::init(SAMPLE_RATE as f32);
    let stop = Arc::new(AtomicBool::new(false));
    let thread = std::thread::Builder::new()
        .name("rn-audio".into())
        .spawn({
            let stop = stop.clone();
            move || run(&stop)
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

fn run(stop: &AtomicBool) {
    // 16-bit stereo PCM: the one format every waveOut driver accepts.
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 2,
        nSamplesPerSec: SAMPLE_RATE,
        nAvgBytesPerSec: SAMPLE_RATE * 4,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    let mut mix = [0.0f32; 2 * FRAMES];
    let mut pcm = vec![[0i16; 2 * FRAMES]; BUFFERS];
    // SAFETY: by-the-book waveOut usage. `pcm` and `headers` are not moved or freed until
    // after waveOutReset + waveOutUnprepareHeader have returned every buffer to us.
    unsafe {
        // Auto-reset event, signalled by waveOut on open/close and on every finished buffer.
        let event = CreateEventW(std::ptr::null(), 0, 0, std::ptr::null());
        if event.is_null() {
            return;
        }
        let mut device = std::ptr::null_mut();
        if waveOutOpen(
            &mut device,
            WAVE_MAPPER,
            &format,
            event as usize,
            0,
            CALLBACK_EVENT,
        ) == MMSYSERR_NOERROR
        {
            let mut headers: Vec<WAVEHDR> = pcm
                .iter_mut()
                .map(|buffer| WAVEHDR {
                    lpData: buffer.as_mut_ptr().cast(),
                    dwBufferLength: size_of_val(buffer) as u32,
                    // Marked "done" so that the first pass of the loop fills them all.
                    dwFlags: WHDR_DONE,
                    ..Default::default()
                })
                .collect();
            let mut ok = headers
                .iter_mut()
                .all(|header| waveOutPrepareHeader(device, header, HDR_SIZE) == MMSYSERR_NOERROR);
            // waveOutPrepareHeader overwrites the flags; mark them done again.
            for header in &mut headers {
                header.dwFlags |= WHDR_DONE;
            }
            while ok && !stop.load(Ordering::Relaxed) {
                for (header, buffer) in headers.iter_mut().zip(&mut pcm) {
                    if header.dwFlags & WHDR_DONE == 0 {
                        continue;
                    }
                    super::render(&mut mix);
                    for (o, s) in buffer.iter_mut().zip(&mix) {
                        *o = (s * 32767.0) as i16;
                    }
                    header.dwFlags &= !WHDR_DONE;
                    ok &= waveOutWrite(device, header, HDR_SIZE) == MMSYSERR_NOERROR;
                }
                // The timeout only matters for noticing `stop` if the device stalls.
                WaitForSingleObject(event, 100);
            }
            waveOutReset(device);
            for header in &mut headers {
                waveOutUnprepareHeader(device, header, HDR_SIZE);
            }
            waveOutClose(device);
        }
        CloseHandle(event);
    }
}
