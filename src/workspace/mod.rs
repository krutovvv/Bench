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
}

impl Center {
    /// A page with its own title and a close in the header (not a thread, not a document).
    pub fn is_settings(self) -> bool {
        matches!(self, Center::Providers)
    }
}

impl BenchApp {
    /// The panel a dock shows now. Beside a thread the right dock's thread is a second one
    /// (`beside_thread`): with none opened, it has nothing to show.
    pub fn shown_panel(&self, dock: DockPosition) -> Option<Panel> {
        let d = match dock {
            DockPosition::Left => &self.left_dock,
            DockPosition::Right => &self.right_dock,
            DockPosition::Bottom => &self.bottom_dock,
        };
        d.active_panel().filter(|p| {
            !(*p == Panel::Thread && self.center == Center::Chat && self.beside_thread.is_none())
        })
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
    /// a thread, the second thread beside it (`open_beside`), when one was opened.
    pub fn toggle_thread_beside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.center == Center::Chat {
            if self.beside_thread.is_some() {
                self.right_dock.toggle_panel(Panel::Thread);
                cx.notify();
            }
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
