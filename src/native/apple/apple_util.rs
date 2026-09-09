// original copyright belongs to Makepad
// Copy-pasted from https://github.com/makepad/makepad/blob/live/platform/src/platform/apple/apple_utils.rs
// and slightly modified

// Helpers around ObjC message sends (`msg_send!`) take `ObjcId` raw pointers;
// callers are the platform event loops that already run inside `unsafe` blocks.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::{
    CursorIcon,
    event::{KeyCode, KeyMods},
    native::apple::frameworks::*,
};

pub fn nsstring_to_string(string: ObjcId) -> String {
    unsafe {
        let utf8_string: *const core::ffi::c_uchar = msg_send![string, UTF8String];
        let utf8_len: usize = msg_send![string, lengthOfBytesUsingEncoding: UTF8_ENCODING];
        let slice = std::slice::from_raw_parts(utf8_string, utf8_len);
        std::str::from_utf8_unchecked(slice).to_owned()
    }
}

pub fn str_to_nsstring(str: &str) -> ObjcId {
    unsafe {
        let ns_string: ObjcId = msg_send![class!(NSString), alloc];
        let ns_string: ObjcId = msg_send![
            ns_string,
            initWithBytes: str.as_ptr()
            length: str.len()
            encoding: UTF8_ENCODING as ObjcId
        ];
        let _: () = msg_send![ns_string, autorelease];
        ns_string
    }
}

pub fn load_native_cursor(cursor_name: &str) -> ObjcId {
    let sel = Sel::register(cursor_name);
    let id: ObjcId = unsafe { msg_send![class!(NSCursor), performSelector: sel] };
    id
}

pub fn load_undocumented_cursor(cursor_name: &str) -> ObjcId {
    unsafe {
        let class = class!(NSCursor);
        let sel = Sel::register(cursor_name);
        let has_selector: BOOL = msg_send![class, respondsToSelector: sel];
        let id: ObjcId = if has_selector != NO {
            msg_send![class, performSelector: sel]
        } else {
            msg_send![class, arrowCursor]
        };
        id
    }
}

/// `dlsym` a GL symbol out of one of Apple's OpenGL frameworks.
/// Only one framework gets loaded per process, so the handle is
/// cached in a single static.
pub unsafe fn get_proc_address_from(
    framework: &'static core::ffi::CStr,
    name: *const u8,
) -> Option<unsafe extern "C" fn()> {
    unsafe {
        mod libc {
            use std::ffi::{c_char, c_int, c_void};

            pub const RTLD_LAZY: c_int = 1;
            unsafe extern "C" {
                pub fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
                pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
            }
        }
        static mut OPENGL: *mut std::ffi::c_void = std::ptr::null_mut();

        if OPENGL.is_null() {
            OPENGL = libc::dlopen(framework.as_ptr() as _, libc::RTLD_LAZY);
        }

        assert!(!OPENGL.is_null());

        let symbol = libc::dlsym(OPENGL, name as _);
        if symbol.is_null() {
            return None;
        }
        Some(std::mem::transmute_copy(&symbol))
    }
}

pub fn get_event_char(event: ObjcId) -> Option<char> {
    unsafe {
        let characters: ObjcId = msg_send![event, characters];
        if characters == nil {
            return None;
        }
        let chars = nsstring_to_string(characters);

        if chars.is_empty() {
            return None;
        }
        Some(chars.chars().next().unwrap())
    }
}

pub fn get_event_key_modifier(event: ObjcId) -> KeyMods {
    let flags: u64 = unsafe { msg_send![event, modifierFlags] };
    KeyMods {
        shift: flags & NSEventModifierFlags::NSShiftKeyMask as u64 != 0,
        ctrl: flags & NSEventModifierFlags::NSControlKeyMask as u64 != 0,
        alt: flags & NSEventModifierFlags::NSAlternateKeyMask as u64 != 0,
        logo: flags & NSEventModifierFlags::NSCommandKeyMask as u64 != 0,
    }
}

