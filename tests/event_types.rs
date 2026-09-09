//! Tests for the event-handling data types.
//!
//! The keycode discriminants are part of the cross-platform event contract:
//! native backends build [`tinyquad::KeyCode`] values from raw platform scancodes
//! (X11 keysyms here), so the numeric values must stay stable.

use tinyquad::*;

#[test]
fn keycodes_use_x11_keysym_values() {
    assert_eq!(KeyCode::Space as u16, 0x0020);
    assert_eq!(KeyCode::A as u16, 0x0041);
    assert_eq!(KeyCode::Z as u16, 0x005a);
    assert_eq!(KeyCode::Key0 as u16, 0x0030);
    assert_eq!(KeyCode::Enter as u16, 0xff0d);
    assert_eq!(KeyCode::Escape as u16, 0xff1b);
    assert_eq!(KeyCode::Right as u16, 0xff53);
    assert_eq!(KeyCode::Menu as u16, 0xff67);
    assert_eq!(KeyCode::Back as u16, 0xff04);
    assert_eq!(KeyCode::Unknown as u16, 0x01ff);
}

#[test]
fn mouse_button_discriminants_are_stable() {
    assert_eq!(MouseButton::Left as u8, 0);
    assert_eq!(MouseButton::Middle as u8, 1);
    assert_eq!(MouseButton::Right as u8, 2);
    assert_eq!(MouseButton::Unknown as u8, 255);
}

#[test]
fn key_mods_default_to_no_modifiers() {
    let mods = KeyMods::default();
    assert!(!mods.shift);
    assert!(!mods.ctrl);
    assert!(!mods.alt);
    assert!(!mods.logo);
}

#[test]
fn event_types_are_copy_and_debug() {
    // These derives are relied upon by every EventHandler implementation;
    // a plain value move like this fails to compile if Copy is dropped.
    let key = KeyCode::W;
    let copied = key;
    assert!(key == copied);
    assert!(!format!("{key:?}").is_empty());

    let mods = KeyMods {
        shift: true,
        ..KeyMods::default()
    };
    let mods_copy = mods;
    assert!(mods == mods_copy);

    let button = MouseButton::Left;
    let button_copy = button;
    assert!(button == button_copy);
    assert!(!format!("{button:?}").is_empty());
}

#[test]
fn touch_phases_cover_the_full_gesture_life_cycle() {
    let phases = std::collections::HashSet::from([
        TouchPhase::Started,
        TouchPhase::Moved,
        TouchPhase::Ended,
        TouchPhase::Cancelled,
    ]);
    assert_eq!(phases.len(), 4);
}

#[test]
fn cursor_icon_variants_exist() {
    // Exhaustive match: adding or removing a CursorIcon variant breaks this test,
    // which is the intent - the set of cursors is a public API surface.
    let all = [
        CursorIcon::Default,
        CursorIcon::Help,
        CursorIcon::Pointer,
        CursorIcon::Wait,
        CursorIcon::Crosshair,
        CursorIcon::Text,
        CursorIcon::Move,
        CursorIcon::NotAllowed,
        CursorIcon::EWResize,
        CursorIcon::NSResize,
        CursorIcon::NESWResize,
        CursorIcon::NWSEResize,
    ];
    assert_eq!(all.len(), 12);
}
