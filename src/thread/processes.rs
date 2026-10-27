//! The agent's background processes in the open session (`process.list`): polled every few
//! seconds while a session is open, shown as a card under the last answer, each one stoppable
//! (`process.kill`) and with its latest output a click away.

use gpui_kit::*;

use crate::app::BenchApp;
use crate::app::model::BgProcess;
use crate::client::rpc::Cmd;
use crate::media::text::plain_text;
use bench_ui::clock;
use bench_ui::icons::Ico;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;

impl BenchApp {
    /// Ask the gateway again (the poll, and right after a Stop).
    pub(crate) fn poll_processes(&self) {
        if let (Some(profile), Some(session)) =
            (self.profile.clone(), self.active_thread.runtime.clone())
        {
            self.cmd(Cmd::Processes { profile, session });
        }
    }

    pub(crate) fn on_processes(&mut self, list: Vec<BgProcess>, cx: &mut Context<Self>) {
        // Running ones first, then the newest.
        let mut list = list;
        list.sort_by(|a, b| {
            b.running.cmp(&a.running).then(
                b.started
                    .unwrap_or(0.)
                    .partial_cmp(&a.started.unwrap_or(0.))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let running = list.iter().any(|p| p.running);
        if list != self.active_thread.processes || running {
            // Running ones repaint for their clocks.
            self.active_thread.processes = list;
            cx.notify();
        }
    }

    pub(crate) fn kill_process(&mut self, id: String, cx: &mut Context<Self>) {
        if let (Some(profile), Some(session)) =
            (self.profile.clone(), self.active_thread.runtime.clone())
        {
            self.cmd(Cmd::KillProcess {
                profile,
                session,
                id,
            });
            self.poll_processes();
        }
        cx.notify();
    }

    /// A row's output shown or hidden.
    pub(crate) fn toggle_process_output(&mut self, id: String, cx: &mut Context<Self>) {
        if self.active_thread.process_open.as_deref() == Some(id.as_str()) {
            self.active_thread.process_open = None;
        } else {
            self.active_thread.process_open = Some(id);
        }
        cx.notify();
    }
}

impl BenchApp {
    /// The background processes the agent started in this session, as a row of its own where the
    /// work began: running or finished, each with Stop (while it runs) and its latest output a
    /// click away.
    pub(crate) fn render_processes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.active_thread.processes.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0., |d| d.as_secs_f64());
        let running = self
            .active_thread
            .processes
            .iter()
            .filter(|p| p.running)
            .count();
        let done = self.active_thread.processes.len() - running;
        let open = self.active_thread.processes_open;
        let mut bits = vec![];
        if running > 0 {
            bits.push(format!("{running} running"));
        }
        if done > 0 {
            bits.push(format!("{done} finished"));
        }
        let rows: Vec<AnyElement> = self
            .active_thread
            .processes
            .iter()
            .filter(|_| open)
            .enumerate()
            .map(|(k, p)| {
                let failed = !p.running && p.exit_code.is_some_and(|c| c != 0);
                let elapsed = p
                    .started
                    .map(|t| (p.exited.unwrap_or(now) - t).max(0.) as u64)
                    .map(clock);
                let state = if p.running {
                    elapsed.unwrap_or_default()
                } else {
                    let code = p
                        .exit_code
                        .map_or("ended".to_string(), |c| format!("exit {c}"));
                    // A reason that only says it ended adds nothing to "exit N".
                    let reason = p.reason.as_deref().filter(|r| {
                        !r.is_empty() && !matches!(*r, "exited" | "exit" | "completed")
                    });
                    match (reason, elapsed) {
                        (Some(r), Some(e)) => format!("{code} · {r} · {e}"),
                        (_, Some(e)) => format!("{code} · {e}"),
                        _ => code,
                    }
                };
                let shown = self.active_thread.process_open.as_deref() == Some(p.id.as_str());
                let (id_toggle, id_kill) = (p.id.clone(), p.id.clone());
                // "cd <project> && cargo build": the folder is the session's, the rest is news.
                let shown_cmd = match p.command.strip_prefix("cd ") {
                    Some(rest) => rest
                        .split_once(" && ")
                        .map_or(p.command.as_str(), |(_, cmd)| cmd),
                    None => p.command.as_str(),
                }
                .to_string();
                let full_cmd = p.command.clone();
                let tail: String = {
                    let lines: Vec<&str> = p.tail.trim_end().lines().collect();
                    lines[lines.len().saturating_sub(60)..].join("\n")
                };
                v_flex()
                    // Rules between the processes, not under the head.
                    .when(k > 0, |d| d.border_t_1().border_color(theme.border))
                    .child(
                        h_flex()
                            .id(("proc", k))
                            .px_2()
                            .py_1()
                            .gap_2()
                            .items_center()
                            .text_base()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(if p.running {
                                Ico::Loader.live(false)
                            } else if failed {
                                Ico::CircleAlert
                                    .icon()
                                    .xsmall()
                                    .text_color(theme.danger)
                                    .into_any_element()
                            } else {
                                Ico::CircleCheck
                                    .icon()
                                    .xsmall()
                                    .text_color(muted)
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .id(("proc-cmd", k))
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(bench_ui::theme::MONO)
                                    .child(shown_cmd)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(full_cmd.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(if failed { theme.danger } else { muted })
                                    .child(state),
                            )
                            .when(p.running, |d| {
                                d.child(
                                    div()
                                        .id(("proc-stop", k))
                                        .flex_none()
                                        .px_1p5()
                                        .text_color(muted)
                                        .hover(|s| {
                                            s.bg(theme.danger).text_color(theme.danger_foreground)
                                        })
                                        .child("Stop")
                                        .tooltip(|window, cx| {
                                            Tooltip::new("Stop this process").build(window, cx)
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.kill_process(id_kill.clone(), cx)
                                        })),
                                )
                            })
                            .child(
                                if shown {
                                    Ico::ChevronDown
                                } else {
                                    Ico::ChevronRight
                                }
                                .icon()
                                .xsmall()
                                .text_color(muted),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_process_output(id_toggle.clone(), cx)
                            })),
                    )
                    .when(shown, |d| {
                        d.child(
                            div()
                                .mx_2()
                                .mb_2()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .font_family(bench_ui::theme::MONO)
                                .text_base()
                                .text_color(theme.foreground)
                                .child(if tail.trim().is_empty() {
                                    div()
                                        .text_color(muted)
                                        .child("No output yet")
                                        .into_any_element()
                                } else {
                                    plain_text(format!("proc-out-{k}"), &tail).into_any_element()
                                }),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let head = bench_ui::fold_head(
            "procs-head",
            open,
            Some(Ico::Terminal),
            format!("Background · {}", bits.join(" · ")),
            cx,
        )
        .when(running > 0, |d| d.child(Ico::Loader.live(false)))
        .when(!open, |d| d.child(div().flex_1()))
        .on_click(cx.listener(|this, _, _, cx| {
            // Folding is the thread's own click: it must not pull the focus to the composer.
            this.click_claimed = true;
            this.active_thread.processes_open = !this.active_thread.processes_open;
            cx.notify();
        }));
        let frame = v_flex()
            .id("procs-block")
            .w_full()
            .min_w_0()
            .flex_none()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            .child(head)
            .when(open, |d| d.children(rows));
        // The agent's row, as its plan's: its avatar, the block in the text column.
        Some(
            h_flex()
                .w_full()
                .flex_none()
                .id("procs-row")
                .pl(px(bench_ui::CHAT_LEFT))
                .pr(px(bench_ui::CHAT_RIGHT))
                .py(px(bench_ui::TURN_Y))
                .gap(px(bench_ui::CHAT_TEXT
                    - bench_ui::CHAT_LEFT
                    - bench_ui::AVATAR))
                .items_start()
                .child(bench_ui::avatar(true, &theme))
                .child(div().flex_1().min_w_0().child(frame))
                .into_any_element(),
        )
    }
}
