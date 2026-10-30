//! What the threads panel keeps: its search (the field, the words, the agent's hits, the panel's
//! chosen row), the
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
    /// The search's panel over the window (⌘K) while open: the row chosen in it.
    pub finding: Option<usize>,
    /// Groups folded or opened against their default (`p:<id>`, `open:loose`), kept on this Mac.
    pub collapsed: HashSet<String>,
    /// Pins and colours of threads and projects (this Mac).
    pub marks: HashMap<String, Mark>,
    /// The projects in the order the person put them by dragging (`order.rs`); those not in it
    /// yet keep the place their source gave, after these.
    pub order: Vec<String>,
    /// The group the project in the air was last over, and at which edge of it: the line says so,
    /// and only while GPUI says a drag is live, so this never outlives one.
    pub drop: Option<(String, bench_ui::DropEdge)>,
    /// A drop line held over a group whatever the pointer does (a picture of how it looks).
    pub held_drop: Option<(usize, bench_ui::DropEdge)>,
    /// The agent's popover opens with the window (a picture of it).
    pub agent_popover_open: bool,
    /// A thread or project being renamed in place.
    pub renaming: Option<Renaming>,
    /// The panel's keys (↑ ↓ ⌫) while it has the focus, after a click on a thread.
    pub focus: FocusHandle,
    /// The last lines of the open thread's agent's log, read as its popover opens.
    pub agent_log: Vec<String>,
}

impl SidebarState {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        Self {
            search: cx.new(|cx| InputState::new(window, cx).placeholder("Search threads and projects")),
            query: String::new(),
            hits: None,
            seq: 0,
            finding: None,
            collapsed: std::fs::read(collapsed_path())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default(),
            marks: super::marks::load(),
            order: super::order::load(),
            drop: None,
            held_drop: None,
            agent_popover_open: false,
            renaming: None,
            focus: cx.focus_handle(),
            agent_log: vec![],
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
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(
            collapsed_path(),
            serde_json::to_vec(&self.sidebar.collapsed).unwrap_or_default(),
        );
        cx.notify();
    }
}
