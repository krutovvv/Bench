//! Notifications where Bench has no native binding yet (Linux, Windows, the browser): nothing
//! is shown. macOS: src/system/notify.rs.

use std::sync::atomic::AtomicU8;

/// As macOS's: 0 = not asked yet, 1 = allowed, 2 = refused. Here never asked.
pub static AUTH: AtomicU8 = AtomicU8::new(0);

pub fn check() {}

pub fn request() {}

pub fn post(_id: &str, _title: &str, _body: &str) {}
