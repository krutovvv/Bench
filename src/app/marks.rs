//! Per-session marks kept on this Mac: a color (a quiet stripe at the row's left) and a pin
//! (the "Pinned" group at the top of the list). Also opening another window.

use std::collections::HashMap;
use std::sync::Mutex;

use gpui_kit::*;

use super::BenchApp;
use crate::connection;

/// Session colors: red, orange, yellow, green, blue, violet, pink (saved by index: new ones go
/// at the end, so a saved color never changes).
pub const SESSION_COLORS: [u32; 7] = [
    0xD0605A, 0xD99A4E, 0xD6C24A, 0x6DB86A, 0x5C8FD8, 0x9A6AD6, 0xD46A9F,
];

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Mark {
    #[serde(default)]
    pub color: Option<usize>,
    #[serde(default)]
    pub pinned: bool,
}

/// Where the next window should start: (target label, profile, session).
pub static NEXT_WINDOW: Mutex<Option<(String, Option<String>, Option<String>)>> = Mutex::new(None);

fn path() -> std::path::PathBuf {
    connection::state_dir().join("session-marks.json")
}

pub(crate) fn load() -> HashMap<String, Mark> {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

impl BenchApp {
    fn store_marks(&self) {
        // Another window may have changed other sessions' marks: merge into what is on disk.
        let mut all = load();
        for (k, v) in &self.marks {
            all.insert(k.clone(), *v);
        }
        all.retain(|_, m| m.pinned || m.color.is_some());
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path(), serde_json::to_vec(&all).unwrap_or_default());
    }

    pub fn mark_of(&self, session: &str) -> Mark {
        self.marks.get(session).copied().unwrap_or_default()
    }

    pub fn toggle_pin(&mut self, session: String, cx: &mut Context<Self>) {
        let m = self.marks.entry(session).or_default();
        m.pinned = !m.pinned;
        self.store_marks();
        cx.notify();
    }

    pub fn set_color(&mut self, session: String, color: Option<usize>, cx: &mut Context<Self>) {
        self.marks.entry(session).or_default().color = color;
        self.store_marks();
        cx.notify();
    }

    /// Another window on the same gateway and profile; with `session`, that chat open in it.
    pub fn open_window(&mut self, session: Option<String>, cx: &mut Context<Self>) {
        *NEXT_WINDOW.lock().unwrap() = Some((self.target.label(), self.profile.clone(), session));
        crate::open_main_window(cx);
    }
}
