//! The agent's background processes (`process start`, Pi tasks of the conversation): a card in
//! the work whose call started them, as their records come in `thread.process` events — running
//! or ended, each one stoppable (`process.kill`) and with its latest output a click away.

use gpui_kit::*;

use super::ThreadView;
use crate::media::text::plain_text;
use ::thread::{Process, ProcessState};
use bench_ui::clock;
use bench_ui::icons::Ico;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;

impl ThreadView {
    pub(crate) fn kill_process(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(thread) = self.thread.id.clone() {
            self.tell(protocol::StopProcess { thread, process: id }, cx);
        }
        cx.notify();
    }

    /// A row's output shown or hidden.
    pub(crate) fn toggle_process_output(&mut self, id: String, cx: &mut Context<Self>) {
        if self.thread.process_open.as_deref() == Some(id.as_str()) {
            self.thread.process_open = None;
        } else {
            self.thread.process_open = Some(id);
        }
        cx.notify();
    }
}

impl ThreadView {
    /// The background processes the calls of the work `work` started, as a card of that work:
    /// running or ended, each with Stop (while it runs) and its latest output a click away.
    /// `None` when there are none, so the work row shows nothing.
    pub(crate) fn processes_frame(
        &self,
        work: &str,
        processes: &[Process],
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if processes.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        // Times are ms, like the journal's.
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0., |d| d.as_secs_f64() * 1000.);
        let running = processes
            .iter()
            .filter(|p| p.state == ProcessState::Running)
            .count();
        let done = processes.len() - running;
        let open = self.thread.processes_open.contains(work);
        let mut bits = vec![];
        if running > 0 {
            bits.push(format!("{running} running"));
        }
        if done > 0 {
            bits.push(format!("{done} ended"));
        }
        let rows: Vec<AnyElement> = processes
            .iter()
            .filter(|_| open)
            .enumerate()
            .map(|(k, p)| {
                let live = p.state == ProcessState::Running;
                let failed = p.state == ProcessState::Failed
                    || (p.state == ProcessState::Exited && p.exit_code.is_some_and(|c| c != 0));
                let elapsed = p
                    .started
                    .map(|t| ((p.ended.unwrap_or(now) - t) / 1000.).max(0.) as u64)
                    .map(clock);
                let state = if live {
                    elapsed.unwrap_or_default()
                } else {
                    let how = match p.state {
                        ProcessState::Exited => p
                            .exit_code
                            .map_or("ended".to_string(), |c| format!("exit {c}")),
                        ProcessState::Stopped => "stopped".into(),
                        ProcessState::Restarted => "stopped by restart".into(),
                        _ => "could not start".into(),
                    };
                    match elapsed {
                        Some(e) => format!("{how} · {e}"),
                        None => how,
                    }
                };
                let shown = self.thread.process_open.as_deref() == Some(p.id.as_str());
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
                    let out = match &p.error {
                        Some(e) if p.tail.trim().is_empty() => e.as_str(),
                        _ => p.tail.as_str(),
                    };
                    let lines: Vec<&str> = out.trim_end().lines().collect();
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
                            .child(if live {
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
                            .when(live, |d| {
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
                                    plain_text(format!("proc-out-{}", p.id), &tail)
                                        .into_any_element()
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
        .on_click({
            let work = work.to_string();
            cx.listener(move |this, _, _, cx| {
                // Folding is the thread's own click: it must not pull the focus to the composer.
                this.claim_click(cx);
                if !this.thread.processes_open.remove(&work) {
                    this.thread.processes_open.insert(work.clone());
                }
                cx.notify();
            })
        });
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
        // It stands in the work row, beside the plan and the questions: no avatar of its own.
        Some(frame.into_any_element())
    }
}
