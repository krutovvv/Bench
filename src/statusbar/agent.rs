//! The agent's item in the status bar: the open thread's agent — connected or not, how many
//! threads it is working on — and on a click Reconnect and Restart, whether macOS lets Bench say
//! it is done, and the last lines of its log (read as the panel opens).

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The open thread's agent — always, ready or not — and, on click, what more there is to
    /// say about it.
    pub(super) fn render_backend(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let at = self.thread_ref(cx);
        let host = self.thread_host(&at);
        let link = self.agents.get(&host);
        let connected = link.is_some_and(|a| a.conn.is_some()) || self.demo;
        let error = link.and_then(|a| a.error.clone());
        // Always shown, healthy or not: the person decides whether to look, not a late error.
        let working = self
            .threads.list
            .iter()
            .filter(|t| t.host == host && t.running)
            .count();
        let (label, color): (String, Hsla) = if connected && working > 0 {
            (format!("Agent · {working} working"), theme.primary)
        } else if connected {
            ("Agent".into(), theme.primary)
        } else if error.is_some() {
            ("Agent offline".into(), theme.danger)
        } else {
            ("Connecting…".into(), muted)
        };
        let log = self.agent_log.clone();
        // Where it runs: this Mac, or its project's server.
        let where_ = self.target_of(&host).label();
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
                                    .child(div().text_size(px(bench_ui::BAR_TEXT)).child(label))
                                    .text_color(color)
                                    .on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| {
                                            this.notif(false, cx);
                                            this.load_agent_log(cx);
                                        })
                                    })
                            }
                        )
                        .content(move |_, _, cx| {
                            let theme = cx.theme().clone();
                            let muted = theme.muted_foreground;
                            let dot = |ok: bool, warn: bool| {
                                div().size(px(bench_ui::DOT)).bg(if ok {
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
                                    .child(div().pt(px(bench_ui::ICON_ON_SMALL_LINE + bench_ui::ICON_ON_LINE)).child(dot(ok, warn)))
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
                            let (host, restart_host) = (host.clone(), host.clone());
                            let mut lines = vec![line(
                                connected,
                                false,
                                if connected {
                                    "Connected".into()
                                } else {
                                    "Not connected".into()
                                },
                            )];
                            if let Some(e) = &error {
                                lines.push(line(false, false, e.clone()));
                            }
                            v_flex()
                                .w(px(bench_ui::POPOVER_L))
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
                                                    let h = host.clone();
                                                    v1.update(cx, |this, cx| {
                                                        this.reconnect_agent(h, window, cx)
                                                    })
                                                }),
                                        )
                                        .child(div().w(px(bench_ui::HAIRLINE)).h(px(bench_ui::BADGE)).bg(theme.border))
                                        .child(
                                            icon(
                                                "backend-restart",
                                                Ico::Power,
                                                "Restart the agent",
                                            )
                                            .on_click(
                                                move |_, window, cx| {
                                                    let h = restart_host.clone();
                                                    v2.update(cx, |this, cx| {
                                                        this.update_agent(h, window, cx)
                                                    })
                                                },
                                            ),
                                        ),
                                )
                                // Whether macOS lets Bench speak at all: the one thing that
                                // stands between "the agent finished" and silence.
                                .child({
                                    let v = view.clone();
                                    let w = view.clone();
                                    let auth = crate::system::notify::AUTH
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
