//! Platforms without a backend (iOS, Android, ...): audio is silently disabled.

pub struct Backend;

pub fn start() -> Option<Backend> {
    None
}
