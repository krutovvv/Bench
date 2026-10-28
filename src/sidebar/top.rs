//! The strip over the sidebar, beside the window's buttons: the fold (a new project is the last
//! row of the projects, `mod.rs`). The
//! Explorer's button is not here: it is in the window's top-right corner (`src/titlebar`), so it
//! is one hop from any thread and stays there while its tree is shown.

use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The strip beside the window buttons over the sidebar: the fold, in its right corner — the
    /// left is the window buttons', and nothing lines up with them. Its icon ends at the
    /// sidebar's inset.
    pub(crate) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(bench_ui::bar_h(cx))
            .flex_none()
            .items_center()
            .justify_end()
            .gap_1()
            .pr(px(bench_ui::SIDEBAR_STRIP_PR))
            .child(
                bench_ui::icon_button(
                    "sidebar-hide",
                    bench_ui::icons::Ico::PanelLeft,
                    "Hide sidebar  ⌘\\",
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .into_any_element()
    }
}
