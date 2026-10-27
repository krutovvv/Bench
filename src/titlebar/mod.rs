//! The bar over the center: the thread's title and its project's branch, the documents' project
//! (their tabs are the center's own bar, `explorer/view.rs`) with the thread-beside button, or a
//! settings page's title and its close.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;

impl BenchApp {
    /// The header's right end, the window's top-right corner: with a document in the center, the
    /// button that shows the thread beside it (⌘J). Shown, the thread's own head takes this corner
    /// with its + and ×.
    fn render_header_tail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let shown = self
            .shown_panel(crate::workspace::DockPosition::Right)
            .is_some();
        h_flex()
            .flex_none()
            .gap_1()
            .items_center()
            .when(self.center == Center::Doc && !shown, |d| {
                d.child(
                    bench_ui::icon_button(
                        "thread-beside",
                        bench_ui::icons::Ico::MessagesSquare,
                        "The thread beside  ⌘J",
                        false,
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.toggle_thread_beside(window, cx)),
                    ),
                )
            })
            .into_any_element()
    }

    /// "app · main": the thread works in a git repository (its folder's name and branch). Git
    /// itself is the agent's: ask it to commit, or to undo.
    fn render_project_chip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.thread(cx).project.clone()?;
        let label = if p.branch.is_empty() {
            p.name()
        } else {
            format!("{} · {}", p.name(), p.branch)
        };
        let theme = cx.theme();
        Some(
            h_flex()
                .flex_none()
                .gap_1()
                .px_1()
                .items_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(bench_ui::icons::Ico::GitBranch.icon().small())
                .child(label)
                .into_any_element(),
        )
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
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(name),
                )
                .child(self.render_header_tail(cx))
                .into_any_element();
        }
        if !self.center.is_settings() {
            let title: SharedString = self
                .thread(cx)
                .session
                .as_ref()
                .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .filter(|t| !t.is_empty())
                // A thread about a task list is named after it until it has a title of its own.
                .unwrap_or_else(|| "New session".into())
                .into();
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(title),
                )
                .children(self.render_project_chip(cx))
                .child(self.render_header_tail(cx))
                .into_any_element();
        }

        let title: SharedString = match self.center {
            Center::NewProject => "New project".into(),
            _ => "Settings".into(),
        };
        bar.child(
            div()
                .px_4()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
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
                    this.project_form = None;
                    cx.notify();
                })),
        )
        .into_any_element()
    }
}
