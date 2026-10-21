//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

use gpui_kit::component::select::Select;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{compact, plural};
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
            .child("Новая сессия")
            .on_click(cx.listener(|this, _, _, cx| this.new_session(cx)));

        let footer_button = |id: &'static str, icon: Ico, tip: &'static str, active: bool| {
            div()
                .id(id)
                .p_1()
                .cursor_pointer()
                .text_color(if active { theme.primary } else { muted })
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        v_flex()
            .w(px(280.))
            .h_full()
            .flex_none()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                v_flex()
                    .pt(px(40.))
                    .px_3()
                    .pb_3()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.sidebar_border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(crate::theme::logo(px(20.)))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Hermes"),
                            ),
                    )
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
                                .child("Подключаюсь…"),
                        )
                    }),
            )
            // Rarely changed: where and which profile, then the status line.
            .child(
                v_flex()
                    .flex_none()
                    .gap_1p5()
                    .px_3()
                    .py_2()
                    .bg(theme.title_bar)
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(Ico::Server.icon().xsmall().text_color(muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(Select::new(&self.target_choice).small()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(Ico::UserRound.icon().xsmall().text_color(muted))
                            .child(
                                div().flex_1().min_w_0().child(
                                    Select::new(&self.profile_choice)
                                        .small()
                                        .placeholder("Профиль"),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .pt_1()
                            .child(div().flex_1().min_w_0().child(self.render_status(cx)))
                            .child(
                                footer_button(
                                    "restart",
                                    Ico::RefreshCw,
                                    "Перезапустить шлюз Hermes",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| this.restart_gateway(window, cx),
                                )),
                            )
                            .child(
                                footer_button(
                                    "settings",
                                    Ico::Settings,
                                    "Настройки: скиллы, MCP, Telegram",
                                    self.panel.is_settings(),
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        if this.panel.is_settings() {
                                            this.panel = Panel::Chat;
                                            cx.notify();
                                        } else {
                                            this.open_panel(Panel::Skills, window, cx);
                                        }
                                    },
                                )),
                            ),
                    ),
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
            "работает…".to_string()
        } else {
            plural(s.messages as usize, "сообщение", "сообщения", "сообщений")
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
                        .child("Удалить")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.delete_session(del_id.clone(), cx)
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
                        .child("Нет")
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.delete_session(del_id.clone(), cx)
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

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        // Model: live usage, else the open session, else the profile default.
        let model = self
            .usage
            .as_ref()
            .map(|u| u.model.clone())
            .filter(|m| !m.is_empty())
            .or_else(|| {
                let id = self.session.as_ref()?;
                self.sessions
                    .iter()
                    .find(|s| &s.id == id)
                    .map(|s| s.model.clone())
            })
            .filter(|m| !m.is_empty())
            .or_else(|| {
                let p = self.profile.as_ref()?;
                self.profiles
                    .iter()
                    .find(|x| &x.name == p)
                    .map(|x| x.model.clone())
            })
            .filter(|m| !m.is_empty())
            .or_else(|| {
                self.profiles
                    .iter()
                    .find(|x| x.name == "default")
                    .map(|x| x.model.clone())
            })
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "—".into());

        let usage = self.usage.as_ref().filter(|u| u.context_max > 0);
        let pct = usage
            .map(|u| (u.context_percent / 100.0).clamp(0.0, 1.0) as f32)
            .unwrap_or(0.);
        let bar = if pct > 0.85 {
            theme.danger
        } else if pct > 0.6 {
            theme.warning
        } else {
            theme.foreground
        };
        let context = usage
            .map(|u| {
                format!(
                    "{} / {} · {:.0}%",
                    compact(u.context_used),
                    compact(u.context_max),
                    u.context_percent
                )
            })
            .unwrap_or_else(|| "контекст —".into());
        let extra = usage
            .map(|u| {
                [
                    (u.total_tokens > 0).then(|| format!("всего {}", compact(u.total_tokens))),
                    (u.compressions > 0)
                        .then(|| plural(u.compressions as usize, "сжатие", "сжатия", "сжатий")),
                    u.cache_hit_pct.map(|c| format!("кэш {c}%")),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ")
            })
            .unwrap_or_default();

        let row = |icon: Ico, text: String| {
            h_flex()
                .gap_1p5()
                .items_center()
                .h(px(18.))
                .child(icon.icon().xsmall().text_color(muted))
                .child(div().truncate().child(text))
        };

        v_flex()
            .gap_1()
            .text_xs()
            .child(row(Ico::Cpu, model))
            .child(row(Ico::MemoryStick, context))
            .child(
                div()
                    .h(px(2.))
                    .w_full()
                    .bg(theme.border)
                    .child(div().h_full().bg(bar).w(relative(pct))),
            )
            .child(div().h(px(16.)).text_color(muted).truncate().child(extra))
    }
}
