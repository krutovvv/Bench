//! Bottom bar across the window: where / which profile on the left; context, reasoning effort,
//! model, gateway restart and settings on the right.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::compact;
use crate::app::{HermesApp, Panel};
use crate::icons::Ico;
use crate::model::{EFFORTS, Step};

impl HermesApp {
    /// Model: live usage, else the open session, else the profile, else the default profile.
    fn model_name(&self) -> String {
        let non_empty = |s: &String| !s.is_empty();
        self.usage
            .as_ref()
            .map(|u| u.model.clone())
            .filter(non_empty)
            .or_else(|| {
                let id = self.session.as_ref()?;
                self.sessions
                    .iter()
                    .find(|s| &s.id == id)
                    .map(|s| s.model.clone())
            })
            .filter(non_empty)
            .or_else(|| {
                let p = self.profile.as_ref()?;
                self.profiles
                    .iter()
                    .find(|x| &x.name == p)
                    .map(|x| x.model.clone())
            })
            .filter(non_empty)
            .or_else(|| {
                self.profiles
                    .iter()
                    .find(|x| x.name == "default")
                    .map(|x| x.model.clone())
            })
            .filter(non_empty)
            .unwrap_or_else(|| "—".into())
    }

    pub(super) fn render_statusbar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let view = cx.entity();

