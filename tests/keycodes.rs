//! Golden tests for the shared keysym table.
//!
//! Before the X11 and Wayland backends shared one table, each had its
//! own; this locks every keysym the old X11 table mapped (plus the
//! Wayland-only LeftTab alias) to the same `KeyCode` after the merge.

#![cfg(target_os = "linux")]

use miniquad::KeyCode;
use miniquad::native::keycodes::translate_keysym;

/// (keysym, expected KeyCode) — the full pre-merge X11 table.
const X11_GOLDEN: &[(u32, KeyCode)] = &[
    (65307, KeyCode::Escape),
    (65289, KeyCode::Tab),
    (65505, KeyCode::LeftShift),
    (65506, KeyCode::RightShift),
    (65507, KeyCode::LeftControl),
    (65508, KeyCode::RightControl),
    (65511, KeyCode::LeftAlt),
    (65513, KeyCode::LeftAlt),
    (65406, KeyCode::RightAlt),
    (65027, KeyCode::RightAlt),
    (65512, KeyCode::RightAlt),
    (65514, KeyCode::RightAlt),
    (65515, KeyCode::LeftSuper),
    (65516, KeyCode::RightSuper),
    (65383, KeyCode::Menu),
    (65407, KeyCode::NumLock),
    (65509, KeyCode::CapsLock),
    (65377, KeyCode::PrintScreen),
    (65300, KeyCode::ScrollLock),
    (65299, KeyCode::Pause),
    (65535, KeyCode::Delete),
    (65288, KeyCode::Backspace),
    (65293, KeyCode::Enter),
    (65360, KeyCode::Home),
    (65367, KeyCode::End),
    (65365, KeyCode::PageUp),
    (65366, KeyCode::PageDown),
    (65379, KeyCode::Insert),
    (65361, KeyCode::Left),
    (65363, KeyCode::Right),
    (65364, KeyCode::Down),
    (65362, KeyCode::Up),
    (65470, KeyCode::F1),
    (65471, KeyCode::F2),
    (65472, KeyCode::F3),
    (65473, KeyCode::F4),
    (65474, KeyCode::F5),
    (65475, KeyCode::F6),
    (65476, KeyCode::F7),
    (65477, KeyCode::F8),
    (65478, KeyCode::F9),
    (65479, KeyCode::F10),
    (65480, KeyCode::F11),
    (65481, KeyCode::F12),
    (65482, KeyCode::F13),
    (65483, KeyCode::F14),
    (65484, KeyCode::F15),
    (65485, KeyCode::F16),
    (65486, KeyCode::F17),
    (65487, KeyCode::F18),
    (65488, KeyCode::F19),
    (65489, KeyCode::F20),
    (65490, KeyCode::F21),
    (65491, KeyCode::F22),
    (65492, KeyCode::F23),
    (65493, KeyCode::F24),
    (65494, KeyCode::F25),
    (65455, KeyCode::KpDivide),
    (65450, KeyCode::KpMultiply),
    (65453, KeyCode::KpSubtract),
    (65451, KeyCode::KpAdd),
    (65438, KeyCode::Kp0),
    (65436, KeyCode::Kp1),
    (65433, KeyCode::Kp2),
    (65435, KeyCode::Kp3),
    (65430, KeyCode::Kp4),
    (65437, KeyCode::Kp5),
    (65432, KeyCode::Kp6),
    (65429, KeyCode::Kp7),
    (65431, KeyCode::Kp8),
    (65434, KeyCode::Kp9),
    (65439, KeyCode::KpDecimal),
    (65469, KeyCode::KpEqual),
    (65421, KeyCode::KpEnter),
    (97, KeyCode::A),
    (98, KeyCode::B),
    (99, KeyCode::C),
    (100, KeyCode::D),
    (101, KeyCode::E),
    (102, KeyCode::F),
    (103, KeyCode::G),
    (104, KeyCode::H),
    (105, KeyCode::I),
    (106, KeyCode::J),
    (107, KeyCode::K),
    (108, KeyCode::L),
    (109, KeyCode::M),
    (110, KeyCode::N),
    (111, KeyCode::O),
    (112, KeyCode::P),
    (113, KeyCode::Q),
    (114, KeyCode::R),
    (115, KeyCode::S),
    (116, KeyCode::T),
    (117, KeyCode::U),
    (118, KeyCode::V),
    (119, KeyCode::W),
    (120, KeyCode::X),
    (121, KeyCode::Y),
    (122, KeyCode::Z),
    (49, KeyCode::Key1),
    (50, KeyCode::Key2),
    (51, KeyCode::Key3),
    (52, KeyCode::Key4),
    (53, KeyCode::Key5),
    (54, KeyCode::Key6),
    (55, KeyCode::Key7),
    (56, KeyCode::Key8),
    (57, KeyCode::Key9),
    (48, KeyCode::Key0),
    (32, KeyCode::Space),
    (45, KeyCode::Minus),
    (61, KeyCode::Equal),
    (91, KeyCode::LeftBracket),
    (93, KeyCode::RightBracket),
    (92, KeyCode::Backslash),
    (59, KeyCode::Semicolon),
    (39, KeyCode::Apostrophe),
    (96, KeyCode::GraveAccent),
    (44, KeyCode::Comma),
    (46, KeyCode::Period),
    (47, KeyCode::Slash),
    (60, KeyCode::World1),
];

#[test]
fn shared_table_matches_the_old_x11_table() {
    for &(keysym, expected) in X11_GOLDEN {
        assert_eq!(
            translate_keysym(keysym),
            expected,
            "keysym {keysym:#x} drifted from the pre-merge X11 table",
        );
    }
}

#[test]
fn modifier_aliases_map_to_the_same_key() {
    // LeftAlt/RightAlt accept several vendor-specific keysyms.
    assert_eq!(translate_keysym(65511), KeyCode::LeftAlt);
    assert_eq!(translate_keysym(65513), KeyCode::LeftAlt);
    assert_eq!(translate_keysym(65406), KeyCode::RightAlt);
    assert_eq!(translate_keysym(65027), KeyCode::RightAlt);
    assert_eq!(translate_keysym(65512), KeyCode::RightAlt);
    assert_eq!(translate_keysym(65514), KeyCode::RightAlt);
}

#[test]
fn wayland_only_aliases_still_resolve() {
    // Wayland reports LeftTab/Keypad aliases the X11 path never sees.
    assert_eq!(translate_keysym(65056), KeyCode::Tab);
    assert_eq!(translate_keysym(65407), KeyCode::NumLock);
    assert_eq!(translate_keysym(65509), KeyCode::CapsLock);
}
