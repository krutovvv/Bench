//! The window's layout, as Zed's workspace has it: a left dock (the sidebar) showing one panel —
//! the threads or the Explorer —, the center showing a thread, a document or a settings page, and
//! a right dock keeping a file beside it (`docs/explorer.md`). What fills each is the panel's or
//! item's own module (`src/ui/sidebar.rs`, `src/explorer/`, `src/ui/thread/`, `src/env/`); here
//! is only which one is shown.

use gpui_kit::*;

use super::BenchApp;

/// What the left dock shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarPanel {
    /// Projects and their threads (`src/ui/sidebar.rs`).
    #[default]
    Threads,
    /// One project's tree (`src/explorer/tree.rs`).
    Explorer,
}

/// What the center shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Center {
    /// The open thread.
    Chat,
    /// An open document of the Explorer, with the tabs of the others (`src/explorer/view.rs`).
    Doc,
    /// The gear: agents, presets, appearance.
    Providers,
    /// A new project: a page of its own, not a thread (a project is not a conversation).
    NewProject,
    /// A project's Settings (`project_env`, `src/env/page.rs`).
    Project,
}

impl Center {
    /// A page with its own title and a close in the header (not a thread, not a document).
    pub fn is_settings(self) -> bool {
        matches!(
            self,
            Center::Providers | Center::NewProject | Center::Project
        )
    }
}

impl BenchApp {
    /// Show `center` in the middle, reading what it needs.
    pub fn open_center(&mut self, center: Center, window: &mut Window, cx: &mut Context<Self>) {
        self.center = center;
        if center == Center::Providers {
            self.load_providers(window, cx);
        }
        cx.notify();
    }
}
