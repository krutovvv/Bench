//! The agent's work between two messages: one line "N actions · time · Done", the steps under
//! it when opened. What one call says in a line is `tool_labels.rs`.

use ::thread::{AskCard, Hold, PlanStep, Step, worked_ms};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

use crate::thread::ThreadView;
use ::thread::tool_labels::activity_word;
use crate::app::now_secs;
use bench_ui::icons::Ico;
use bench_ui::{clip, clock, plural};

impl ThreadView {
    pub(crate) fn render_work(
        &mut self,
        key: &str,
        steps: &[Step],
        from: Option<f64>,
        to: Option<f64>,
        live: bool,
        // The person's holds of the live run: its clock skips them; the last open one means
        // it stands held, and the hold's own button shows it.
        holds: &[Hold],
        plan: Option<&(String, Vec<PlanStep>)>,
        asks: &[AskCard],
        processes: &[::thread::Process],
        // The answer this work leads to, when the run has ended: its read-aloud and copy
        // ride this line's far edge, the exact place Pause and Stop held while the run
        // lived — standing there from the start, shown on hover.
        answer: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let faint = bench_ui::faint(&theme);
        let tools = steps.iter().filter(|s| s.is_tool()).count();
        let errors = steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { failed: true, .. }))
            .count();
        let open = self.thread.open_work.contains(key);
        let paused = live && holds.last().is_some_and(|h| h.to.is_none());
        // A live piece of work counts on — but not while held: it stands, then goes on from
        // there; a finished one keeps the time its steps took.
        let secs = match (from, to, live) {
            (Some(a), _, true) => Some(worked_ms(a, now_secs() * 1000., holds) / 1000.),
            (Some(a), Some(b), false) => Some((b - a).max(0.) / 1000.),
            _ => None,
        };
        let mut parts: Vec<(String, Hsla)> = vec![];
        if tools > 0 {
            parts.push((plural(tools, "action", "actions", "actions"), muted));
        }
        if errors > 0 {
            parts.push((plural(errors, "error", "errors", "errors"), theme.danger));
        }
        if let Some(s) = secs.filter(|s| *s >= 1.) {
            parts.push((clock(s as u64), muted));
        }
        let has_parts = !parts.is_empty();
        // What the agent is doing now, in a line: the words it wrote — the narrative of what
        // it is after — and, when a call is the newest thing, a short word for its kind after
        // a dot: the story and the concrete step at once. Commands and paths stay where they
        // belong — in the opened step list. Before any word: the neutral one. Held: no
        // shimmer, and the hold's button says why.
        let last_live = steps.iter().rev().find(|s| !matches!(s, Step::Thought(_)));
        let narrative = steps.iter().rev().find_map(|s| match s {
            Step::Note(t) => t
                .lines()
                .rev()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| clip(l, 120)),
            _ => None,
        });
        let doing = live.then(|| match last_live {
            // The model's streams are taken: nothing is being done yet (`agent/src/runtime/streams.ts`).
            _ if self.thread.journal.in_line().is_some() => {
                format!("In line · {}", self.thread.journal.in_line().unwrap_or_default())
            }
            // Held: nothing is being done, and the line says so in the step's place.
            _ if paused => match &narrative {
                Some(n) => format!("{n} · Paused"),
                None => "Paused".to_string(),
            },
            Some(Step::Tool { name, preview, .. }) => {
                let w = activity_word(name, preview);
                match narrative.clone() {
                    Some(n) => format!("{n} · {w}"),
                    None => w,
                }
            }
            Some(Step::Note(_)) => narrative.unwrap_or_else(|| "Thinking…".to_string()),
            _ => "Thinking…".to_string(),
        });
        let work_key = key.to_string();
        // The tail's two, in the transport's seat: same buttons' size, same edge, same gap.
        let tail = answer.filter(|_| !live);
        let tgroup: SharedString = format!("worktail-{key}").into();
        let (tplay, tspeak) = tail
            .as_ref()
            .map_or((false, false), |(ak, _)| match self.app(cx) {
                Some(a) => (
                    a.media.playing.as_ref().is_some_and(|(j, _)| j == ak),
                    a.media.speaking.as_deref() == Some(ak),
                ),
                None => (false, false),
            });
        let summary = h_flex()
            .id(SharedString::from(format!("work-{key}")))
            .group(tgroup.clone())
            .gap(Gap::S)
            .items_center()
            .min_w_0()
            .text_size(px(bench_ui::THREAD_META))
            .text_color(muted)
            .cursor_pointer()
            .hover(|s| s.text_color(theme.foreground))
            // The counts in the muted grey; what follows them (the time, the errors keep their
            // own colour) one level fainter, as "Done" is.
            .children(parts.into_iter().enumerate().map(|(k, (t, c))| {
                div()
                    .flex_none()
                    .text_color(if k > 0 && c == muted { faint } else { c })
                    .child(if k == 0 { t } else { format!("· {t}") })
            }))
            .map(|d| match doing {
                Some(label) => {
                    // The narrative continues the counts; with no counts it stands alone.
                    let label = if has_parts {
                        format!("· {label}")
                    } else {
                        label
                    };
                    d.child(div().min_w_0().truncate().child(if paused {
                        div().text_color(muted).child(label).into_any_element()
                    } else {
                        ShimmerText::new(label)
                            .text_size(px(bench_ui::THREAD_META))
                            .text_color(muted)
                            .into_any_element()
                    }))
                }
                // A piece of work without tool calls was the model thinking before it answered.
                None => d.child(div().flex_none().text_color(faint).child(
                    match (has_parts, tools > 0 || errors > 0) {
                        (true, true) => "· Done",
                        (false, true) => "Done",
                        (true, false) => "· Thought",
                        (false, false) => "Thought",
                    },
                )),
            })
            // The hold and the stop, at the far edge while the run is live: the actions are
            // where the minutes go, so the answers to them ride the actions' own line, always
            // in sight, and never make the line jump. A finished run has nothing to hold.
            .when(live, |d| {
                let held = paused;
                let stop_view = self.thread_ref();
                d.child(
                    h_flex()
                        .flex_none()
                        .ml_auto()
                        .gap(Gap::Xs)
                        .child(
                            bench_ui::head_icon_button(
                                "work-hold",
                                if held { Ico::Play } else { Ico::Pause },
                                if held { "Resume" } else { "Pause" },
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    // The button's, not the line's too: it does not fold.
                                    cx.stop_propagation();
                                    this.set_paused(!held, cx);
                                },
                            )),
                        )
                        .child(
                            bench_ui::head_icon_button("work-stop", Ico::Square, "Stop", cx)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(thread) = stop_view.id.clone() {
                                        this.tell(protocol::StopThread { thread }, cx);
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
            // The answer's own two, in the same seat the hold and the stop sat in: read
            // aloud, and copy the agent's markdown. Hidden until the line is hovered —
            // unless that answer is playing or being spoken right now.
            .when_some(tail, |d, (ak, atext)| {
                let copy_id = format!("work-copy-{key}");
                let speak_id = format!("work-speak-{key}");
                let fork_id = format!("work-fork-{key}");
                let ak_copy = ak.clone();
                let ak_fork = ak.clone();
                d.child(
                    h_flex()
                        .flex_none()
                        .ml_auto()
                        .gap(Gap::Xs)
                        .items_center()
                        .when(
                            !(tplay || tspeak) && !self.tools_shown,
                            |s| {
                                let shown = self.reveal_tail.as_deref() == Some(ak.as_str());
                                s.when(!shown, |s| {
                                    s.opacity(0.).group_hover(tgroup, |g| g.opacity(1.))
                                })
                            },
                        )
                        .child(
                            bench_ui::head_icon_button(
                                &fork_id,
                                Ico::GitFork,
                                "Fork from here",
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.fork_at(&ak_fork, cx);
                                },
                            )),
                        )
                        .child(
                            bench_ui::head_icon_button(&copy_id, Ico::Copy, "Copy", cx).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.copy_answer(&ak_copy, window, cx);
                                }),
                            ),
                        )
                        .child(
                            bench_ui::head_icon_button(
                                &speak_id,
                                if tplay {
                                    Ico::Square
                                } else if tspeak {
                                    Ico::Loader
                                } else {
                                    Ico::Volume
                                },
                                if tplay { "Stop" } else { "Read aloud" },
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.app
                                        .update(cx, |app, cx| {
                                            app.toggle_speak(&ak, atext.clone(), window, cx)
                                        })
                                        .ok();
                                },
                            )),
                        ),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                let folded = this.thread.open_work.remove(&work_key);
                if !folded {
                    this.thread.open_work.insert(work_key.clone());
                }
                // Remembered past this visit: back in the thread, the work stays as left.
                if let Some(id) = this.thread.id.as_ref().map(|i| i.as_str().to_string()) {
                    let left = this.work_left.entry(id).or_default();
                    if folded {
                        left.insert(work_key.clone());
                    } else {
                        left.remove(&work_key);
                    }
                }
                cx.notify();
            }));
        // Opened: the calls in order beside the chosen one, whole (`rows/steps.rs`).
        let items: Vec<AnyElement> = if open && !steps.is_empty() {
            vec![self.render_steps(key, steps, live, cx)]
        } else {
            vec![]
        };
        // The steps as a Mac's grouped list (Xcode's build log, System Settings' lists): a soft
        // rounded ground, no rule, rows that light up whole — not a rule down their left with
        // lines hung on it. Open with no steps yet (a run that has only begun): nothing.
        let list = (!items.is_empty()).then(|| bench_ui::block_frame(cx).children(items));
        // The plan, the questions and the background processes this work's calls made, under
        // its steps.
        // A call of this work waiting for the person's approval: its card joins the questions.
        // The request is live state (the call is the journal's): it is asked again after a restart.
        let approval = self.thread.journal.asks().iter().find(|a| {
            a.kind == protocol::AskKind::Approve
                && steps
                    .iter()
                    .any(|s| matches!(s, Step::Tool { call_id, .. } if *call_id == a.id))
        });
        let asks: Vec<AskCard> = asks
            .iter()
            .cloned()
            .chain(approval.map(|a| AskCard {
                id: a.id.clone(),
                ask_kind: "approve".into(),
                question: if a.description.is_empty() {
                    "The agent asks to run this".into()
                } else {
                    format!("The agent asks to run this: {}", a.description)
                },
                detail: Some(a.command.clone()),
                choices: a.choices.clone(),
                multi: false,
                answer: None,
            }))
            .collect();
        let mut artifacts = self.render_work_artifacts(plan, &asks, cx);
        artifacts.extend(self.processes_frame(key, processes, cx));
        // Work that only asked or only planned has no "N actions" line to open: the cards are it.
        let no_steps = steps.is_empty() && !live;
        crate::thread::look::turn(
            Some(true),
            v_flex()
                .gap(Gap::L)
                .when(!no_steps, |d| d.child(summary))
                .children(list)
                .children(artifacts),
            cx,
        )
        .into_any_element()
    }
}





