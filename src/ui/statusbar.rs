//! Bottom bar across the window: where / which profile on the left; context, reasoning effort,
//! model, gateway restart and settings on the right.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::compact;
use crate::app::{BenchApp, Panel};
use crate::icons::Ico;
use ::thread::{Row, Step};

impl BenchApp {
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
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        // A label as wide as its text that opens a menu of choices.
        // `note`: a line above the choices saying what they apply to.
        let picker = |id: &'static str,
                      icon: Ico,
                      label: String,
                      note: Option<&'static str>,
                      // (index of the first item, the group's name): a label above each group.
                      sections: Vec<(usize, String)>,
                      items: Vec<(
            String,
            bool,
            Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>,
        )>| {
            let items = std::rc::Rc::new(items);
            let view = view.clone();
            // The icon sits inside the button, so hover lights up both.
            h_flex().h_full().items_center().pl_1().child(
                Button::new(id)
                    .ghost()
                    .small()
                    .icon(icon.icon())
                    .label(label)
                    // One tone across the bar, like the model and the icon buttons.
                    .text_color(muted)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        if let Some(note) = note {
                            menu = menu.label(note).separator();
                        }
                        for (k, (text, checked, _)) in items.iter().enumerate() {
                            if let Some((_, name)) = sections.iter().find(|(at, _)| *at == k) {
                                if k > 0 {
                                    menu = menu.separator();
                                }
                                menu = menu.label(name.clone());
                            }
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

        // Context: the agent's numbers once a turn has run; before that (a reopened chat) a rough
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
        let context = (max > 0 && (self.session.is_some() || !self.thread.is_empty())).then(|| {
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
                        .small()
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
                                .text_sm()
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

        // Speed: how fast the model writes, in the bar's own muted colour.
        let rate = |v: f64| {
            if v >= 1000. {
                format!("{:.1}k", v / 1000.)
            } else {
                format!("{v:.0}")
            }
        };
        // Speed: how fast the model writes — the live reading (the last second) while text
        // streams, else the last call's measured generation, else the agent's blended average.
        // Prefill is not shown here: it is in the panel this opens. Click: the chart of this answer.
        let label = match (self.speed.live, self.speed.generation) {
            (Some(l), _) => Some(format!("{} t/s", rate(l))),
            (None, Some(g)) => Some(format!("{} t/s", rate(g))),
            _ => self
                .usage
                .as_ref()
                .and_then(|u| u.avg_tps)
                .map(|t| format!("{} t/s", rate(t))),
        };
        let samples = self.speed.samples.clone();
        let (prefill, generation) = (self.speed.prefill, self.speed.generation);
        let speed = label.map(|label| {
            gpui_kit::component::popover::Popover::new("speed-pop")
                .anchor(gpui_kit::Anchor::BottomRight)
                .trigger(
                    Button::new("speed")
                        .ghost()
                        .small()
                        .icon(Ico::Gauge.icon())
                        .label(label)
                        .text_color(muted),
                )
                .content(move |_, _, cx| speed_panel(&samples, prefill, generation, cx))
        });

        // Terminals sent to the background: each brings it back into its thread; Stop all ends them.
        let mut background_terms: Vec<(
            String,
            bool,
            Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>,
        )> = self
            .ssh
            .tabs
            .iter()
            .filter(|t| t.background)
            .enumerate()
            .map(|(k, t)| {
                let id = t.id;
                let f: Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)> =
                    Box::new(move |this, _, cx| {
                        if let Some(t) = this.ssh.tabs.iter_mut().find(|t| t.id == id) {
                            t.background = false;
                            t.collapsed = false;
                        }
                        cx.notify();
                    });
                (format!("Terminal {}", k + 1), false, f)
            })
            .collect();
        if !background_terms.is_empty() {
            background_terms.push((
                "Stop all".into(),
                false,
                Box::new(|this, _, cx| {
                    for t in this.ssh.tabs.iter().filter(|t| t.background) {
                        t.session.kill();
                    }
                    this.ssh.tabs.retain(|t| !t.background);
                    cx.notify();
                }),
            ));
        }
        h_flex()
            .h(px(super::STATUS_H - 1.))
            .flex_none()
            .w_full()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            .text_sm()
            // Nothing about where the agent runs or which one: a thread runs with its project.
            // Far left: the gateway of the machine this project lives on — always there, healthy
            // or not; its state in detail, its log, Reconnect and Restart are inside. Then the
            // model, how full its context is, how hard it thinks, what it may run.
            .children(self.render_backend(cx))
            .children(context.map(|c| h_flex().h_full().items_center().pl_1().child(c)))
            .children(speed.map(|s| h_flex().h_full().items_center().pl_1().child(s)))
            // The fallback answering, in red: the main model failed, and why, until it answers again.
            .children(self.fallback_on.clone().map(|(model, why)| {
                let tip = if why.is_empty() {
                    "The main model is unavailable: the fallback answers until it is back.".to_string()
                } else {
                    format!("The main model is unavailable ({why}): the fallback answers until it is back.")
                };
                h_flex().h_full().items_center().pl_1().child(
                    item("fallback")
                        .text_color(theme.danger)
                        .child(Ico::CircleAlert.icon().small())
                        .child(format!("Main model down · on fallback {model}"))
                        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx)),
                )
            }))
            .child(div().flex_1())
            // Right: icons only — the terminals in the background (if any), attach, dictate,
            // settings, and Stop at the very edge.
            .when(!background_terms.is_empty(), |d| {
                d.child(picker(
                    "bg-terminals",
                    Ico::Terminal,
                    String::new(),
                    Some("Running terminals"),
                    vec![],
                    background_terms,
                ))
            })
            .child(
                button("attach", Ico::Paperclip, "Attach files", false)
                    .on_click(cx.listener(|this, _, window, cx| this.pick_attachments(window, cx))),
            )
            // Dictation: the mic stays the mic while it records — a small bar on its edge shows
            // the level (⌘⇧D or the same button stops). The typed text is never hidden: a
            // thought is often dictated in pieces, with pauses and typing between them. A
            // spinner while transcribing.
            .child({
                let recording = self.recorder.is_some();
                if self.transcribing {
                    item("mic")
                        .text_color(muted)
                        .child(
                            gpui_kit::component::spinner::Spinner::new()
                                .icon(Ico::Loader.icon())
                                .small(),
                        )
                        .tooltip(|window, cx| Tooltip::new("Transcribing…").build(window, cx))
                } else if recording {
                    let level = self
                        .recorder
                        .as_ref()
                        .map(|r| r.level())
                        .unwrap_or(0.)
                        .min(1.);
                    item("mic")
                        .text_color(theme.danger)
                        .child(
                            div().relative().child(Ico::Mic.icon().small()).child(
                                div()
                                    .absolute()
                                    .right(px(-2.))
                                    .bottom(px(1.))
                                    .w(px(2.))
                                    .h(px(2. + level * 9.))
                                    .rounded(px(1.))
                                    .bg(theme.danger),
                            ),
                        )
                        .tooltip(|window, cx| {
                            Tooltip::new("Stop and transcribe  ⌘⇧D").build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)))
                } else {
                    button("mic", Ico::Mic, "Dictate  ⌘⇧D", false)
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)))
                }
            })
            .child(
                button(
                    "settings",
                    Ico::Settings,
                    "Settings",
                    self.panel == Panel::Providers,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.panel == Panel::Providers {
                        this.panel = Panel::Chat;
                        cx.notify();
                    } else {
                        this.open_panel(Panel::Providers, window, cx);
                    }
                })),
            )
            // Stop last, at the window's edge: a filled chip is the heaviest thing on the bar, so
            // it belongs at the end, not among the flat icons (⌘. does the same).
            .when(self.live.is_some(), |d| {
                d.child(
                    h_flex().h_full().items_center().pl_1().pr_2().child(
                        div()
                            .id("stop")
                            .h(px(20.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_1()
                            .cursor_pointer()
                            .rounded_md()
                            .bg(theme.danger)
                            .text_color(theme.danger_foreground)
                            .text_sm()
                            .hover(|s| s.opacity(0.85))
                            .child(Ico::Square.icon().xsmall())
                            .child("Stop")
                            .tooltip(|window, cx| Tooltip::new("Stop  ⌘.").build(window, cx))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.click_claimed = true;
                                this.stop(cx);
                            })),
                    ),
                )
            })
    }
}

