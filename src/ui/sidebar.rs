//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::plural;
use crate::app::{HermesApp, Panel};
use crate::icons::Ico;
use crate::model::source_label;

impl HermesApp {
    pub(super) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        // Group visible sessions: by git project when they work in one, else by where they live
        // (Hermes UI/CLI, Telegram, cron…). Plain Hermes chats first, then projects, then the rest.
        let mut groups: Vec<(String, Vec<usize>)> = vec![];
        let mut rank: Vec<(String, u8)> = vec![];
        for i in self.visible_sessions() {
            let (label, r) = match self.project_of(&self.sessions[i].id) {
                Some(p) => (p.name(), 1),
                None => {
                    let l = source_label(&self.sessions[i].source);
                    let r = if l == "Hermes" { 0 } else { 2 };
                    (l, r)
                }
            };
            match groups.iter_mut().find(|(l, _)| *l == label) {
                Some((_, rows)) => rows.push(i),
                None => {
                    rank.push((label.clone(), r));
                    groups.push((label, vec![i]));
                }
            }
        }
        let rank_of = |l: &str| {
            rank.iter()
                .find(|(x, _)| x == l)
                .map(|(_, r)| *r)
                .unwrap_or(2)
        };
        groups.sort_by(|(a, _), (b, _)| {
            (rank_of(a), a.to_lowercase()).cmp(&(rank_of(b), b.to_lowercase()))
        });
        let single = groups.len() <= 1;

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
            groups.clear();
        }
        for (label, rows) in groups {
            if !single {
                list.push(
                    div()
                        .px_4()
                        .pt_4()
                        .pb_1()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("{} · {}", label.to_uppercase(), rows.len()))
                        .into_any_element(),
                );
            }
            for i in rows {
                list.push(self.render_session_row(i, cx).into_any_element());
            }
        }

        let new_session = h_flex()
            .id("new-session")
            .h(px(32.))
            .px_3()
            .gap_2()
            .items_center()
            .justify_center()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .cursor_pointer()
            .hover(|s| s.bg(theme.primary_hover))
            .child(Ico::Plus.icon().small())
            .child("New session")
            .on_click(cx.listener(|this, _, _, cx| this.new_session(cx)));

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(
                // Mirrors the footer: same 8px inset, same 32px control, a rule on the inner side.
                div().pt(px(38.)).flex_none().child(
                    h_flex()
                        .h(px(super::BAR_H))
                        .px_4()
                        .items_center()
                        .border_b_1()
                        .border_color(theme.sidebar_border)
                        .child(
                            Input::new(&self.search)
                                .h(px(32.))
                                .cleanable(true)
                                .prefix(Ico::Search.icon().small().text_color(muted)),
                        ),
                ),
            )
            .child(
                v_flex()
                    .id("sessions")
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
            .child(
                // Same height as the composer, so the two top rules line up across the window.
                h_flex()
                    .flex_none()
                    .h(px(super::BAR_H))
                    .px_4()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(new_session.flex_1()),
            )
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
        let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(id);
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
            .child(div().text_sm().truncate().child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .line_clamp(2)
                    .child(snippet.replace(">>>", "").replace("<<<", "")),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open_session(open_id.clone(), cx)))
    }

    fn render_session_row(&self, i: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let s = &self.sessions[i];
        let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(&s.id);
        let armed = self.confirm_delete.as_deref() == Some(&s.id);
        let group: SharedString = format!("srow{i}").into();
        let count = if s.running {
            "working…".to_string()
        } else {
            plural(s.messages as usize, "message", "messages", "messages")
        };
        // A chat in a branch of its own says which.
        let meta = match self.project_of(&s.id).filter(|p| p.in_branch_folder()) {
            Some(p) if !p.branch.is_empty() => format!("{} · {count}", p.branch),
            _ => count,
        };
        let (open_id, del_id) = (s.id.clone(), s.id.clone());

        let delete = if armed {
            h_flex()
                .gap_1()
                .text_xs()
                .child(
                    div()
                        .id(("del-yes", i))
                        .px_1p5()
                        .py_0p5()
                        .bg(theme.danger)
                        .text_color(theme.danger_foreground)
                        .cursor_pointer()
                        .child("Delete")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.delete_session(del_id.clone(), window, cx)
                        })),
                )
                .child(
                    div()
                        .id(("del-no", i))
                        .px_1()
                        .py_0p5()
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(|s| s.text_color(theme.foreground))
                        .child("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.cancel_delete(cx)
                        })),
                )
                .into_any_element()
        } else {
            div()
                .id(("del", i))
                .p_1()
                .cursor_pointer()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                .text_color(muted)
                .hover(|s| s.text_color(theme.danger))
                .child(Ico::Trash.icon().xsmall())
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.delete_session(del_id.clone(), window, cx)
                }))
                .into_any_element()
        };

        h_flex()
            .id(("session", i))
            .group(group)
            .gap_1()
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
            .when(selected, |d| {
                d.bg(theme.sidebar_accent)
                    .text_color(theme.sidebar_accent_foreground)
            })
            .hover(|s| s.bg(theme.sidebar_accent))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(s.title.clone()))
                    .child(div().text_xs().text_color(muted).child(meta)),
            )
            .child(delete)
            .on_click(cx.listener(move |this, _, _, cx| this.open_session(open_id.clone(), cx)))
    }
}
