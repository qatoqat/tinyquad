use windows_sys::Win32::{
    Foundation::GlobalFree,
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Ole::CF_UNICODETEXT,
    },
};

use std::ptr;

struct ClipboardGuard;
impl ClipboardGuard {
    unsafe fn open() -> Option<Self> {
        unsafe {
            let result = OpenClipboard(ptr::null_mut());
            (result != 0).then_some(ClipboardGuard)
        }
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

unsafe fn get_raw_clipboard() -> Option<Vec<u16>> {
    unsafe {
        // https://docs.microsoft.com/en-us/windows/win32/dataxchg/about-the-clipboard

        let Some(_guard) = ClipboardGuard::open() else {
            eprintln!("Failed to open clipboard");
            return None;
        };

        // Returns a handle to a clipboard object
        let clipboard_data = GetClipboardData(CF_UNICODETEXT as u32);
        if clipboard_data.is_null() {
            return None;
        }

        let data_ptr = GlobalLock(clipboard_data) as *const u16;
        if data_ptr.is_null() {
            return None;
        }
        // `GlobalSize` is in bytes, the text is UTF-16.
        let max_len = GlobalSize(clipboard_data) / size_of::<u16>();

        // The string ends at the first NUL (or at the end of the allocation).
        let slice = std::slice::from_raw_parts(data_ptr, max_len);
        let len = slice.iter().position(|&c| c == 0).unwrap_or(max_len);
        let res = slice[..len].to_vec();

        GlobalUnlock(clipboard_data);

        Some(res)
    }
}

unsafe fn set_raw_clipboard(data: *const u8, len: usize) {
    unsafe {
        let Some(_guard) = ClipboardGuard::open() else {
            eprintln!("Failed to open clipboard");
            return;
        };

        let alloc_handle = GlobalAlloc(GMEM_MOVEABLE, len);

        if alloc_handle.is_null() {
            eprintln!("Failed to set clipboard: memory not allocated");
            return;
        }

        let lock = GlobalLock(alloc_handle) as *mut u8;
        if lock.is_null() {
            eprintln!("Failed to set clipboard: memory not locked");
            GlobalFree(alloc_handle);
            return;
        }
        ptr::copy_nonoverlapping(data, lock, len);

        GlobalUnlock(alloc_handle);
        EmptyClipboard();

        // On success the system owns the memory; otherwise it is still ours.
        if SetClipboardData(CF_UNICODETEXT as u32, alloc_handle).is_null() {
            eprintln!("Failed to set clipboard data");
            GlobalFree(alloc_handle);
        }
    }
}

pub struct WindowsClipboard {}
impl WindowsClipboard {
    pub fn new() -> WindowsClipboard {
        WindowsClipboard {}
    }
}
impl crate::native::Clipboard for WindowsClipboard {
    fn get(&mut self) -> Option<String> {
        unsafe { get_raw_clipboard().map(|data| String::from_utf16_lossy(&data)) }
    }

    fn set(&mut self, data: &str) {
        unsafe {
            let text_w: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
            set_raw_clipboard(text_w.as_ptr().cast(), size_of_val(&text_w[..]));
        }
    }
}
