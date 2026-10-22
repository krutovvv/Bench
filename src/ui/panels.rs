//! Settings panels: skills and config (editable text), MCP (JSON), cron (read-only), Telegram filter.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{HermesApp, Panel};

/// A toolbar over a full-height code editor.
fn editor_page(
    editor: &Entity<EditorState>,
    left: AnyElement,
    error: Option<SharedString>,
    actions: Vec<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .size_full()
        .child(
            h_flex()
                .h(px(36.))
                .flex_none()
                .pl_4()
                .pr_2()
                .gap_2()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(left),
                )
                .children(error.map(|e| {
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .truncate()
                        .max_w(px(520.))
                        .child(e)
                }))
                .children(actions),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(Editor::new(editor).appearance(false).h_full()),
        )
}

impl HermesApp {
    pub(super) fn render_skills(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.skill_open.clone();

        let mut rows: Vec<AnyElement> = vec![];
        let mut category = String::new();
        for (i, s) in self.skills.iter().enumerate() {
            if s.category != category {
                category = s.category.clone();
                rows.push(
                    div()
                        .pt_4()
                        .pb_1()
                        .px_4()
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
                    .pl(px(14.))
                    .pr_3()
                    .py_1p5()
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .when(selected, |d| d.bg(theme.list_active))
                    .hover(|d| d.bg(theme.list_hover))
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

        let content = match &open {
            Some(name) => editor_page(
                &self.skill_editor,
                format!("{name} · SKILL.md").into_any_element(),
                None,
                vec![
                    Button::new("skill-delete")
                        .small()
                        .map(|b| {
                            if self.skill_delete_armed {
                                b.danger()
                            } else {
                                b.ghost()
                            }
                        })
                        .label(if self.skill_delete_armed {
                            "Confirm delete"
                        } else {
                            "Delete"
                        })
                        .on_click(cx.listener(|this, _, window, cx| this.delete_skill(window, cx)))
                        .into_any_element(),
                    Button::new("skill-save")
                        .small()
                        .primary()
                        .label("Save")
                        .on_click(cx.listener(|this, _, window, cx| this.save_skill(window, cx)))
                        .into_any_element(),
                ],
                cx,
            )
            .into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child(if self.skills.is_empty() {
                    "Loading skills…".to_string()
                } else {
                    format!("{} skills — pick one on the left", self.skills.len())
                })
                .into_any_element(),
        };

        h_flex()
            .size_full()
            .child(
                v_flex()
                    .id("skills-list")
                    .w(px(300.))
                    .h_full()
                    .flex_none()
                    .overflow_y_scroll()
                    .pb_4()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(rows),
            )
            .child(div().flex_1().min_w_0().h_full().child(content))
    }

    pub(super) fn render_mcp(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.mcp_status.clone();
        editor_page(
            &self.mcp_editor,
            format!("mcp_servers from config.yaml {status}").into_any_element(),
            self.mcp_error.clone(),
            vec![
                Button::new("mcp-reload")
                    .small()
                    .ghost()
                    .label("Reload")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_panel(Panel::Mcp, window, cx)),
                    )
                    .into_any_element(),
                Button::new("mcp-save")
                    .small()
                    .primary()
                    .label("Save")
                    .on_click(cx.listener(|this, _, window, cx| this.save_mcp(window, cx)))
                    .into_any_element(),
            ],
            cx,
        )
    }

    pub(super) fn render_config(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        editor_page(
            &self.config_editor,
            format!("config.yaml · {}", self.current_profile()).into_any_element(),
            self.config_error.clone(),
            vec![
                Button::new("cfg-reload")
                    .small()
                    .ghost()
                    .label("Reload")
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.open_panel(Panel::Config, window, cx)
                        }),
                    )
                    .into_any_element(),
                Button::new("cfg-save")
                    .small()
                    .primary()
                    .label("Save")
                    .on_click(cx.listener(|this, _, window, cx| this.save_config(window, cx)))
                    .into_any_element(),
            ],
            cx,
        )
    }

    pub(super) fn render_cron(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let rows = self.cron.iter().map(|j| {
            v_flex()
                .px_4()
                .py_2()
                .gap_0p5()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .when(!j.enabled, |d| d.text_color(muted))
                                .child(j.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_family("Menlo")
                                .text_color(muted)
                                .child(j.schedule.clone()),
                        )
                        .when(!j.enabled, |d| {
                            d.child(div().text_xs().text_color(theme.warning).child("paused"))
                        })
                        .child(div().flex_1())
                        .when(!j.last_status.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("last: {}", j.last_status)),
                            )
                        })
                        .when(!j.next_run.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("next: {}", j.next_run)),
                            )
                        }),
                )
                .when(!j.prompt.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .line_clamp(2)
                            .child(j.prompt.clone()),
                    )
                })
        });
        v_flex()
            .id("cron")
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .when(self.cron.is_empty(), |d| {
                d.child(
                    div()
                        .p_4()
                        .text_sm()
                        .text_color(muted)
                        .child("No scheduled jobs in this profile."),
                )
            })
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
                "Whose Telegram sessions the list shows. Checked people are shown, the rest are hidden (the sessions themselves are not touched).",
            ))
            .child(
                v_flex()
                    .max_w(px(560.))
                    .border_t_1()
                    .border_color(theme.border)
                    .children(rows)
                    .when(self.tg_people.is_empty(), |d| {
                        d.child(div().py_3().text_sm().text_color(muted).child("No Telegram sessions in this profile."))
                    }),
            )
    }
}
