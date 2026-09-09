//! There are quite a few different notions for keycodes. And most of them are `u32` so it does get
//! very confusing...
//! Basically
//!   - Wayland server sends a scancode `key` of type `c_uint`
//!   - `key + 8` becomes a `xkb` scancode of type `xkb_keycode_t`
//!   - We feed this to `xkb` to get a `keysym` of type `xkb_keysym_t`
//!     - The `keysym` can be modifier-dependent: `Shift + Key1` can be translated to either `Key1`
//!       (without modifier) or `Exclam` (with modifier)
//!   - We then feed the `keysym` to `translate_keysym` to get a Miniquad `Keycode`
//!
//! Note that the default Miniquad behavior is without modifier; there is not even a Keycode for
//! `Exclam`. So we must provide the unmodified `keysym` or we will get a `Keycode::Unknown`.
//!
//! On the other hand, the modified `keysym` is useful when we want to translate it into the
//! underlying character.
pub use crate::native::keycodes::translate_keysym;