        let item = |id: &'static str| {
            h_flex()
                .id(id)
                .h_full()
                .px_2()
                .gap_1()
                .items_center()
                .text_color(muted)
        };
        let button = |id: &'static str, icon: Ico, tip: &'static str, active: bool| {
            item(id)
                .cursor_pointer()
                .when(active, |d| d.text_color(theme.primary))
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().xsmall())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        // A label as wide as its text that opens a menu of choices.
        let picker = |id: &'static str,
                      icon: Ico,
                      label: String,
                      items: Vec<(
            String,
            bool,
            Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)>,
        )>| {
            let items = std::rc::Rc::new(items);
            let view = view.clone();
            // The icon sits inside the button, so hover lights up both.
            h_flex().h_full().items_center().pl_1().child(
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .icon(icon.icon())
                    .label(label)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        for (k, (text, checked, _)) in items.iter().enumerate() {
                            let (items, view) = (items.clone(), view.clone());
                            menu = menu.item(
                                PopupMenuItem::new(text.clone()).checked(*checked).on_click(
                                    move |_, window, cx| {
                                        view.update(cx, |this, cx| (items[k].2)(this, window, cx))
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            )
        };

        let targets = self
            .targets
            .iter()
            .map(|t| {
                let t2 = t.clone();
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, window, cx| {
                        if this.target != t2 || this.conn.is_none() {
                            this.connect(t2.clone(), window, cx)
                        }
                    });
                (t.label(), *t == self.target, f)
            })
            .collect();
        let profiles = self
            .profiles
            .iter()
            .map(|p| {
                let name = p.name.clone();
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, window, cx| this.select_profile(name.clone(), window, cx));
                (p.label.clone(), self.profile.as_deref() == Some(&p.name), f)
            })
            .collect();
        let efforts = EFFORTS
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, _, cx| this.set_effort(i, cx));
                (e.to_string(), i == self.effort, f)
            })
            .collect();
        let approvals: Vec<_> = crate::model::APPROVAL_MODES
            .iter()
            .map(|m| {
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, _, cx| this.set_approval_mode(m, cx));
                let label = match *m {
                    "off" => "off — never ask",
                    "smart" => "smart — ask for risky commands",
                    _ => "manual — ask every time",
                };
                (
                    label.to_string(),
                    self.approval_mode.as_deref() == Some(*m),
                    f,
                )
            })
            .collect();
        let current_model = self.model_name();
        let models: Vec<_> = self
            .profile_models
            .iter()
            .enumerate()
            .map(|(i, (m, p))| {
                let f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)> =
                    Box::new(move |this, _, cx| this.set_model(i, cx));
                let checked = self.model_pick.map_or(*m == current_model, |k| k == i);
                let tag = if i == 0 { "primary" } else { "fallback" };
                let label = if p.is_empty() {
                    format!("{m}  ·  {tag}")
                } else {
                    format!("{m}  ·  {p}, {tag}")
                };
                (label, checked, f)
            })
            .collect();
        let profile_label = self
            .profiles
            .iter()
            .find(|p| Some(&p.name) == self.profile.as_ref())
            .map(|p| p.label.clone())
            .unwrap_or_else(|| "Profile".into());

        // Context: Hermes' numbers once a turn has run; before that (a reopened chat) a rough
        // estimate from the transcript, marked "~". The limit falls back to model.context_length.
        let usage = self.usage.clone();
        let real = usage.as_ref().filter(|u| u.context_used > 0);
        let max = real
            .map(|u| u.context_max)
            .filter(|m| *m > 0)
            .unwrap_or(self.profile_context);
        let (used, estimated) = match real {
            Some(u) => (u.context_used, false),
            None => (self.estimate_tokens(), true),
        };
        let percent = if max > 0 {
            used as f64 * 100. / max as f64
        } else {
            0.
        };
        let stats = self.session_stats(used, max, estimated);
        let context = (max > 0 && (self.session.is_some() || !self.turns.is_empty())).then(|| {
            let color = if percent > 85. {
                theme.danger
            } else if percent > 60. {
                theme.warning
            } else {
                muted
            };
            let label = format!(
                "{}{} / {} · {:.0}%",
                if estimated { "~" } else { "" },
                compact(used),
                compact(max),
                percent
            );
            gpui_kit::component::popover::Popover::new("context-pop")
                .anchor(gpui_kit::Anchor::BottomRight)
                .trigger(
                    Button::new("context")
                        .ghost()
                        .xsmall()
                        .icon(Ico::MemoryStick.icon())
                        .label(label)
                        .text_color(color),
                )
                .content(move |_, _, cx| {
                    let theme = cx.theme();
                    v_flex()
                        .w(px(260.))
                        .py_1()
                        .children(stats.iter().map(|(k, v)| {
                            h_flex()
                                .px_3()
                                .py_0p5()
                                .gap_4()
                                .text_xs()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(theme.muted_foreground)
                                        .child(k.clone()),
                                )
                                .child(div().child(v.clone()))
                        }))
                })
        });

        // Speed: our own split (prefill / generation) when measured, else Hermes' blended average.
        let rate = |v: f64| {
            if v >= 1000. {
                format!("{:.1}k", v / 1000.)
            } else {
                format!("{v:.0}")
            }
        };
        let speed = match (self.speed.prefill, self.speed.generation) {
            (_, Some(g)) => Some((
                match self.speed.prefill {
                    Some(p) => format!("{} · {} t/s", rate(p), rate(g)),
                    None => format!("{} t/s", rate(g)),
                },
                SharedString::from(
                    [
                        self.speed
                            .prefill
                            .map(|p| format!("prefill {} tok/s", rate(p))),
                        Some(format!("generation {} tok/s", rate(g))),
                        Some("last model call, measured here".to_string()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n"),
                ),
            )),
            (Some(p), None) => Some((
                format!("{} t/s prefill", rate(p)),
                SharedString::from(format!("prefill {} tok/s", rate(p))),
            )),
            (None, None) => self.usage.as_ref().and_then(|u| u.avg_tps).map(|t| {
                (
                    format!("{} t/s", rate(t)),
                    SharedString::from(
                        "average over recent calls, prompt processing included (Hermes)",
                    ),
                )
            }),
        }
        .map(|(label, tip)| {
            item("speed")
                .child(Ico::Gauge.icon().xsmall())
                .child(label)
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        });

        h_flex()
            .h(px(28.))
            .flex_none()
            .w_full()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            .text_xs()
            .child(picker("target", Ico::Server, self.target.label(), targets))
            .when(!self.profiles.is_empty(), |d| {
                d.child(picker("profile", Ico::UserRound, profile_label, profiles))
            })
            .child(div().flex_1())
            .children(speed)
            .children(context.map(|c| h_flex().h_full().items_center().pl_1().child(c)))
            .when_some(self.approval_mode.clone(), |d, mode| {
                d.child(picker("approvals", Ico::Shield, mode, approvals))
            })
            .child(picker(
                "effort",
                Ico::Brain,
                EFFORTS[self.effort].to_string(),
                efforts,
            ))
            .map(|d| {
                if models.is_empty() || self.session.is_some() {
                    d.child(
                        item("model")
                            .child(Ico::Cpu.icon().xsmall())
                            .child(current_model),
                    )
                } else {
                    d.child(picker("model", Ico::Cpu, current_model, models))
                }
            })
            .child(
                button(
                    "restart",
                    Ico::RefreshCw,
                    "Restart the Hermes gateway",
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| this.restart_gateway(window, cx))),
            )
            .child(
                button(
                    "settings",
                    Ico::Settings,
                    "Settings",
                    self.panel.is_settings(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.panel.is_settings() {
                        this.panel = Panel::Chat;
                        cx.notify();
                    } else {
                        this.open_panel(Panel::Skills, window, cx);
                    }
                })),
            )
    }
}

