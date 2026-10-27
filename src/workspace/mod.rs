//! The window's layout, as Zed's workspace has it: three docks (`dock.rs`) around the center,
//! each showing one of its panels — the left one the threads or the Explorer's tree, the right
//! one the thread beside the code, the bottom one what runs under the center — and the center
//! showing a thread, a document (a project's settings among them) or a settings page
//! (`docs/explorer.md`). What fills each is the panel's or item's own module
//! (`src/sidebar/mod.rs`, `src/explorer/`, `src/thread/`, `src/env/`); here is only which one is
//! shown.

pub mod dock;
mod layout;

use gpui_kit::*;

pub use dock::{Dock, DockPosition, Panel};

use crate::app::BenchApp;

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
}

impl Center {
    /// A page with its own title and a close in the header (not a thread, not a document).
    pub fn is_settings(self) -> bool {
        matches!(self, Center::Providers | Center::NewProject)
    }
}

impl BenchApp {
    /// The panel a dock shows now. The thread is beside something else than itself only: with
    /// the center showing it, the right dock has nothing to show.
    pub fn shown_panel(&self, dock: DockPosition) -> Option<Panel> {
        let d = match dock {
            DockPosition::Left => &self.left_dock,
            DockPosition::Right => &self.right_dock,
            DockPosition::Bottom => &self.bottom_dock,
        };
        d.active_panel()
            .filter(|p| !(*p == Panel::Thread && self.center == Center::Chat))
    }

    /// A panel drawn, by its own module.
    pub(crate) fn render_panel(
        &mut self,
        panel: Panel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match panel {
            Panel::Threads => self.render_threads_panel(window, cx),
            Panel::Explorer => self.render_explorer_panel(window, cx),
            Panel::Thread => self.render_thread_panel(window, cx),
            Panel::Terminal => self.render_terminal_panel(window, cx),
        }
    }

    /// ⌘J, a document's thread button: the thread beside the code, or not. The center showing
    /// the thread itself, there is nothing to put beside it.
    pub fn toggle_thread_beside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.center == Center::Chat {
            return;
        }
        self.right_dock.toggle_panel(Panel::Thread);
        if self.right_dock.shows(Panel::Thread) {
            self.focus_composer(window, cx);
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
