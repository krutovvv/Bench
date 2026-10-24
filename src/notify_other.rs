//! Notifications where Bench has no native binding yet (Linux, Windows, the browser): nothing
//! is shown. macOS: src/notify.rs.

pub fn request() {}

pub fn post(_id: &str, _title: &str, _body: &str) {}
