//! Varios X11 keycode to event mappings
//! for keyboard, mouse, basically any long table of keycode->enum belongs here

use super::{Display, LibX11};
use crate::event::{KeyCode, KeyMods, MouseButton};
use crate::native::keycodes::translate_keysym;
use crate::native::linux_x11::libx11::LibXkbCommon;

pub unsafe fn translate_key(libx11: &mut LibX11, display: *mut Display, scancode: i32) -> KeyCode {
    unsafe {
        let mut dummy: libc::c_int = 0;
        let keysyms =
            (libx11.XGetKeyboardMapping)(display, scancode as _, 1 as libc::c_int, &mut dummy);
        assert!(!keysyms.is_null());

        let keysym = *keysyms.offset(0 as libc::c_int as isize);
        (libx11.XFree)(keysyms as *mut libc::c_void);
        // X11 keysyms are the same keysym space the Wayland backend
        // reaches through `xkb`, so they share one translation table.
        translate_keysym(keysym as u32)
    }
}

pub unsafe fn translate_mod(x11_mods: i32) -> KeyMods {
    let mut mods = KeyMods::default();
    if x11_mods & super::libx11::ShiftMask != 0 {
        mods.shift = true;
    }
    if x11_mods & super::libx11::ControlMask != 0 {
        mods.ctrl = true;
    }
    if x11_mods & super::libx11::Mod1Mask != 0 {
        mods.alt = true;
    }
    if x11_mods & super::libx11::Mod4Mask != 0 {
        mods.logo = true;
    }
    mods
}

pub unsafe fn translate_mouse_button(button: i32) -> MouseButton {
    match button {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        _ => MouseButton::Unknown,
    }
}

pub unsafe extern "C" fn keysym_to_unicode(
    libxkbcommon: &mut LibXkbCommon,
    keysym: super::libx11::KeySym,
) -> i32 {
    unsafe { (libxkbcommon.xkb_keysym_to_utf32)(keysym as u32) as i32 }
}
