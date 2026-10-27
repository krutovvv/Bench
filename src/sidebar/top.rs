//! The strip over the sidebar, beside the window's buttons: which panel the left dock shows,
//! a new project, and the fold.

use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The strip beside the window buttons over the sidebar: which panel the sidebar shows
    /// (threads, Explorer ⌘E), "+" (a new project) and the fold. 35 + the search band's top rule =
    /// the header's 36: the two rules are one line.
    pub(crate) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let explorer = self.sidebar_panel == crate::workspace::SidebarPanel::Explorer;
        h_flex()
            .h(bench_ui::bar_h(cx))
            .flex_none()
            .justify_end()
            .items_center()
            .gap_1()
            .pr(px(bench_ui::SIDEBAR_STRIP_PR))
            .child(
                bench_ui::icon_button(
                    "side-threads",
                    bench_ui::icons::Ico::MessagesSquare,
                    "Threads  ⌘E",
                    !explorer,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if explorer {
                        this.toggle_side(window, cx)
                    }
                })),
            )
            .child(
                bench_ui::icon_button(
                    "side-explorer",
                    bench_ui::icons::Ico::FolderTree,
                    "Explorer  ⌘E",
                    explorer,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if !explorer {
                        this.toggle_side(window, cx)
                    }
                })),
            )
            .child(div().w(bench_ui::Gap::M.px()))
            .child(
                bench_ui::icon_button(
                    "new-item",
                    bench_ui::icons::Ico::Plus,
                    "New project",
                    false,
                    cx,
                )
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

