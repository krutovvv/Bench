//! The strip over the sidebar, beside the window's buttons: a new project (or, in the Explorer,
//! its pressed button that closes it), and the fold.

use gpui_kit::component::h_flex;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The strip beside the window buttons over the sidebar: "+" (a new project) and the fold;
    /// in the IDE (the Explorer) its button, pressed, which closes it, instead of "+".
    /// 35 + the search band's top rule = the header's 36: the two rules are one line.
    pub(crate) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ide = self.sidebar_panel == crate::workspace::SidebarPanel::Explorer;
        let first = if ide {
            bench_ui::icon_button(
                "ide-close",
                bench_ui::icons::Ico::FolderTree,
                "Close the Explorer  ⌘E",
                true,
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| this.close_explorer(cx)))
        } else {
            bench_ui::icon_button(
                "new-item",
                bench_ui::icons::Ico::Plus,
                "New project",
                false,
                cx,
            )
            .on_click(cx.listener(|this, _, window, cx| this.new_project_form(window, cx)))
        };
        h_flex()
            .h(bench_ui::bar_h(cx))
            .flex_none()
            .justify_end()
            .items_center()
            .gap_1()
            .pr(px(bench_ui::SIDEBAR_STRIP_PR))
            .child(first)
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
