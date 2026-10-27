//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

pub(crate) mod items;
pub(crate) mod marks;
mod menu;
mod project_list;
mod project_row;
mod telegram;
mod thread_row;
mod top;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use self::menu::item_menu;
use crate::app::BenchApp;
use crate::sidebar::items::Item;
use crate::workspace::Center;
use bench_ui::icons::Ico;

/// A session title: the sidebar's 14px.
pub(crate) const SESSION_TITLE: f32 = 14.;

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
        let searching = !self.search_query.is_empty();
        if searching {
            match &self.search_hits {
                None => list.push(self.sidebar_note("Searching…", cx)),
                Some(hits) if hits.is_empty() => list.push(self.sidebar_note("Nothing found", cx)),
                Some(hits) => {
                    for (k, (id, snippet)) in hits.iter().enumerate() {
                        list.push(
                            self.render_search_hit(k, id, snippet, cx)
                                .into_any_element(),
                        );
                    }
                }
            }
        } else {
            // Projects, each with its threads: the sidebar's only unit. Host first, always: this
            // machine's own project, with the threads of no project, Telegram's and cron's; then
            // the projects, pinned ones first (`sidebar_items.rs` — ↑ ↓ walk the same order).
            let (loose, groups) = self.sidebar_groups();
            list.extend(self.render_loose(loose, cx));
            for (g, (p, rows)) in groups.into_iter().enumerate() {
                list.extend(self.render_project(g + 1, &p, rows, cx));
            }
            if self.projects.is_empty() && !self.connecting {
                list.push(self.sidebar_note("No projects yet. “New project” above starts one.", cx));
            }
        }

        v_flex()
            .size_full()
            // The search as a band, then the groups as an IDE's tree (a new chat: "+" on a
            // group, or in the top strip).
            .child(bench_ui::search_band(&self.search, cx))
            .child(
                v_flex()
                    .id("sessions")
                    // After a click on a thread: ↑ ↓ open the next, ⌫ deletes (`sidebar_items.rs`).
                    .track_focus(&self.sidebar_focus)
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        this.sidebar_key(&ev.keystroke, window, cx)
                    }))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(list)
                    .when(self.connecting, |d| {
                        d.child(
                            div()
                                .px_4()
                                .py_2()
                                .text_sm()
                                .text_color(muted)
                                .child("Connecting…"),
                        )
                    }),
            )
            .into_any_element()
    }

    /// Host: the machine's own agent and environment, for looking after the machine itself (what every project's threads get too) and
    /// the threads of no project; first in the sidebar, folded until opened.
    fn render_loose(&mut self, rows: Vec<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let host_color: Option<Hsla> = self
            .mark_of(&format!("p:{}", crate::app::model::ProjectKey::HOST))
            .color
            .and_then(|c| crate::sidebar::marks::MARK_COLORS.get(c))
            .map(|c| rgb(*c).into());
        let key = "open:loose".to_string();
        let open = self.ssh.collapsed.contains(&key);
        let group: SharedString = "loose-head".into();
        let mut out = vec![
            h_flex()
                .id("loose-head")
                .group(group.clone())
                .cursor_pointer()
                .px_4()
                .pt(rems(0.5))
                .pb_1()
                .gap_1p5()
                .items_center()
                // Small caps, as every project's head (`project_row`): HOST is a group too.
                .text_xs()
                .child(bench_ui::head_icon_in(Ico::Server, host_color, cx))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(host_color.unwrap_or(muted))
                        .child(crate::app::LOCAL_HOST.to_uppercase()),
                )
                .child(div().flex_1())
                // "⋯" (on hover), as a right click: its environment, its color — the same menu as
                // a project's, without pin and remove (it is always first, and always there).
                .child(
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new("general-more")
                                .ghost()
                                .xsmall()
                                .icon(Ico::Ellipsis.icon())
                                .dropdown_menu(item_menu(
                                    cx.entity(),
                                    Item::Project(crate::app::model::ProjectKey::HOST.into()),
                                    false,
                                )),
                        ),
                )
                // The gear (on hover), as on a project's row: this Mac's environment, in the
                // Explorer.
                .child(
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new("general-set")
                                .ghost()
                                .xsmall()
                                .icon(Ico::Settings.icon())
                                .tooltip("Settings")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.open_project_settings(
                                        crate::app::model::ProjectKey::HOST.into(),
                                        window,
                                        cx,
                                    )
                                })),
                        ),
                )
                .child(
                    bench_ui::head_plus_slot()
                        .id("general-new")
                        .cursor_pointer()
                        .text_color(muted)
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .hover(|s| {
                            s.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().foreground)
                        })
                        .child(Ico::Plus.icon().xsmall())
                        .tooltip(|window, cx| Tooltip::new("New thread").build(window, cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.new_thread_in(String::new(), cx)
                        })),
                )
                .child(bench_ui::group_chevron(!open, group, cx))
                // Folded by default: the key marks it open.
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)))
                .context_menu(item_menu(
                    cx.entity(),
                    Item::Project(crate::app::model::ProjectKey::HOST.into()),
                    false,
                ))
                .into_any_element(),
        ];
        if !open {
            return out;
        }
        let n = rows.len();
        for (k, i) in rows.into_iter().enumerate() {
            out.push(
                self.render_session_row(i, Some(k + 1 == n), cx)
                    .into_any_element(),
            );
        }
        out
    }

    fn sidebar_note(&self, text: &'static str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .px_4()
            .py_2()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .into_any_element()
    }

    /// A search result: the session's title (if it is in the list) and the matching snippet.
    fn render_search_hit(
        &self,
        k: usize,
        id: &str,
        snippet: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected =
            self.center == Center::Chat && self.thread(cx).session.as_deref() == Some(id);
        let title = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.title.clone())
            .unwrap_or_else(|| "Untitled session".into());
        let open_id = id.to_string();
        v_flex()
            .id(("hit", k))
            .pl(px(14.))
            .pr_3()
            .py_2()
            .cursor_pointer()
            .border_l_2()
            .border_color(if selected {
                theme.primary
            } else {
                theme.transparent
            })
            .when(selected, |d| d.bg(theme.sidebar_accent))
            .hover(|s| s.bg(theme.sidebar_accent))
            .child(div().text_size(px(SESSION_TITLE)).truncate().child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .line_clamp(2)
                    .child(snippet.replace(">>>", "").replace("<<<", "")),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open_session(open_id.clone(), cx)))
    }
}
