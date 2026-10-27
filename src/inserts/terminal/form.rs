//! The header's right end: a new window.

use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The header's right end: a new window.
    pub(crate) fn render_header_tail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().flex_none().gap_1().items_center().child(
            bench_ui::icon_button("new-window", Ico::AppWindow, "New window  ⌘⇧N", false, cx)
                .on_click(cx.listener(|this, _, _, cx| this.open_new_window(cx))),
        )
    }
}
