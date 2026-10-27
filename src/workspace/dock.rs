//! A dock, as Zed's: a side of the window holding panels, one shown at a time, open or folded, at
//! a size of its own. The left dock holds the threads and the Explorer's tree; the right one the
//! thread beside a document; the bottom one what runs under the center (the project's terminal). A new panel is a `Panel` variant, its dock's list, and its drawing in
//! `BenchApp::render_panel` — nothing else changes.

use gpui_kit::*;

/// Where a dock is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPosition {
    Left,
    Right,
    Bottom,
}

impl DockPosition {
    /// Its size at first and how far it can be dragged (a width; the bottom's a height).
    pub fn size(self) -> bench_ui::DockSize {
        match self {
            DockPosition::Left => bench_ui::LEFT_DOCK,
            DockPosition::Right => bench_ui::RIGHT_DOCK,
            DockPosition::Bottom => bench_ui::BOTTOM_DOCK,
        }
    }
}

/// What a dock can show (Zed's `Panel`). Each draws in its own module (`render_panel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// Projects and their threads (`src/sidebar/mod.rs`).
    Threads,
    /// One project's tree (`src/explorer/tree.rs`).
    Explorer,
    /// The open thread, beside a document (`src/thread/mod.rs` `render_thread_panel`; the
    /// center's own when it shows the thread).
    Thread,
    /// A terminal on the project's machine, in its folder (`src/inserts/terminal/panel.rs`).
    Terminal,
}

/// A side of the window: its panels, the one shown, whether it is open, its size.
pub struct Dock {
    pub position: DockPosition,
    panels: Vec<Panel>,
    active: usize,
    pub open: bool,
    /// As last dragged; None: the position's own.
    pub size: Option<Pixels>,
}

impl Dock {
    pub fn new(position: DockPosition, panels: &[Panel], open: bool) -> Self {
        Self {
            position,
            panels: panels.to_vec(),
            active: 0,
            open,
            size: None,
        }
    }

    /// The panel it shows; None while folded or empty.
    pub fn active_panel(&self) -> Option<Panel> {
        self.open
            .then(|| self.panels.get(self.active).copied())
            .flatten()
    }

    /// Whether it shows `panel` now.
    pub fn shows(&self, panel: Panel) -> bool {
        self.active_panel() == Some(panel)
    }

    /// Show `panel` (one of its own), opening the dock.
    pub fn activate(&mut self, panel: Panel) {
        if let Some(at) = self.panels.iter().position(|p| *p == panel) {
            self.active = at;
            self.open = true;
        }
    }

    /// Its active panel shown, or folded (⌘\ for the left dock).
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// `panel` shown, or the dock folded when it shows it already (⌘J for the thread beside).
    pub fn toggle_panel(&mut self, panel: Panel) {
        if self.shows(panel) {
            self.open = false;
        } else {
            self.activate(panel);
        }
    }

    /// Its size now, in pixels.
    pub fn size(&self) -> Pixels {
        self.size.unwrap_or(px(self.position.size().start))
    }

    /// The range it can be dragged within.
    pub fn size_range(&self) -> std::ops::Range<Pixels> {
        let s = self.position.size();
        px(s.min)..px(s.max)
    }
}

#[cfg(test)]
mod tests {
    use super::{Dock, DockPosition, Panel};

    #[test]
    fn a_dock_shows_one_of_its_panels_while_open() {
        let mut left = Dock::new(DockPosition::Left, &[Panel::Threads, Panel::Explorer], true);
        assert_eq!(left.active_panel(), Some(Panel::Threads));
        left.activate(Panel::Explorer);
        assert!(left.shows(Panel::Explorer));
        // Not its panel: nothing changes.
        left.activate(Panel::Thread);
        assert!(left.shows(Panel::Explorer));
        left.toggle();
        assert_eq!(left.active_panel(), None);
        left.toggle();
        assert!(
            left.shows(Panel::Explorer),
            "folded and back, the same panel"
        );

        let mut right = Dock::new(DockPosition::Right, &[Panel::Thread], false);
        right.toggle_panel(Panel::Thread);
        assert!(right.shows(Panel::Thread));
        right.toggle_panel(Panel::Thread);
        assert_eq!(right.active_panel(), None);

        let bottom = Dock::new(DockPosition::Bottom, &[], true);
        assert_eq!(bottom.active_panel(), None, "an empty dock shows nothing");
    }
}
