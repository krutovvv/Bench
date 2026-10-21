//! Profile panels: skills (read-only browser) and MCP servers (raw JSON editor).

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Editor;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;

impl HermesApp {
    pub(super) fn render_skills(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.skill_open.as_ref().map(|(n, _)| n.clone());

        let mut rows: Vec<AnyElement> = vec![];
        let mut category = String::new();
        for (i, s) in self.skills.iter().enumerate() {
            if s.category != category {
                category = s.category.clone();
                rows.push(
                    div()
                        .pt_3()
                        .pb_1()
                        .px_2()
                        .text_xs()
                        .text_color(muted)
                        .child(category.to_uppercase())
                        .into_any_element(),
                );
            }
            let name = s.name.clone();
            let selected = open.as_deref() == Some(&s.name);
            rows.push(
                v_flex()
                    .id(("skill", i))
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .when(selected, |d| d.bg(theme.secondary))
                    .hover(|d| d.bg(theme.secondary))
                    .child(
                        div()
                            .text_sm()
                            .when(!s.enabled, |d| d.text_color(muted))
                            .child(s.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .line_clamp(2)
                            .child(s.description.clone()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_skill(name.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
        }

        let content = match &self.skill_open {
            Some((name, body)) => v_flex()
                .id("skill-body")
                .size_full()
                .overflow_y_scroll()
                .px_8()
                .py_6()
                .child(TextView::markdown(
                    SharedString::from(format!("skill-{name}")),
                    body.clone(),
                ))
                .into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child(if self.skills.is_empty() {
                    "Загружаю скиллы…".to_string()
                } else {
                    format!("{} скиллов — выбери слева", self.skills.len())
                })
                .into_any_element(),
        };

        h_flex()
            .size_full()
            .child(
                v_flex()
                    .id("skills-list")
                    .w(px(320.))
                    .h_full()
                    .flex_none()
                    .overflow_y_scroll()
                    .px_3()
                    .pb_4()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(rows),
            )
            .child(div().flex_1().min_w_0().h_full().child(content))
    }

    pub(super) fn render_mcp(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .size_full()
            .px_4()
            .py_3()
            .gap_2()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Блок mcp_servers из config.yaml профиля. Секреты показаны как есть — сохраняется целиком."),
                    )
                    .child(div().text_sm().text_color(theme.muted_foreground).child(self.mcp_status.clone()))
                    .children(self.mcp_error.clone().map(|e| div().text_sm().text_color(theme.danger).child(e)))
                    .child(Button::new("mcp-reload").small().ghost().label("Обновить").on_click(cx.listener(|this, _, window, cx| {
                        this.open_panel(crate::app::Panel::Mcp, window, cx)
                    })))
                    .child(Button::new("mcp-save").small().primary().label("Сохранить").on_click(cx.listener(|this, _, window, cx| {
                        this.save_mcp(window, cx)
                    }))),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .border_1()
                    .border_color(theme.border)
                    .child(Editor::new(&self.mcp_editor).appearance(false).h_full()),
            )
    }

    pub(super) fn render_telegram(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let all = self.tg_visible.is_none();
        let rows = self.tg_people.iter().enumerate().map(|(i, (user, name))| {
            let on = all || self.tg_visible.as_ref().is_some_and(|v| v.contains(user));
            let id = user.clone();
            h_flex()
                .id(("tg", i))
                .gap_3()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    div()
                        .size(px(14.))
                        .border_1()
                        .border_color(if on { theme.primary } else { theme.input })
                        .when(on, |d| d.bg(theme.primary)),
                )
                .child(div().flex_1().text_sm().child(name.clone()))
                .child(div().text_xs().text_color(muted).child(user.clone()))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.toggle_telegram_person(id.clone(), cx)),
                )
        });
        v_flex()
            .size_full()
            .px_4()
            .py_3()
            .gap_3()
            .child(div().text_sm().text_color(muted).child(
                "Чьи Telegram-сессии показывать в списке. Отмеченные — видны, остальные скрыты (сами сессии не трогаются).",
            ))
            .child(
                v_flex()
                    .max_w(px(560.))
                    .border_t_1()
                    .border_color(theme.border)
                    .children(rows)
                    .when(self.tg_people.is_empty(), |d| {
                        d.child(div().py_3().text_sm().text_color(muted).child("В этом профиле нет Telegram-сессий."))
                    }),
            )
    }
}