impl BenchApp {
    /// Rough tokens of the open chat: its text at ~3.5 characters a token.
    fn estimate_tokens(&self) -> u64 {
        let chars: usize = self
            .tv
            .rows
            .iter()
            .map(|r| match r {
                Row::User { text, .. } | Row::Answer { text, .. } => text.len(),
                Row::Work { steps, .. } => steps
                    .iter()
                    .map(|s| match s {
                        Step::Tool { args, output, .. } => {
                            args.to_string().len() + output.as_ref().map_or(0, String::len)
                        }
                        Step::Note(x) | Step::Thought(x) => x.len(),
                    })
                    .sum(),
                _ => 0,
            })
            .sum();
        (chars as f64 / 3.5) as u64
    }

    /// The rows of the context panel: what this chat has used and done.
    fn session_stats(&self, used: u64, max: u64, estimated: bool) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> = vec![];
        let tilde = if estimated { "~" } else { "" };
        // Which model this thread answers with; the project sets it in its environment.
        if let Some(m) = self
            .usage
            .as_ref()
            .map(|u| u.model.clone())
            .filter(|m| !m.is_empty())
        {
            rows.push(("Model".into(), m));
        }
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
        let messages = self
            .tv
            .rows
            .iter()
            .filter(|r| matches!(r, Row::User { .. }))
            .count();
        rows.push(("Messages".into(), messages.to_string()));
        let tools = self
            .tv
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Work { steps, .. } => Some(steps.iter().filter(|s| s.is_tool()).count()),
                _ => None,
            })
            .sum::<usize>();
        rows.push(("Tool calls".into(), tools.to_string()));
        let files = self.session_files();
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

