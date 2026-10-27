//! The strip over the sidebar, beside the window's buttons: a new project and the fold. The
//! Explorer's button is not here: it is in the window's top-right corner (`src/titlebar`), so it
//! is one hop from any thread and stays there while its tree is shown.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::h_flex;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The strip beside the window buttons over the sidebar: "New project" and the fold.
    /// The new project is spelled out, not a lone plus: a bare plus by the window buttons
    /// says nothing about what it adds. Same ghost button as the status bar's items.
    /// 35 + the search band's top rule = the header's 36: the two rules are one line.
    pub(crate) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(bench_ui::bar_h(cx))
            .flex_none()
            .justify_end()
            .items_center()
            .gap_1()
            .pr(px(bench_ui::SIDEBAR_STRIP_PR))
            .child(
                Button::new("new-item")
                    .ghost()
                    .small()
                    .child(div().text_xs().child("New project"))
                    .text_color(cx.theme().muted_foreground)
                    .on_click(cx.listener(|this, _, window, cx| this.new_project_form(window, cx))),
            )
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
