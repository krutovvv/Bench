//! The left dock's foot, under the threads or the Explorer, as Claude's account row: the open thread's agent — where it runs, a dot
//! for connected or not, how many threads it is working on — and on a click Reconnect and
//! Restart, whether macOS lets Bench say it is done, and the last lines of its log (read as the
//! panel opens); the gear (⌘,) at the other end.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The foot: the open thread's agent — always, ready or not — and, on click, what more there
    /// is to say about it; the gear.
    pub(crate) fn render_foot(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let at = self.thread_ref(cx);
        let host = self.thread_host(&at);
        let link = self.agents.get(&host);
        let connected = link.is_some_and(|a| a.conn.is_some());
        let error = link.and_then(|a| a.error.clone());
        // Always shown, healthy or not: the person decides whether to look, not a late error.
        let working = self
            .threads.list
            .iter()
            .filter(|t| t.host == host && t.running)
            .count();
        // Where it runs (this Mac, or its project's server) and a small dot that says the state —
        // green up, grey connecting, red down; then what it does, quieter.
        let (state, dot): (String, Hsla) = if connected && working > 0 {
            (format!("{working} working"), theme.success)
        } else if connected {
            (String::new(), theme.success)
        } else if error.is_some() {
            ("offline".into(), theme.danger)
        } else {
            ("connecting…".into(), bench_ui::faint(&theme))
        };
        let state_color = if error.is_some() && !connected { theme.danger } else { muted };
        let log = self.sidebar.agent_log.clone();
        let where_ = self.target_of(&host).label();
        let name = where_.clone();
        let view = cx.entity();
        h_flex()
            .flex_none()
            .h(px(bench_ui::SIDEBAR_FOOT_H))
            .pl(px(bench_ui::SIDEBAR_ROW_INSET))
            .pr(px(bench_ui::SIDEBAR_ROW_INSET))
            .items_center()
            .border_t_1()
            .border_color(theme.border.opacity(0.6))
            .child(
                h_flex().flex_1().min_w_0().child(
                    gpui_kit::component::popover::Popover::new("backend-pop")
                        .anchor(gpui_kit::Anchor::BottomLeft)
                        // Its sections pad themselves, so their rules run edge to edge as a
                        // Mac popover's separators do (the panel's own padding inset them).
                        .p(Gap::Zero)
                        .default_open(self.sidebar.agent_popover_open)
                        .trigger(
                            // Opening the panel asks macOS what it allows now (the choice may
                            // have changed in System Settings since the start).
                            {
                                let v = view.clone();
                                Button::new("backend")
                                    .ghost()
                                    .small()
                                    .child(
                                        bench_ui::row(Gap::S)
                                            .text_size(px(bench_ui::SIDEBAR_TEXT))
                                            .child(bench_ui::dot(dot))
                                            .child(div().text_color(theme.foreground).child(name))
                                            .when(!state.is_empty(), |d| {
                                                d.child(div().text_color(state_color).child(state))
                                            }),
                                    )
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
                                bench_ui::dot(if ok {
                                    theme.success
                                } else if warn {
                                    theme.warning
                                } else {
                                    theme.danger
                                })
                            };
                            let line = |ok: bool, warn: bool, text: String| {
                                bench_ui::row(Gap::M)
                                    .text_size(px(bench_ui::SIDEBAR_TEXT))
                                    .child(dot(ok, warn))
                                    // A URL or a refusal is longer than the panel: it wraps
                                    // inside it, never over its edge.
                                    .child(div().flex_1().min_w_0().whitespace_normal().child(text))
                            };
                            let icon = |id: &'static str, ico: Ico, tip: &'static str| {
                                div()
                                    .id(id)
                                    .p(Gap::Xs)
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
                                .child(bench_ui::popover_section(false, cx).child(
                                    h_flex()
                                        .gap(Gap::M)
                                        .items_start()
                                        .child(v_flex().flex_1().min_w_0().gap(Gap::Xs).children(lines))
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
                                ))
                                // Whether macOS lets Bench speak at all: the one thing that
                                // stands between "the agent finished" and silence.
                                .child({
                                    let v = view.clone();
                                    let w = view.clone();
                                    let auth = crate::system::notify::AUTH
                                        .load(std::sync::atomic::Ordering::Relaxed);
                                    bench_ui::popover_section(true, cx)
                                        .gap(Gap::Xs)
                                        .child(
                                            bench_ui::row(Gap::M)
                                                .child(bench_ui::popover_title("Notifications", cx))
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
                                                    .text_size(px(bench_ui::SIDEBAR_TEXT))
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
                                    bench_ui::popover_section(true, cx)
                                        .gap(Gap::Hair)
                                        .child(bench_ui::popover_title("Log · ~/.bench/server.log", cx))
                                        .when(log.is_empty(), |d| {
                                            d.child(
                                                div()
                                                    .text_size(px(bench_ui::SIDEBAR_TEXT))
                                                    .text_color(muted)
                                                    .child("Nothing logged"),
                                            )
                                        })
                                        .children(log.iter().enumerate().map(|(i, l)| {
                                            // One line each, cut with an ellipsis: a log row
                                            // never grows the panel; the whole line is under the
                                            // pointer on hover.
                                            let tip: SharedString = l.clone().into();
                                            div()
                                                .id(format!("agent-log-{i}"))
                                                .w_full()
                                                .text_size(px(bench_ui::SIDEBAR_HEAD_TEXT))
                                                .text_color(muted)
                                                .truncate()
                                                .child(l.clone())
                                                .tooltip(move |window, cx| {
                                                    gpui_kit::component::tooltip::Tooltip::new(
                                                        tip.clone(),
                                                    )
                                                    .build(window, cx)
                                                })
                                        })),
                                )
                                .child(
                                    bench_ui::popover_section(true, cx)
                                        .text_size(px(bench_ui::SIDEBAR_TEXT))
                                        .text_color(muted)
                                        .whitespace_normal()
                                        .child(where_.clone()),
                                )
                        }),
                ),
            )
            .child(
                bench_ui::icon_button("open-settings", Ico::Settings, "Settings  ⌘,", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.on_open_settings(&crate::app::menu::OpenSettings, window, cx)
                    })),
            )
            .into_any_element()
    }
}
