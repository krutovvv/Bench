//! The bar over the center: the thread's title (its meters, project and repo are the composer's
//! foot, `thread/composer/foot.rs`), or the documents' project (their tabs are the center's own
//! bar, `explorer/pane/view.rs`), then Chat | Code at its right end (`tail.rs`); or a settings
//! page's title and its close.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;

mod tail;

impl BenchApp {
    /// The top bar's lead, left of the center's header: room for the window's buttons, the
    /// sidebar's fold (hide or show, ⌘\), a short rule; the title follows it, as a Mac's
    /// unified title bar has it, whether the sidebar is shown or not.
    pub(crate) fn render_window_lead(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let shown = self.shown_panel(crate::workspace::DockPosition::Left).is_some();
        h_flex()
            .flex_none()
            .h(bench_ui::header_h(cx))
            .pl(px(bench_ui::WINDOW_BUTTONS_W))
            .gap_2()
            .items_center()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .child(
                bench_ui::icon_button(
                    "sidebar-fold",
                    bench_ui::icons::Ico::PanelLeft,
                    if shown { "Hide sidebar  ⌘\\" } else { "Show sidebar  ⌘\\" },
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .child(
                div()
                    .w(px(bench_ui::HAIRLINE))
                    .h(px(bench_ui::BAR_SEPARATOR_H))
                    .bg(theme.border),
            )
            .into_any_element()
    }

    /// Header strip: the session title, or the settings tabs.
    pub(crate) fn render_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let bar = h_flex()
            .id("header")
            .h(bench_ui::header_h(cx))
            .flex_none()
            .bg(theme.title_bar)
            .items_center()
            .border_b_1()
            .border_color(theme.border);

        // Documents: the header names the project they belong to; their tabs are the center's
        // own bar under it (as Zed's pane), not the window's title bar, which drags the window.
        if self.center == Center::Doc {
            let name = self.project_name(&self.explorer.project);
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // The header's title: the sidebar's size, semibold; its chips the bars' size.
                        .text_size(px(bench_ui::BAR_TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(name),
                )
                .child(self.render_header_tail(cx))
                .into_any_element();
        }
        // Settings and the new project's page: their own close sits here.
        if !self.center.is_settings() {
            let title: SharedString = self
                .thread(cx)
                .id
                .as_ref()
                .and_then(|id| self.threads.list.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .filter(|t| !t.is_empty())
                // A thread about a task list is named after it until it has a title of its own.
                .unwrap_or_else(|| "New thread".into())
                .into();
            // A second thread beside this one: its title after a slash, a quieter one, with its
            // close — the two threads named in one bar, not a second bar over the dock.
            let beside = (self.center == Center::Chat
                && self.right_dock.shows(crate::workspace::Panel::Thread))
            .then(|| self.beside_thread.clone())
            .flatten()
            .map(|v| self.thread_title(v.read(cx).thread.id.as_ref()));
            let theme = cx.theme().clone();
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1p5()
                        .items_center()
                        // The header's title: the sidebar's size, semibold; its chips the bars' size.
                        .text_size(px(bench_ui::BAR_TITLE))
                        .child(
                            div()
                                .min_w_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(title),
                        )
                        .when_some(beside, |d, second| {
                            d.child(div().flex_none().text_color(bench_ui::faint(&theme)).child("/"))
                                .child(
                                    div()
                                        .min_w_0()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(second),
                                )
                                .child(
                                    bench_ui::icon_button(
                                        "beside-close",
                                        bench_ui::icons::Ico::X,
                                        "Close the thread beside  ⌘J",
                                        false,
                                        cx,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_beside(cx);
                                        this.right_dock.open = false;
                                    })),
                                )
                        }),
                )
                .child(self.render_header_tail(cx))
                .into_any_element();
        }

        let title: SharedString = if self.center == Center::NewProject { "New project" } else { "Settings" }.into();
        bar.child(
            div()
                .px_4()
                .text_size(px(bench_ui::BAR_TITLE))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("settings-close")
                .px_3()
                .h_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .child(bench_ui::icons::Ico::X.icon().small())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.center = Center::Chat;
                    this.cancel_project_form(cx);
                })),
        )
        .into_any_element()
    }
}
