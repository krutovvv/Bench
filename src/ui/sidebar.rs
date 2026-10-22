//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

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

        // Group visible sessions by where they live (Hermes UI/CLI, Telegram, cron…), Hermes first.
        let mut groups: Vec<(String, Vec<usize>)> = vec![];
        for i in self.visible_sessions() {
            let label = source_label(&self.sessions[i].source);
            match groups.iter_mut().find(|(l, _)| *l == label) {
                Some((_, rows)) => rows.push(i),
                None => groups.push((label, vec![i])),
            }
        }
        groups.sort_by_key(|(l, _)| l != "Hermes");
        let single = groups.len() <= 1;

        let mut list: Vec<AnyElement> = vec![];
        for (label, rows) in groups {
            if !single {
                list.push(
                    div()
                        .px_3()
                        .pt_3()
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
                v_flex()
                    .pt(px(38.))
                    .px_3()
                    .pb_3()
                    .border_b_1()
                    .border_color(theme.sidebar_border)
                    .child(new_session),
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
                                .px_3()
                                .py_2()
                                .text_sm()
                                .text_color(muted)
                                .child("Connecting…"),
                        )
                    }),
            )
    }

    fn render_session_row(&self, i: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let s = &self.sessions[i];
        let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(&s.id);
        let armed = self.confirm_delete.as_deref() == Some(&s.id);
        let group: SharedString = format!("srow{i}").into();
        let meta = if s.running {
            "working…".to_string()
        } else {
            plural(s.messages as usize, "message", "messages", "messages")
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
            .px_3()
            .py_1p5()
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
