//! Left column: where, which profile, navigation, sessions, status bar.

use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{compact, plural};
use crate::app::{HermesApp, Panel};

impl HermesApp {
    pub(super) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        let nav = |id: &'static str, icon: IconName, label: &'static str, active: bool| {
            h_flex()
                .id(id)
                .gap_2()
                .px_2()
                .py_1()
                .text_sm()
                .cursor_pointer()
                .border_l_2()
                .border_color(if active {
                    theme.primary
                } else {
                    theme.transparent
                })
                .when(active, |d| d.bg(theme.sidebar_accent))
                .hover(|s| s.bg(theme.sidebar_accent))
                .child(Icon::new(icon).small().text_color(muted))
                .child(label)
        };

        let sessions = self.sessions.iter().enumerate().map(|(i, s)| {
            let id = s.id.clone();
            let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(&s.id);
            let meta = if s.running {
                "работает…".to_string()
            } else {
                plural(s.messages as usize, "сообщение", "сообщения", "сообщений")
            };
            div()
                .id(("session", i))
                .px_2()
                .py_1()
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
                .child(div().text_sm().truncate().child(s.title.clone()))
                .child(div().text_xs().text_color(muted).child(meta))
                .on_click(cx.listener(move |this, _, _, cx| this.open_session(id.clone(), cx)))
        });

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
                    .px_2()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .pb_1()
                            .child(
                                div()
                                    .size(px(20.))
                                    .bg(theme.foreground)
                                    .text_color(theme.background)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::Bot).small()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("HERMES"),
                            ),
                    )
                    .child(Select::new(&self.target_choice).small())
                    .child(
                        Select::new(&self.profile_choice)
                            .small()
                            .placeholder("Профиль"),
                    ),
            )
            .child(
                v_flex()
                    .px_2()
                    .pt_3()
                    .gap_0()
                    .child(
                        nav("nav-new", IconName::Plus, "Новая сессия", false)
                            .on_click(cx.listener(|this, _, _, cx| this.new_session(cx))),
                    )
                    .child(
                        nav(
                            "nav-skills",
                            IconName::BookOpen,
                            "Скиллы",
                            self.panel == Panel::Skills,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_panel(Panel::Skills, window, cx)
                        })),
                    )
                    .child(
                        nav(
                            "nav-mcp",
                            IconName::Network,
                            "MCP",
                            self.panel == Panel::Mcp,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_panel(Panel::Mcp, window, cx)
                        })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pt_3()
                    .gap_1()
                    .child(div().px_2().text_xs().text_color(muted).child("СЕССИИ"))
                    .child(
                        v_flex()
                            .id("sessions")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .gap_0()
                            .pb_2()
                            .children(sessions)
                            .when(self.connecting, |d| {
                                d.child(
                                    div()
                                        .px_2()
                                        .text_sm()
                                        .text_color(muted)
                                        .child("Подключаюсь…"),
                                )
                            }),
                    ),
            )
            .child(self.render_status(cx))
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
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
            .or_else(|| self.sessions.first().map(|s| s.model.clone()))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "—".into());

        let context = self.usage.as_ref().filter(|u| u.context_max > 0).map(|u| {
            let pct = (u.context_percent / 100.0).clamp(0.0, 1.0) as f32;
            let color = if pct > 0.85 {
                theme.danger
            } else if pct > 0.6 {
                theme.warning
            } else {
                theme.foreground
            };
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(Icon::new(IconName::MemoryStick).xsmall().text_color(muted))
                        .child(format!(
                            "{} / {} · {:.0}%",
                            compact(u.context_used),
                            compact(u.context_max),
                            u.context_percent
                        )),
                )
                .child(
                    div()
                        .h(px(2.))
                        .w_full()
                        .bg(theme.border)
                        .child(div().h_full().bg(color).w(relative(pct))),
                )
                .child(
                    div().text_color(muted).child(
                        [
                            (u.total_tokens > 0)
                                .then(|| format!("всего {}", compact(u.total_tokens))),
                            (u.compressions > 0).then(|| {
                                plural(u.compressions as usize, "сжатие", "сжатия", "сжатий")
                            }),
                            u.cache_hit_pct.map(|c| format!("кэш {c}%")),
                        ]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · "),
                    ),
                )
        });

        v_flex()
            .flex_none()
            .gap_1()
            .px_3()
            .py_2()
            .bg(theme.title_bar)
            .border_t_1()
            .border_color(theme.sidebar_border)
            .text_xs()
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(Icon::new(IconName::Cpu).xsmall().text_color(muted))
                    .child(div().truncate().child(model)),
            )
            .children(context)
            .when_some(self.notice.clone(), |d, n| {
                d.child(div().text_color(theme.danger).child(n))
            })
    }
}
