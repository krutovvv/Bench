//! Marks of the sidebar's items, kept on this Mac (`sidebar_items.rs`): a pin (first in its
//! group) and a color — a thread's is a quiet stripe and tint on its row, a project's its icon
//! and name. Keyed by a thread's id, a project's `p:<id>`. Also opening another window.

use std::collections::HashMap;
use std::sync::Mutex;

use gpui_kit::*;
use protocol::ThreadId;

use crate::app::BenchApp;
use crate::client::connection;

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Mark {
    #[serde(default)]
    pub color: Option<usize>,
    #[serde(default)]
    pub pinned: bool,
}

/// The thread the next window opens: its agent and its id.
pub static NEXT_WINDOW: Mutex<Option<(String, ThreadId)>> = Mutex::new(None);

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
        for (k, v) in &self.sidebar.marks {
            all.insert(k.clone(), *v);
        }
        all.retain(|_, m| m.pinned || m.color.is_some());
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path(), serde_json::to_vec(&all).unwrap_or_default());
    }

    pub fn mark_of(&self, session: &str) -> Mark {
        self.sidebar.marks.get(session).copied().unwrap_or_default()
    }

    pub fn toggle_pin(&mut self, session: String, cx: &mut Context<Self>) {
        let m = self.sidebar.marks.entry(session).or_default();
        m.pinned = !m.pinned;
        self.store_marks();
        cx.notify();
    }

    pub fn set_color(&mut self, session: String, color: Option<usize>, cx: &mut Context<Self>) {
        self.sidebar.marks.entry(session).or_default().color = color;
        self.store_marks();
        cx.notify();
    }

    /// Another window; with `thread`, that thread open in it (else the last one).
    pub fn open_window(&mut self, thread: Option<(String, ThreadId)>, cx: &mut Context<Self>) {
        *NEXT_WINDOW.lock().unwrap() = thread;
        crate::open_main_window(cx);
    }
}
