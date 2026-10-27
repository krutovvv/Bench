//! The strip over the sidebar, beside the window's buttons: the fold and a new project. The
//! Explorer's button is not here: it is in the window's top-right corner (`src/titlebar`), so it
//! is one hop from any thread and stays there while its tree is shown.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The strip beside the window buttons over the sidebar: the fold and "New project".
    /// The fold takes the place the window buttons leave, and the new project is spelled out
    /// in the far corner — where the old lone plus stood: a bare plus there says nothing about
    /// what it adds. Same ghost button as the status bar's items.
    /// 35 + the search band's top rule = the header's 36: the two rules are one line.
    pub(crate) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(bench_ui::bar_h(cx))
            .flex_none()
            .items_center()
            // Past the traffic lights, which overlay this strip.
            .pl(px(76.))
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
            .child(
                Button::new("new-item")
                    .ghost()
                    .small()
                    .icon(bench_ui::icons::Ico::FolderPlus.icon())
                    .child(div().text_xs().child("New project"))
                    .text_color(cx.theme().muted_foreground)
                    .ml_auto()
                    .mr(px(6.))
                    .on_click(cx.listener(|this, _, window, cx| this.new_project_form(window, cx))),
            )
            .into_any_element()
    }
}
