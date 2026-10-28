//! The agent's work between two messages: one line "N actions · time · Done", the steps under
//! it when opened. What one call says in a line is `tool_labels.rs`.

use ::thread::{AskCard, Hold, PlanStep, Step, worked_ms};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use crate::thread::rows::tool_labels::{activity_word, pretty, tool_label};
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
            .gap_1p5()
            .items_center()
            .min_w_0()
            .text_color(muted)
            .cursor_pointer()
            .hover(|s| s.text_color(theme.foreground))
            .children(parts.into_iter().enumerate().map(|(k, (t, c))| {
                div()
                    .flex_none()
                    .text_color(c)
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
                            .text_base()
                            .text_color(muted)
                            .into_any_element()
                    }))
                }
                // A piece of work without tool calls was the model thinking before it answered.
                None => d.child(div().flex_none().child(
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
                        .gap_1()
                        .child(
                            bench_ui::head_icon_button(
                                "work-hold",
                                if held { Ico::Play } else { Ico::Pause },
                                if held { "Resume" } else { "Pause" },
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.claim_click(cx);
                                    this.set_paused(!held, cx);
                                },
                            )),
                        )
                        .child(
                            bench_ui::head_icon_button("work-stop", Ico::Square, "Stop", cx)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.claim_click(cx);
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
                        .gap_1()
                        .items_center()
                        .when(
                            !(tplay || tspeak) && std::env::var("BENCH_TOOLS_SHOWN").is_err(),
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
                                    this.claim_click(cx);
                                    this.fork_at(&ak_fork, cx);
                                },
                            )),
                        )
                        .child(
                            bench_ui::head_icon_button(&copy_id, Ico::Copy, "Copy", cx).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.claim_click(cx);
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
                                    this.claim_click(cx);
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
                this.claim_click(cx);
                // The click takes the auto-opened line over: from now the person decides
                // whether it stays open, also when the run ends.
                if this.thread.auto_work.as_deref() == Some(work_key.as_str()) {
                    this.thread.auto_work = None;
                }
                if !this.thread.open_work.remove(&work_key) {
                    this.thread.open_work.insert(work_key.clone());
                }
                cx.notify();
            }));
        let items: Vec<AnyElement> = if open {
            steps
                .iter()
                .enumerate()
                .map(|(k, s)| self.render_step(key, k, s, cx))
                .collect()
        } else {
            vec![]
        };
        let list = open.then(|| {
            v_flex()
                .pl_3()
                .gap_1()
                .border_l_1()
                .border_color(theme.border)
                .children(items)
        });
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
        bench_ui::turn_row(
            Some(true),
            v_flex()
                .gap_3()
                .when(!no_steps, |d| d.child(summary))
                .children(list)
                .children(artifacts),
            cx,
        )
        .into_any_element()
    }

    /// One step; a click unfolds its details (reasoning, arguments, output).
    fn render_step(
        &mut self,
        key: &str,
        k: usize,
        step: &Step,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.thread.open_steps.contains(&(key.to_string(), k));
        let (icon, color, label, details): (Ico, Hsla, String, Vec<(&str, String)>) = match step {
            Step::Tool {
                name,
                preview,
                args,
                output,
                failed,
                ..
            } => (
                if *failed {
                    Ico::CircleAlert
                } else {
                    Ico::Terminal
                },
                if *failed { theme.danger } else { muted },
                clip(&tool_label(name, preview), 220),
                [
                    ("Arguments", pretty(args)),
                    ("Output", output.clone().unwrap_or_default()),
                ]
                .into_iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .collect(),
            ),
            Step::Note(text) => (
                Ico::ChevronRight,
                theme.foreground.opacity(0.75),
                clip(text, 400),
                vec![],
            ),
            Step::Thought(text) => (
                Ico::Brain,
                muted,
                clip(text, 160),
                vec![("Reasoning", text.clone())],
            ),
        };
        let expandable = !details.is_empty();
        let step_key = (key.to_string(), k);
        let bodies: Vec<(&str, AnyElement)> = if open {
            details
                .into_iter()
                .map(|(title, body)| {
                    let text =
                        self.thread
                            .texts
                            .plain(&format!("step-{key}-{k}-{title}"), &body, cx);
                    (title, text.into_any_element())
                })
                .collect()
        } else {
            vec![]
        };
        v_flex()
            .child(
                h_flex()
                    .id(SharedString::from(format!("step-{key}-{k}")))
                    .gap_2()
                    .items_start()
                    .text_color(color)
                    .when(expandable, |d| {
                        d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
                    })
                    .child(icon.icon().xsmall().mt(px(bench_ui::ICON_ON_SMALL_LINE)))
                    .child(div().flex_1().min_w_0().child(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.claim_click(cx);
                        if !this.thread.open_steps.remove(&step_key) {
                            this.thread.open_steps.insert(step_key.clone());
                        }
                        cx.notify();
                    })),
            )
            .when(open && expandable, |d| {
                d.children(bodies.into_iter().map(|(title, body)| {
                    v_flex()
                        .ml_5()
                        .mt_1()
                        .mb_1()
                        .child(div().text_color(muted).child(title))
                        .child(
                            div()
                                .mt_0p5()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .font_family(bench_ui::theme::MONO)
                                .text_color(theme.foreground)
                                .child(body),
                        )
                }))
            })
            .into_any_element()
    }
}