pub fn get_event_keycode(event: ObjcId) -> Option<KeyCode> {
    let scan_code: core::ffi::c_ushort = unsafe { msg_send![event, keyCode] };

    // Check mapping here
    // https://boredzo.org/blog/archives/2007-05-22/virtual-key-codes
    Some(match scan_code {
        0x00 => KeyCode::A,
        0x01 => KeyCode::S,
        0x02 => KeyCode::D,
        0x03 => KeyCode::F,
        0x04 => KeyCode::H,
        0x05 => KeyCode::G,
        0x06 => KeyCode::Z,
        0x07 => KeyCode::X,
        0x08 => KeyCode::C,
        0x09 => KeyCode::V,
        //0x0a => World 1,
        0x0b => KeyCode::B,
        0x0c => KeyCode::Q,
        0x0d => KeyCode::W,
        0x0e => KeyCode::E,
        0x0f => KeyCode::R,
        0x10 => KeyCode::Y,
        0x11 => KeyCode::T,
        0x12 => KeyCode::Key1,
        0x13 => KeyCode::Key2,
        0x14 => KeyCode::Key3,
        0x15 => KeyCode::Key4,
        0x16 => KeyCode::Key6,
        0x17 => KeyCode::Key5,
        0x18 => KeyCode::Equal,
        0x19 => KeyCode::Key9,
        0x1a => KeyCode::Key7,
        0x1b => KeyCode::Minus,
        0x1c => KeyCode::Key8,
        0x1d => KeyCode::Key0,
        0x1e => KeyCode::RightBracket,
        0x1f => KeyCode::O,
        0x20 => KeyCode::U,
        0x21 => KeyCode::LeftBracket,
        0x22 => KeyCode::I,
        0x23 => KeyCode::P,
        0x24 => KeyCode::Enter,
        0x25 => KeyCode::L,
        0x26 => KeyCode::J,
        0x27 => KeyCode::Apostrophe,
        0x28 => KeyCode::K,
        0x29 => KeyCode::Semicolon,
        0x2a => KeyCode::Backslash,
        0x2b => KeyCode::Comma,
        0x2c => KeyCode::Slash,
        0x2d => KeyCode::N,
        0x2e => KeyCode::M,
        0x2f => KeyCode::Period,
        0x30 => KeyCode::Tab,
        0x31 => KeyCode::Space,
        0x32 => KeyCode::GraveAccent,
        0x33 => KeyCode::Backspace,
        //0x34 => unkown,
        0x35 => KeyCode::Escape,
        //0x36 => KeyCode::RLogo,
        //0x37 => KeyCode::LLogo,
        //0x38 => KeyCode::LShift,
        0x39 => KeyCode::CapsLock,
        //0x3a => KeyCode::LAlt,
        //0x3b => KeyCode::LControl,
        //0x3c => KeyCode::RShift,
        //0x3d => KeyCode::RAlt,
        //0x3e => KeyCode::RControl,
        //0x3f => Fn key,
        //0x40 => KeyCode::F17,
        0x41 => KeyCode::KpDecimal,
        //0x42 -> unkown,
        0x43 => KeyCode::KpMultiply,
        //0x44 => unkown,
        0x45 => KeyCode::KpAdd,
        //0x46 => unkown,
        0x47 => KeyCode::NumLock,
        //0x48 => KeypadClear,
        //0x49 => KeyCode::VolumeUp,
        //0x4a => KeyCode::VolumeDown,
        0x4b => KeyCode::KpDivide,
        0x4c => KeyCode::KpEnter,
        0x4e => KeyCode::KpSubtract,
        //0x4d => unkown,
        //0x4e => KeyCode::Subtract,
        //0x4f => KeyCode::F18,
        //0x50 => KeyCode::F19,
        0x51 => KeyCode::KpEqual,
        0x52 => KeyCode::Kp0,
        0x53 => KeyCode::Kp1,
        0x54 => KeyCode::Kp2,
        0x55 => KeyCode::Kp3,
        0x56 => KeyCode::Kp4,
        0x57 => KeyCode::Kp5,
        0x58 => KeyCode::Kp6,
        0x59 => KeyCode::Kp7,
        //0x5a => KeyCode::F20,
        0x5b => KeyCode::Kp8,
        0x5c => KeyCode::Kp9,
        //0x5d => KeyCode::Yen,
        //0x5e => JIS Ro,
        //0x5f => unkown,
        0x60 => KeyCode::F5,
        0x61 => KeyCode::F6,
        0x62 => KeyCode::F7,
        0x63 => KeyCode::F3,
        0x64 => KeyCode::F8,
        0x65 => KeyCode::F9,
        //0x66 => JIS Eisuu (macOS),
        0x67 => KeyCode::F11,
        //0x68 => JIS Kana (macOS),
        0x69 => KeyCode::PrintScreen,
        //0x6a => KeyCode::F16,
        //0x6b => KeyCode::F14,
        //0x6c => unkown,
        0x6d => KeyCode::F10,
        //0x6e => unkown,
        0x6f => KeyCode::F12,
        //0x70 => unkown,
        //0x71 => KeyCode::F15,
        0x72 => KeyCode::Insert,
        0x73 => KeyCode::Home,
        0x74 => KeyCode::PageUp,
        0x75 => KeyCode::Delete,
        0x76 => KeyCode::F4,
        0x77 => KeyCode::End,
        0x78 => KeyCode::F2,
        0x79 => KeyCode::PageDown,
        0x7a => KeyCode::F1,
        0x7b => KeyCode::Left,
        0x7c => KeyCode::Right,
        0x7d => KeyCode::Down,
        0x7e => KeyCode::Up,
        //0x7f =>  unkown,
        //0xa => KeyCode::Caret,
        _ => return None,
    })
}