/// The speed panel: per-second generation speed of the current/last answer as bars, its range,
/// and the last call's measured prefill and generation.
fn speed_panel(samples: &[f64], prefill: Option<f64>, generation: Option<f64>, cx: &App) -> Div {
    let theme = cx.theme();
    let row = |k: &str, v: String| {
        h_flex()
            .px_3()
            .py_0p5()
            .gap_4()
            .text_sm()
            .child(
                div()
                    .flex_1()
                    .text_color(theme.muted_foreground)
                    .child(k.to_string()),
            )
            .child(div().child(v))
    };
    let mut panel = v_flex().w(px(300.)).py_2();
    if samples.is_empty() {
        panel = panel.child(
            div()
                .px_3()
                .pb_1()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("The chart fills in while the next answer is written."),
        );
    } else {
        let max = samples.iter().cloned().fold(0., f64::max).max(1.);
        let min = samples.iter().cloned().fold(f64::MAX, f64::min);
        let avg = samples.iter().sum::<f64>() / samples.len() as f64;
        // The last 68 seconds at most (as many 3px bars as fit); each bar is one second.
        let shown = &samples[samples.len().saturating_sub(68)..];
        // One 3px bar per second from the left: a short answer is a few bars, not one slab.
        let bars = shown.iter().map(|v| {
            div()
                .w(px(3.))
                .flex_none()
                .h(px((v / max * 56.).max(1.) as f32))
                .bg(theme.primary.opacity(0.75))
        });
        panel = panel
            .child(
                h_flex()
                    .mx_3()
                    .mb_2()
                    .h(px(56.))
                    .gap(px(1.))
                    .items_end()
                    .border_b_1()
                    .border_color(theme.border)
                    .children(bars),
            )
            .child(row(
                "Now / average",
                format!(
                    "{:.0} / {:.0} tok/s",
                    shown.last().copied().unwrap_or(0.),
                    avg
                ),
            ))
            .child(row("Min – max", format!("{min:.0} – {max:.0} tok/s")))
            // One sample is a second the model was writing; waits for tools and prefill are left
            // out of every figure here, so this row counts writing, not the turn's wall time.
            .child(row("Writing", format!("{} s", samples.len())));
    }
    if let Some(p) = prefill {
        panel = panel.child(row("Prefill (last call)", format!("{p:.0} tok/s")));
    }
    if let Some(g) = generation {
        panel = panel.child(row("Generation (last call)", format!("{g:.0} tok/s")));
    }
    panel
}

