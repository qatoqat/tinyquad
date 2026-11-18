use super::libx11::{Cursor, Display, LibX11, Window, XColor};
use core::ffi::{c_char, c_int, c_uint, c_ulong};

pub unsafe fn create_empty_cursor(
    display: *mut Display,
    root: Window,
    libx11: &mut LibX11,
) -> Cursor {
    unsafe {
        let mut cursor_color = XColor {
            pixel: 0 as c_int as c_ulong,
            red: 0,
            green: 0,
            blue: 0,
            flags: 0,
            pad: 0,
        };

        let mut cursor_color_data: [c_char; 1] = [0 as c_int as c_char];
        let cursor_pixmap = (libx11.XCreateBitmapFromData)(
            display,
            root,
            cursor_color_data.as_mut_ptr(),
            1 as c_int as c_uint,
            1 as c_int as c_uint,
        );
        let empty_cursor = (libx11.XCreatePixmapCursor)(
            display,
            cursor_pixmap,
            cursor_pixmap,
            &mut cursor_color,
            &mut cursor_color,
            0 as c_int as c_uint,
            0 as c_int as c_uint,
        );
        (libx11.XFreePixmap)(display, cursor_pixmap);

        empty_cursor
    }
}
