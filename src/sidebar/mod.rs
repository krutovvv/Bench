//! The threads panel (the left dock's), as Claude's sidebar: "New thread" and "Search" over the
//! list (`search.rs`, ⌘K), the projects, each with its threads, the Host first. The left dock's
//! foot, under this panel or the Explorer's, is the agent and the gear (`agent.rs`).

mod agent;
mod search;
pub(crate) mod state;
pub(crate) mod items;
pub(crate) mod marks;
pub(crate) mod order;
mod menu;
mod project_list;
mod project_row;
mod telegram;
mod thread_row;

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use crate::sidebar::order::DraggedProject;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The left dock's threads panel (`Panel::Threads`): the search, then the projects and their
    /// threads. The dock's strip over it is the dock's (`top.rs`).
    pub(crate) fn render_threads_panel(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let mut list: Vec<AnyElement> = vec![];
        // Projects, each with its threads: the sidebar's only unit. Host first, always: this
        // machine's own project, with the threads of no project, Telegram's and cron's; then
        // the projects in the person's order (`items.rs` — ↑ ↓ walk the same order).
        let (loose, groups) = self.sidebar_groups();
        list.push(self.render_host(loose, cx));
        let last = groups.last().map(|(p, _)| p.id.clone());
        for (g, (p, rows)) in groups.into_iter().enumerate() {
            list.push(self.render_project(g + 1, &p, rows, cx));
        }
        list.extend(last.map(|id| self.last_place_zone(id, cx)));

        v_flex()
            .size_full()
            // What is made and found, over the list, as Claude's "New" and "Search".
            .child(self.render_actions(cx))
            .child(
                v_flex()
                    .id("threads")
                    // After a click on a thread: ↑ ↓ open the next, ⌫ deletes (`sidebar_items.rs`).
                    .track_focus(&self.sidebar.focus)
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        this.sidebar_key(&ev.keystroke, window, cx)
                    }))
                    // Out of the list, the project in the air lands nowhere: no line says so.
                    .on_drag_move::<DraggedProject>(cx.listener(
                        |this, ev: &DragMoveEvent<DraggedProject>, _, cx| {
                            if !ev.bounds.contains(&ev.event.position) && this.sidebar.drop.take().is_some() {
                                cx.notify();
                            }
                        },
                    ))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    // Room under the last group before the foot's hairline.
                    .pt(Gap::Xs)
                    .pb(Gap::L)
                    .children(list)
                    // This Mac's agent not up yet, and nothing to list.
                    .when(self.local_conn().is_none() && self.threads.list.is_empty(), |d| {
                        d.child(
                            div()
                                // Where the rows' words start, as every line of the sidebar.
                                .pl(px(bench_ui::SIDEBAR_TEXT_X))
                                .pr(Gap::Xl)
                                .py(Gap::Xs)
                                .text_size(px(bench_ui::SIDEBAR_TEXT))
                                .text_color(muted)
                                .child("Connecting…"),
                        )
                    }),
            )
            .into_any_element()
    }

    /// The rows over the list: a new thread (where a thread lives now), a new project, the search.
    fn render_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let new_project = self.new_project.form.is_some();
        let finding = self.sidebar.finding.is_some();
        v_flex()
            .flex_none()
            .pt(Gap::M)
            // Air under them before the first group, as Claude's before its sections.
            .pb(Gap::L)
            .gap(Gap::Hair)
            // "+", as each project's head has it: the same action, the same glyph (Claude's "New").
            .child(
                bench_ui::nav_row("nav-new-thread", Ico::Plus, "New thread", "⌘N", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.new_thread_here(cx);
                        this.focus_composer(window, cx);
                    })),
            )
            .child(
                bench_ui::nav_row("nav-new-project", Ico::FolderPlus, "New project", "", new_project, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.new_project_form(window, cx))),
            )
            .child(
                bench_ui::nav_row("nav-search", Ico::Search, "Search", "⌘K", finding, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx))),
            )
            .into_any_element()
    }

    pub(crate) fn sidebar_note(&self, text: &'static str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .pl(px(bench_ui::SIDEBAR_TEXT_X))
            .pr(Gap::Xl)
            .py(Gap::Xs)
            .text_size(px(bench_ui::SIDEBAR_TEXT))
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .into_any_element()
    }
}
