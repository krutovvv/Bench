//! What the threads panel keeps: its search (the field, the words, the agent's hits), the
//! groups folded, the marks (pins and colours), the item being renamed, and the focus that lets
//! ↑ ↓ ⌫ work on its rows.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::input::InputState;
use gpui_kit::*;

use super::items::Renaming;
use super::marks::Mark;
use crate::app::BenchApp;
use crate::client::connection;

pub struct SidebarState {
    /// The search field, what was typed, the agent's hits (None while searching), the newest
    /// search's number (only its answer is shown).
    pub search: Entity<InputState>,
    pub query: String,
    pub hits: Option<Vec<(String, String)>>,
    pub seq: u64,
    /// Groups folded or opened against their default (`p:<id>`, `open:loose`), kept on this Mac.
    pub collapsed: HashSet<String>,
    /// Pins and colours of threads and projects (this Mac).
    pub marks: HashMap<String, Mark>,
    /// A thread or project being renamed in place.
    pub renaming: Option<Renaming>,
    /// The panel's keys (↑ ↓ ⌫) while it has the focus, after a click on a thread.
    pub focus: FocusHandle,
}

impl SidebarState {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        Self {
            search: cx.new(|cx| InputState::new(window, cx).placeholder("Search threads")),
            query: String::new(),
            hits: None,
            seq: 0,
            collapsed: std::fs::read(collapsed_path())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default(),
            marks: super::marks::load(),
            renaming: None,
            focus: cx.focus_handle(),
        }
    }
}

fn collapsed_path() -> std::path::PathBuf {
    connection::state_dir().join("collapsed.json")
}

impl BenchApp {
    /// Fold or unfold a sidebar group (its header was clicked).
    pub fn toggle_group(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.sidebar.collapsed.remove(&key) {
            self.sidebar.collapsed.insert(key);
        }
        // The demo folds and unfolds in memory: the person's own folds stay as they were.
        if self.demo {
            cx.notify();
            return;
        }
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(
            collapsed_path(),
            serde_json::to_vec(&self.sidebar.collapsed).unwrap_or_default(),
        );
        cx.notify();
    }
}