impl HermesApp {
    /// Rough tokens of the open chat: its text at ~3.5 characters a token.
    fn estimate_tokens(&self) -> u64 {
        let chars: usize = self
            .turns
            .iter()
            .map(|t| {
                t.user.len()
                    + t.answer.len()
                    + t.steps
                        .iter()
                        .map(|s| match s {
                            Step::Tool { args, output, .. } => args.len() + output.len(),
                            Step::Note(x) | Step::Thought(x) => x.len(),
                            Step::Subagent { summary, .. } => summary.len(),
                        })
                        .sum::<usize>()
            })
            .sum();
        (chars as f64 / 3.5) as u64
    }

    /// The rows of the context panel: what this chat has used and done.
    fn session_stats(&self, used: u64, max: u64, estimated: bool) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> = vec![];
        let tilde = if estimated { "~" } else { "" };
        rows.push((
            "Context".into(),
            if max > 0 {
                format!("{tilde}{} of {}", compact(used), compact(max))
            } else {
                format!("{tilde}{}", compact(used))
            },
        ));
        if max > 0 {
            rows.push(("Left".into(), compact(max.saturating_sub(used))));
        }
        rows.push(("Turns".into(), self.turns.len().to_string()));
        let tools = self
            .turns
            .iter()
            .flat_map(|t| &t.steps)
            .filter(|s| matches!(s, Step::Tool { .. }))
            .count();
        rows.push(("Tool calls".into(), tools.to_string()));
        let subagents = self
            .turns
            .iter()
            .flat_map(|t| &t.steps)
            .filter(|s| matches!(s, Step::Subagent { .. }))
            .count();
        if subagents > 0 {
            rows.push(("Subagents".into(), subagents.to_string()));
        }
        let files: std::collections::HashSet<&str> = self
            .turns
            .iter()
            .flat_map(|t| &t.files)
            .map(|f| f.path.as_str())
            .collect();
        if !files.is_empty() {
            rows.push(("Files changed".into(), files.len().to_string()));
        }
        if let Some(u) = self.usage.as_ref() {
            if u.calls > 0 {
                rows.push(("Model calls".into(), u.calls.to_string()));
            }
            if u.input + u.output > 0 {
                rows.push((
                    "Tokens in / out".into(),
                    format!("{} / {}", compact(u.input), compact(u.output)),
                ));
            }
            if let Some(c) = u.cache_hit_pct {
                rows.push(("Cache hit".into(), format!("{c}%")));
            }
            rows.push(("Compactions".into(), u.compressions.to_string()));
        }
        if let Some(p) = self.speed.prefill {
            rows.push(("Prefill".into(), format!("{p:.0} tok/s")));
        }
        if let Some(g) = self.speed.generation {
            rows.push(("Generation".into(), format!("{g:.0} tok/s")));
        }
        if estimated {
            rows.push(("".into(), "~ estimated until the next reply".into()));
        }
        rows
    }
}