pub fn load_mouse_cursor(cursor: CursorIcon) -> ObjcId {
    match cursor {
        CursorIcon::Default => load_native_cursor("arrowCursor"),
        CursorIcon::Pointer => load_native_cursor("pointingHandCursor"),
        CursorIcon::Text => load_native_cursor("IBeamCursor"),
        CursorIcon::NotAllowed /*| CursorIcon::NoDrop*/ => load_native_cursor("operationNotAllowedCursor"),
        CursorIcon::Crosshair => load_native_cursor("crosshairCursor"),
        /*
        CursorIcon::Grabbing | CursorIcon::Grab => load_native_cursor("closedHandCursor"),
        CursorIcon::VerticalText => load_native_cursor("IBeamCursorForVerticalLayout"),
        CursorIcon::Copy => load_native_cursor("dragCopyCursor"),
        CursorIcon::Alias => load_native_cursor("dragLinkCursor"),
        CursorIcon::ContextMenu => load_native_cursor("contextualMenuCursor"),
        */
        //CursorIcon::EResize => load_native_cursor("resizeRightCursor"),
        //CursorIcon::NResize => load_native_cursor("resizeUpCursor"),
        //CursorIcon::WResize => load_native_cursor("resizeLeftCursor"),
        //CursorIcon::SResize => load_native_cursor("resizeDownCursor"),
        CursorIcon::EWResize => load_native_cursor("resizeLeftRightCursor"),
        CursorIcon::NSResize => load_native_cursor("resizeUpDownCursor"),

        CursorIcon::NESWResize => load_undocumented_cursor("_windowResizeNorthEastSouthWestCursor"),
        CursorIcon::NWSEResize => load_undocumented_cursor("_windowResizeNorthWestSouthEastCursor"),

        // Undocumented cursors: https://stackoverflow.com/a/46635398/5435443
        // Unfortunately undocumented cursors requires NSTracking areas that
        // we do not use yet.
        _ => load_native_cursor("arrowCursor"),
    }
}

macro_rules! msg_send_ {
    ($obj:expr, $name:ident) => ({
        let res: ObjcId = msg_send!($obj, $name);
        res
    });
    ($obj:expr, $($name:ident : $arg:expr)+) => ({
        let res: ObjcId = msg_send!($obj, $($name: $arg)*);
        res
    });
}
pub(crate) use msg_send_;

pub extern "C" fn yes(_: &Object, _: Sel) -> BOOL {
    YES
}

pub extern "C" fn yes1(_: &Object, _: Sel, _: ObjcId) -> BOOL {
    YES
}
