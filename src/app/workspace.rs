//! The window's layout, as Zed's workspace has it: a left dock (the sidebar) showing one panel —
//! the threads or the Explorer —, the center showing a thread, a document or a settings page, and
//! a right dock showing one panel — the thread beside the code, or a file beside whatever the
//! center shows (`docs/explorer.md`). What fills each is the panel's or
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

/// What the right dock shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RightPanel {
    /// The open thread, beside a document (`render_chat`, the center's own when it is `Chat`).
    Thread,
    /// A file kept beside (`src/explorer/view.rs`).
    File(crate::explorer::FileRef),
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
    /// What the right dock shows now: the thread only beside something else than itself.
    pub fn right_shown(&self) -> Option<RightPanel> {
        match &self.right {
            Some(RightPanel::Thread) if self.center == Center::Chat => None,
            other => other.clone(),
        }
    }

    /// The file kept beside, if the right dock holds one.
    pub fn beside_file(&self) -> Option<&crate::explorer::FileRef> {
        match &self.right {
            Some(RightPanel::File(f)) => Some(f),
            _ => None,
        }
    }

    /// ⌘J, the header's thread button: the thread beside the code, or not.
    pub fn toggle_thread_beside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.right == Some(RightPanel::Thread) {
            self.right = None;
        } else {
            self.right = Some(RightPanel::Thread);
            self.composer.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    /// Show `center` in the middle, reading what it needs.
    pub fn open_center(&mut self, center: Center, window: &mut Window, cx: &mut Context<Self>) {
        self.center = center;
        if center == Center::Providers {
            self.load_providers(window, cx);
        }
        cx.notify();
    }
}
