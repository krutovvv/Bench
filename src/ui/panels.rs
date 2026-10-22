//! Settings panels: skills and config (editable text), MCP (JSON), cron (read-only), Telegram filter.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{HermesApp, Panel};
use crate::icons::Ico;

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
        let open = self.cron_open.clone();

        let rows: Vec<AnyElement> = self
            .cron
            .iter()
            .enumerate()
            .map(|(i, j)| {
                let selected = open.as_deref() == Some(&j.id);
                let id = j.id.clone();
                let when = if !j.enabled {
                    "paused".to_string()
                } else if j.next_run.is_empty() {
                    j.schedule.clone()
                } else {
                    format!("{} · next {}", j.schedule, short_time(&j.next_run))
                };
                v_flex()
                    .id(("cron", i))
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
                            .when(!j.enabled, |d| d.text_color(muted))
                            .child(j.name.clone()),
                    )
                    .child(div().text_xs().text_color(muted).truncate().child(when))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_cron(id.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();

        let job = open
            .as_ref()
            .and_then(|id| self.cron.iter().find(|j| &j.id == id))
            .cloned();
        let content = match job {
            Some(job) => {
                let icon_button = |id: &'static str, icon: Ico, tip: &'static str| {
                    div()
                        .id(id)
                        .p_1()
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                        .child(icon.icon().small())
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
                        })
                };
                let status = if job.last_status.is_empty() {
                    String::new()
                } else {
                    format!("last run: {}", job.last_status)
                };
                editor_page(
                    &self.cron_editor,
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().text_color(theme.foreground).child(job.name.clone()))
                        .child(
                            div().w(px(240.)).child(
                                gpui_kit::component::input::Input::new(&self.cron_schedule)
                                    .small()
                                    .font_family("Menlo"),
                            ),
                        )
                        .child(div().text_xs().child(status))
                        .into_any_element(),
                    None,
                    vec![
                        icon_button("cron-run", Ico::Play, "Run now")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cron_act("trigger", window, cx)
                            }))
                            .into_any_element(),
                        (if job.enabled {
                            icon_button("cron-pause", Ico::Pause, "Pause").on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.cron_act("pause", window, cx)
                                }),
                            )
                        } else {
                            icon_button("cron-resume", Ico::RotateCcw, "Resume").on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.cron_act("resume", window, cx)
                                }),
                            )
                        })
                        .into_any_element(),
                        Button::new("cron-save")
                            .small()
                            .primary()
                            .label("Save")
                            .on_click(cx.listener(|this, _, window, cx| this.save_cron(window, cx)))
                            .into_any_element(),
                    ],
                    cx,
                )
                .into_any_element()
            }
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child(if self.cron.is_empty() {
                    "No scheduled jobs in this profile."
                } else {
                    "Pick a job on the left"
                })
                .into_any_element(),
        };

        h_flex()
            .size_full()
            .child(
                v_flex()
                    .id("cron-list")
                    .w(px(300.))
                    .h_full()
                    .flex_none()
                    .overflow_y_scroll()
                    .py_2()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(rows),
            )
            .child(div().flex_1().min_w_0().h_full().child(content))
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

    pub(super) fn render_appearance(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::theme::{ACCENTS, Mode, Size};
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let a = self.appearance;

        let row = |title: &'static str, hint: &'static str, control: AnyElement| {
            h_flex()
                .gap_4()
                .py_3()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .flex_1()
                        .gap_0p5()
                        .child(div().text_sm().child(title))
                        .child(div().text_xs().text_color(muted).child(hint)),
                )
                .child(control)
        };
        // A segmented control: square cells sharing borders, the chosen one in the accent.
        let segmented = |id: &'static str,
                         items: Vec<(&'static str, bool, crate::theme::Appearance)>,
                         cx: &mut Context<Self>| {
            h_flex()
                .border_1()
                .border_color(theme.border)
                .children(items.into_iter().enumerate().map(|(k, (label, on, next))| {
                    div()
                        .id((id, k))
                        .px_3()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .when(k > 0, |d| d.border_l_1().border_color(theme.border))
                        .when(on, |d| {
                            d.bg(theme.primary).text_color(theme.primary_foreground)
                        })
                        .when(!on, |d| d.hover(|s| s.bg(theme.list_hover)))
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_appearance(next, window, cx)
                        }))
                }))
                .into_any_element()
        };

        let modes = [
            ("Light", Mode::Light),
            ("Dark", Mode::Dark),
            ("System", Mode::System),
        ]
        .into_iter()
        .map(|(l, m)| (l, a.mode == m, crate::theme::Appearance { mode: m, ..a }))
        .collect();
        let sizes = [
            ("Small", Size::Small),
            ("Normal", Size::Normal),
            ("Large", Size::Large),
        ]
        .into_iter()
        .map(|(l, s)| (l, a.size == s, crate::theme::Appearance { size: s, ..a }))
        .collect();
        let dark = theme.mode.is_dark();
        let accents = h_flex()
            .gap_1()
            .children(
                ACCENTS
                    .iter()
                    .enumerate()
                    .map(|(k, (name, light, dark_c))| {
                        let on = a.accent == k;
                        let color: Hsla = rgb(if dark { *dark_c } else { *light }).into();
                        let next = crate::theme::Appearance { accent: k, ..a };
                        div()
                            .id(("accent", k))
                            .p(px(3.))
                            .border_1()
                            .border_color(if on {
                                theme.foreground
                            } else {
                                theme.transparent
                            })
                            .cursor_pointer()
                            .child(div().size(px(18.)).bg(color))
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(*name).build(window, cx)
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_appearance(next, window, cx)
                            }))
                    }),
            )
            .into_any_element();

        v_flex()
            .id("appearance")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .py_2()
            .child(
                v_flex()
                    .max_w(px(640.))
                    .child(row(
                        "Theme",
                        "Light, dark, or follow macOS.",
                        segmented("mode", modes, cx),
                    ))
                    .child(row("Accent", "Buttons, selection, links.", accents))
                    .child(row(
                        "Size",
                        "Text and spacing across the app.",
                        segmented("size", sizes, cx),
                    )),
            )
    }

    pub(super) fn render_gateways(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let rows: Vec<AnyElement> = self
            .targets
            .clone()
            .into_iter()
            .enumerate()
            .map(|(k, target)| {
                let i = self
                    .installs
                    .get(&target.label())
                    .cloned()
                    .unwrap_or_default();
                let status: SharedString = match (&i.busy, &i.error, &i.available) {
                    (Some(b), _, _) => b.clone().into(),
                    (_, Some(e), _) => e.clone().into(),
                    (_, _, Some(v)) if v.starts_with('v') => format!("{v} is available").into(),
                    (_, _, Some(v)) => format!("Update available: {v}").into(),
                    _ if i.checked => "Up to date".into(),
                    _ => "".into(),
                };
                let status_color = if i.error.is_some() {
                    theme.danger
                } else if i.available.is_some() {
                    theme.primary
                } else {
                    muted
                };
                let t2 = target.clone();
                h_flex()
                    .gap_4()
                    .py_3()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_baseline()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(target.label()),
                                    )
                                    .child(div().text_sm().font_family("Menlo").child(
                                        if i.version.is_empty() {
                                            "—".to_string()
                                        } else {
                                            i.version.clone()
                                        },
                                    ))
                                    .when(!i.channel.is_empty(), |d| {
                                        d.child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(i.channel.clone()),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(status_color)
                                    .truncate()
                                    .child(status),
                            ),
                    )
                    .when(i.available.is_some() && i.busy.is_none(), |d| {
                        d.child(
                            Button::new(("gw-update", k))
                                .small()
                                .primary()
                                .label("Update")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.update_install(t2.clone(), window, cx)
                                })),
                        )
                    })
            })
            .map(|r| r.into_any_element())
            .collect();
        v_flex()
            .id("gateways")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .py_2()
            .child(
                v_flex()
                    .max_w(px(720.))
                    .child(
                        h_flex()
                            .py_2()
                            .gap_3()
                            .items_center()
                            .child(div().flex_1().text_xs().text_color(muted).child(
                                "Hermes on each machine. Updating takes a full backup first, then restarts that gateway; going back to an older release means restoring that backup.",
                            ))
                            .child(
                                Button::new("gw-check")
                                    .small()
                                    .ghost()
                                    .label("Check again")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.check_installs(window, cx)
                                    })),
                            ),
                    )
                    .children(rows),
            )
    }
}

/// "2026-10-23T10:30:00+05:00" → "Oct 23 10:30".
fn short_time(iso: &str) -> String {
    let (date, time) = iso.split_once('T').unwrap_or((iso, ""));
    let mut d = date.split('-');
    let (_, m, day) = (d.next(), d.next(), d.next());
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = m
        .and_then(|m| m.parse::<usize>().ok())
        .and_then(|m| MONTHS.get(m.wrapping_sub(1)));
    match (month, day) {
        (Some(mo), Some(day)) => format!(
            "{mo} {} {}",
            day.trim_start_matches('0'),
            time.get(..5).unwrap_or("")
        ),
        _ => iso.to_string(),
    }
}