impl BenchApp {
    /// The gateway's state — always, ready or not — and, on click, its state in detail, its
    /// messaging platforms, the last lines of its log, Reconnect and Restart.
    fn render_backend(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let status = match &self.gw_status {
            Some(Ok(s)) if self.online() => Some(s.clone()),
            _ => None,
        };
        // Always shown, healthy or not: the person decides whether to look, not a late error.
        let working = status.as_ref().map(|s| s.active_agents).unwrap_or(0);
        let (label, color): (String, Hsla) = if self.connecting {
            ("Connecting…".into(), muted)
        } else if !self.online() {
            ("Gateway offline".into(), theme.danger)
        } else {
            match (&self.gw_status, &status) {
                (Some(Err(_)), _) => ("Gateway unreachable".into(), theme.danger),
                (_, Some(s)) if !s.running => ("Gateway stopped".into(), theme.danger),
                (_, Some(s)) if s.state != "running" && !s.state.is_empty() => {
                    (format!("Gateway {}", s.state), theme.warning)
                }
                (_, Some(s)) if s.overall != "ok" => ("Gateway degraded".into(), theme.warning),
                _ if working > 0 => (format!("Gateway · {working} working"), theme.primary),
                _ => ("Gateway".into(), theme.primary),
            }
        };
        let log = self.gw_log.clone();
        let connected = self.online();
        let error = match &self.gw_status {
            Some(Err(e)) => Some(e.clone()),
            _ => None,
        };
        let where_ = format!("{} · {}", self.target.label(), self.current_profile());
        let view = cx.entity();
        Some(
            h_flex()
                .h_full()
                .items_center()
                .pl_1()
                .child(
                    gpui_kit::component::popover::Popover::new("backend-pop")
                        .anchor(gpui_kit::Anchor::BottomLeft)
                        .trigger(
                            // Opening the panel asks macOS what it allows now (the choice may
                            // have changed in System Settings since the start).
                            {
                                let v = view.clone();
                                Button::new("backend")
                                    .ghost()
                                    .small()
                                    .icon(Ico::Activity.icon())
                                    .label(label)
                                    .text_color(color)
                                    .on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| this.notif(false, cx))
                                    })
                            }
                        )
                        .content(move |_, _, cx| {
                            let theme = cx.theme().clone();
                            let muted = theme.muted_foreground;
                            let dot = |ok: bool, warn: bool| {
                                div().size(px(6.)).bg(if ok {
                                    theme.primary
                                } else if warn {
                                    theme.warning
                                } else {
                                    theme.danger
                                })
                            };
                            let line = |ok: bool, warn: bool, text: String| {
                                h_flex()
                                    .gap_2()
                                    .items_start()
                                    .text_sm()
                                    .child(div().pt(px(5.)).child(dot(ok, warn)))
                                    // A URL or a refusal is longer than the panel: it wraps
                                    // inside it, never over its edge.
                                    .child(div().flex_1().min_w_0().whitespace_normal().child(text))
                            };
                            let icon = |id: &'static str, ico: Ico, tip: &'static str| {
                                div()
                                    .id(id)
                                    .p_1()
                                    .cursor_pointer()
                                    .text_color(muted)
                                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                                    .child(ico.icon().small())
                                    .tooltip(move |window, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(tip)
                                            .build(window, cx)
                                    })
                            };
                            let (v1, v2) = (view.clone(), view.clone());
                            let mut lines = vec![line(
                                connected,
                                false,
                                if connected {
                                    "Connected".into()
                                } else {
                                    "Not connected".into()
                                },
                            )];
                            if let Some(s) = &status {
                                let ok = s.running && s.state == "running";
                                let mut text = if s.running {
                                    format!(
                                        "Gateway {}",
                                        if s.state.is_empty() {
                                            "running"
                                        } else {
                                            &s.state
                                        }
                                    )
                                } else {
                                    "Gateway stopped".to_string()
                                };
                                if !s.version.is_empty() {
                                    text.push_str(&format!(" · v{}", s.version));
                                }
                                if s.active_agents > 0 {
                                    text.push_str(&format!(" · {} working", s.active_agents));
                                }
                                lines.push(line(ok, s.running, text));
                                if let Some(r) = &s.exit_reason {
                                    lines.push(line(false, false, r.clone()));
                                }
                            } else if let Some(e) = &error {
                                lines.push(line(false, false, e.clone()));
                            }
                            let platforms: Vec<AnyElement> = status
                                .iter()
                                .flat_map(|s| s.platforms.iter())
                                .map(|(name, state, err)| {
                                    // "inbox:telegram" → "inbox · Telegram"
                                    let name = match name.split_once(':') {
                                        Some((who, kind)) => {
                                            let mut k = kind.to_string();
                                            if let Some(f) = k.get_mut(0..1) {
                                                f.make_ascii_uppercase();
                                            }
                                            format!("{who} · {k}")
                                        }
                                        None => name.replace('_', " "),
                                    };
                                    let ok = state == "connected";
                                    v_flex()
                                        .py_0p5()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .text_sm()
                                                .child(
                                                    div().flex_1().min_w_0().truncate().child(name),
                                                )
                                                .child(dot(
                                                    ok,
                                                    state == "connecting" || state == "retrying",
                                                ))
                                                .child(
                                                    div().text_color(muted).child(state.clone()),
                                                ),
                                        )
                                        .children(err.clone().map(|e| {
                                            div()
                                                .text_sm()
                                                .text_color(theme.danger)
                                                .whitespace_normal()
                                                .child(e.clone())
                                        }))
                                        .into_any_element()
                                })
                                .collect();
                            v_flex()
                                .w(px(380.))
                                // Whatever a line holds, the panel keeps it in: nothing paints
                                // over its edge.
                                .overflow_hidden()
                                .child(
                                    h_flex()
                                        .px_3()
                                        .py_2()
                                        .gap_2()
                                        .items_start()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(v_flex().flex_1().min_w_0().gap_1().children(lines))
                                        .child(
                                            icon("backend-reconnect", Ico::RefreshCw, "Reconnect")
                                                .on_click(move |_, window, cx| {
                                                    v1.update(cx, |this, cx| {
                                                        let t = this.target.clone();
                                                        this.connect(t, window, cx)
                                                    })
                                                }),
                                        )
                                        .child(div().w(px(1.)).h(px(18.)).bg(theme.border))
                                        .child(
                                            icon(
                                                "backend-restart",
                                                Ico::Power,
                                                "Restart the gateway",
                                            )
                                            .on_click(
                                                move |_, window, cx| {
                                                    v2.update(cx, |this, cx| {
                                                        this.restart_gateway(window, cx)
                                                    })
                                                },
                                            ),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .px_3()
                                        .py_2()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("MESSAGING PLATFORMS"),
                                        )
                                        .when(platforms.is_empty(), |d| {
                                            d.child(
                                                div()
                                                    .text_sm()
                                                    .text_color(muted)
                                                    .child("None connected"),
                                            )
                                        })
                                        .children(platforms),
                                )
                                // Whether macOS lets Bench speak at all: the one thing that
                                // stands between "the agent finished" and silence.
                                .child({
                                    let v = view.clone();
                                    let w = view.clone();
                                    let auth = crate::notify::AUTH
                                        .load(std::sync::atomic::Ordering::Relaxed);
                                    v_flex()
                                        .px_3()
                                        .py_2()
                                        .gap_1()
                                        .border_t_1()
                                        .border_color(theme.border)
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(muted)
                                                        .child("NOTIFICATIONS"),
                                                )
                                                .child(div().flex_1())
                                                .child(
                                                    icon(
                                                        "notif-test",
                                                        Ico::Bell,
                                                        "Send a test notification",
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        v.update(cx, |this, cx| {
                                                            this.notif(true, cx)
                                                        })
                                                    }),
                                                ),
                                        )
                                        .when(auth != 2, |d| {
                                            d.child(line(
                                                auth == 1,
                                                auth != 1,
                                                if auth == 1 {
                                                    "Allowed — a banner when Bench is in the background".into()
                                                } else {
                                                    "Not asked yet — macOS asks once, on the next finish".into()
                                                },
                                            ))
                                        })
                                        .when(auth == 2, |d| {
                                            d.child(
                                                div()
                                                    .id("notif-settings")
                                                    .cursor_pointer()
                                                    .text_sm()
                                                    .text_color(theme.danger)
                                                    .whitespace_normal()
                                                    .child(
                                                        "Refused — macOS keeps Bench quiet. Open System Settings → Notifications and allow it.",
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        w.update(cx, |this, cx| {
                                                            this.notif_settings(cx)
                                                        })
                                                    }),
                                            )
                                        })
                                })
                                // Its own log: the last lines of ~/.bench/server.log, where a
                                // problem says itself before the status has anything to say.
                                .child(
                                    v_flex()
                                        .px_3()
                                        .py_2()
                                        .gap_px()
                                        .border_t_1()
                                        .border_color(theme.border)
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("LOG · ~/.bench/server.log"),
                                        )
                                        .when(log.is_empty(), |d| {
                                            d.child(
                                                div()
                                                    .text_sm()
                                                    .text_color(muted)
                                                    .child("Nothing logged"),
                                            )
                                        })
                                        .children(log.iter().map(|l| {
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .whitespace_normal()
                                                .child(l.clone())
                                        })),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_1p5()
                                        .border_t_1()
                                        .border_color(theme.border)
                                        .text_sm()
                                        .text_color(muted)
                                        .whitespace_normal()
                                        .child(where_.clone()),
                                )
                        }),
                )
                .into_any_element(),
        )
    }
}
