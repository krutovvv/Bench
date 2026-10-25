//! The terminal in the browser build: there is no shell or PTY to run, so a session can't be
//! started (SSH mode says so). Same names as src/term.rs, which every desktop build uses.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;

/// The host name of the terminal on this machine.
pub const LOCAL: &str = "";

#[derive(Clone, Copy, Default)]
pub struct Palette;

pub struct Session {
    pub exited: AtomicBool,
    pub exit_note: std::sync::Mutex<Option<String>>,
}

impl Session {
    /// A tab opens (its pane says terminals run in the desktop app); nothing runs.
    pub fn spawn(
        _host: &str,
        _palette: Palette,
        _wake: async_channel::Sender<()>,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Session {
            exited: AtomicBool::new(false),
            exit_note: std::sync::Mutex::new(None),
        }))
    }

    pub fn title(&self) -> Option<String> {
        None
    }

    pub fn set_palette(&self, _palette: Palette) {}

    pub fn input(&self, _bytes: &[u8]) {}

    pub fn kill(&self) {}

    pub fn write(&self, _bytes: &[u8]) {}

    pub fn layout(&self) -> Layout {
        Layout::default()
    }

    pub fn screen_text(&self, _n: usize) -> String {
        String::new()
    }

    pub fn set_min_rows(&self, _rows: usize) {}

    pub fn used_rows(&self) -> usize {
        0
    }
}

/// Where the cells are on screen (nothing is drawn here).
#[derive(Clone, Copy, Default, Debug)]
pub struct Layout {
    pub origin_x: f32,
    pub origin_y: f32,
    pub cell_w: f32,
    pub line_h: f32,
}

/// A link under the pointer (none in the browser build).
pub struct TermLink {
    pub target: String,
    pub line: Option<usize>,
    pub cwd: Option<String>,
}
