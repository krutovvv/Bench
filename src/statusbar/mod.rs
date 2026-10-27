//! Bottom bar across the window: where / which profile on the left; context, reasoning effort,
//! model, gateway restart and settings on the right.

mod agent;
mod context;
pub(crate) mod speed;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use self::context::speed_panel;
use crate::app::BenchApp;
use crate::workspace::Center;
use bench_ui::compact;
use bench_ui::icons::Ico;

impl BenchApp {
    pub(crate) fn render_statusbar(
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
                // Muted from `item`; the accent while its panel is open, so set after it.
                .when(active, |d| d.text_color(theme.primary))
                .text_xs()
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        // A label as wide as its text that opens a menu of choices.
        // `note`: a line above the choices saying what they apply to.
        // A Button's `.label()` takes its font from the control's size, so bar labels are passed
        // as children with the bar's extra-small font.
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
            // Icon-only (an empty label): the icon sits alone in a square button, and the wrapper's
            // px_1 makes the inset 4+4 px a side — the same as the neighbouring items' px_2.
            // With a label: the icon sits inside the button, so hover lights up both.
            let icon_only = label.is_empty();
            let button = Button::new(id).ghost().small().icon(icon.icon());
            let button = if icon_only {
                button.text_xs()
            } else {
                button.child(div().text_xs().child(label))
            };
            let wrap = if icon_only {
                h_flex().h_full().items_center().px_1()
            } else {
                h_flex().h_full().items_center().pl_1()
            };
            wrap.child(
                button
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
                        .child(div().text_xs().child(label))
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
                        .child(div().text_xs().child(label))
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
            .h(px(bench_ui::STATUS_H - 1.))
            .flex_none()
            .w_full()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            // The bar's font: a row's extra-small, like the header and the pane strips.
            .text_xs()
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
                        .text_xs()
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
                    self.center == Center::Providers,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.center == Center::Providers {
                        this.center = Center::Chat;
                        cx.notify();
                    } else {
                        this.open_center(Center::Providers, window, cx);
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
                            .text_xs()
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
