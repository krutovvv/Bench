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
}
