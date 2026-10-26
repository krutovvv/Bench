//! The terminal pane in the browser build: a note instead of a terminal (src/ui/terminal.rs
//! draws the real one on the desktop).

use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::terminal::session::Palette;

pub fn palette(_theme: &Theme) -> Palette {
    Palette
}

impl BenchApp {
    pub(crate) fn render_terminal(
        &mut self,
        _ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("Terminals run in the desktop app — this is the browser demo.")
            .into_any_element()
    }
}
