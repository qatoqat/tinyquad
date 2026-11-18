//! macOS: AudioQueue Services (AudioToolbox). The queue owns three buffers; whenever it
//! finishes playing one it calls us back on its own thread, we refill and re-enqueue it.

use std::ffi::c_void;
use std::ptr::null_mut;

const SAMPLE_RATE: f64 = 48000.0;
/// Frames per buffer (21 ms); three of them are in flight.
const FRAMES: u32 = 1024;
const BUFFERS: usize = 3;
const BYTES_PER_FRAME: u32 = 8; // 2 x f32

type OSStatus = i32;
type AudioQueueRef = *mut c_void;

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
struct AudioQueueBuffer {
    audio_data_bytes_capacity: u32,
    audio_data: *mut c_void,
    audio_data_byte_size: u32,
    user_data: *mut c_void,
    packet_description_capacity: u32,
    packet_descriptions: *mut c_void,
    packet_description_count: u32,
}

type OutputCallback = extern "C" fn(*mut c_void, AudioQueueRef, *mut AudioQueueBuffer);

const FORMAT_LINEAR_PCM: u32 = u32::from_be_bytes(*b"lpcm");
const FLAG_IS_FLOAT: u32 = 1 << 0;
const FLAG_IS_PACKED: u32 = 1 << 3;

#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioQueueNewOutput(
        format: *const AudioStreamBasicDescription,
        callback: OutputCallback,
        user_data: *mut c_void,
        callback_run_loop: *mut c_void,
        callback_run_loop_mode: *mut c_void,
        flags: u32,
        out_queue: *mut AudioQueueRef,
    ) -> OSStatus;
    fn AudioQueueAllocateBuffer(
        queue: AudioQueueRef,
        byte_size: u32,
        out_buffer: *mut *mut AudioQueueBuffer,
    ) -> OSStatus;
    fn AudioQueueEnqueueBuffer(
        queue: AudioQueueRef,
        buffer: *mut AudioQueueBuffer,
        num_packet_descs: u32,
        packet_descs: *const c_void,
    ) -> OSStatus;
    fn AudioQueueStart(queue: AudioQueueRef, start_time: *const c_void) -> OSStatus;
    fn AudioQueueStop(queue: AudioQueueRef, immediate: u8) -> OSStatus;
    fn AudioQueueDispose(queue: AudioQueueRef, immediate: u8) -> OSStatus;
}

/// Runs on the AudioQueue's internal thread each time a buffer has been played.
extern "C" fn refill(_user: *mut c_void, queue: AudioQueueRef, buffer: *mut AudioQueueBuffer) {
    // SAFETY: the queue hands us a valid buffer it allocated with room for FRAMES frames
    // of 32-bit floats (suitably aligned by the allocator).
    unsafe {
        let buffer_ref = &mut *buffer;
        let samples = (buffer_ref.audio_data_bytes_capacity / 4) as usize;
        let out = std::slice::from_raw_parts_mut(buffer_ref.audio_data.cast::<f32>(), samples);
        super::render(out);
        buffer_ref.audio_data_byte_size = (samples * 4) as u32;
        AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null());
    }
}

pub struct Backend {
    queue: AudioQueueRef,
}

// SAFETY: an AudioQueueRef may be used from any thread; we only stop/dispose it on drop.
unsafe impl Send for Backend {}

pub fn start() -> Option<Backend> {
    let format = AudioStreamBasicDescription {
        sample_rate: SAMPLE_RATE,
        format_id: FORMAT_LINEAR_PCM,
        format_flags: FLAG_IS_FLOAT | FLAG_IS_PACKED,
        bytes_per_packet: BYTES_PER_FRAME,
        frames_per_packet: 1,
        bytes_per_frame: BYTES_PER_FRAME,
        channels_per_frame: 2,
        bits_per_channel: 32,
        reserved: 0,
    };
    super::init(SAMPLE_RATE as f32);
    // SAFETY: by-the-book AudioQueue setup; every status is checked and the queue is
    // disposed on any failure. A null run loop means "call back on an internal thread".
    unsafe {
        let mut queue = null_mut();
        if AudioQueueNewOutput(
            &format,
            refill,
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut queue,
        ) != 0
        {
            return None;
        }
        let backend = Backend { queue };
        for _ in 0..BUFFERS {
            let mut buffer = null_mut();
            if AudioQueueAllocateBuffer(queue, FRAMES * BYTES_PER_FRAME, &mut buffer) != 0 {
                return None;
            }
            // Prime the queue: fill and enqueue each buffer once by hand.
            refill(null_mut(), queue, buffer);
        }
        if AudioQueueStart(queue, std::ptr::null()) != 0 {
            return None;
        }
        Some(backend)
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        // SAFETY: `queue` is a live queue created in `start`. Stopping synchronously
        // guarantees the callback is not running any more; disposing frees the buffers.
        unsafe {
            AudioQueueStop(self.queue, 1);
            AudioQueueDispose(self.queue, 1);
        }
    }
}
