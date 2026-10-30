//! The opened work as strips of milestones: the agent's own words, or a sentence of its calls
//! where it wrote none, in order, all shut till a click. Inside a milestone its
//! calls in order, each on a row under the milestone's words, its icon where they start: what it
//! did, in the agent's own words where it wrote some, and what came back, beside it. A row opens
//! to its card: why, what the evaluator said, what was sent, what came back. Nothing scrolls but a long command or output inside its own field.
//! What a call is said as is `::thread::steps`.

use ::thread::Step;
use ::thread::steps::{self, Kind, Milestone};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use crate::media::text::{FOLD_AFTER_LINES, render_code};
use ::thread::tool_labels::pretty;
use bench_ui::icons::Ico;

/// How much of the reasoning a card shows before "Show all".
const WHY_LINES: usize = 5;
/// …and its characters, for reasoning written as one long paragraph.
const WHY_CHARS: usize = 420;

impl ThreadView {
    pub(crate) fn render_steps(
        &mut self,
        key: &str,
        steps: &[Step],
        _live: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let milestones = steps::milestones(steps);
        let mut rows: Vec<AnyElement> = vec![];
        // Where each milestone's steps start: past the one before, so the reasoning that led to
        // its first call is its own.
        let mut from = 0usize;
        for m in milestones.iter() {
            let to = m.steps.iter().copied().max().unwrap_or(from);            // One milestone open at a time: only the person's choice, else none.
            let open = match self.thread.open_milestone.get(key) {
                Some(first) => *first == m.steps[0],
                None => false,
            };
            let calls: Vec<usize> = m.steps.iter().copied().filter(|k| steps[*k].is_tool()).collect();
            rows.push(self.render_milestone_row(key, steps, m, &calls, open, cx));
            if open && !calls.is_empty() {
                // Its steps under its strip with air above and below: no frame, no fill.
                let mut kids: Vec<AnyElement> = vec![];
                for k in from..=to.min(steps.len().saturating_sub(1)) {
                    if steps[k].is_tool() {
                        // One left edge for all: the steps start where the milestone does.
                        self.push_step(key, k, steps, &mut kids, cx);
                    }
                }
                rows.push(
                    v_flex().pt_2().pb_3().children(kids).into_any_element(),
                );
            }
            from = to + 1;
        }
        v_flex().p_2p5().gap_0p5().children(rows).into_any_element()
    }

    /// A milestone's strip: the disclosure, the agent's words (or the sentence of its calls), and
    /// at the end how many actions, how many failed, whether the evaluator spoke.
    fn render_milestone_row(
        &mut self,
        key: &str,
        steps: &[Step],
        m: &Milestone,
        calls: &[usize],
        open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let faint = bench_ui::faint(&theme);
        let failed = calls.iter().filter(|k| matches!(steps[**k], Step::Tool { failed: true, .. })).count();
        let judged = calls.iter().any(|k| judged(&steps[*k]));
        let first = m.steps[0];
        h_flex()
            .id(SharedString::from(format!("milestone-{key}-{}", m.steps[0])))
            .min_h(px(bench_ui::LIST_ROW_H))
            .px_2()
            .gap_2()
            .items_center()
            .rounded_md()
            .text_size(px(bench_ui::THREAD_META))
            .when(!calls.is_empty(), |d| d.cursor_pointer())
            // Open, its strip stays as hovered, in the person's bubble's grey: the one grey of
            // the block.
            .when(open && !calls.is_empty(), |d| d.bg(theme.secondary))
            .when(!calls.is_empty() && !open, |d| d.hover(|s| s.bg(theme.secondary)))
            .child(
                div()
                    .flex_none()
                    .text_color(faint)
                    .when(calls.is_empty(), |d| d.invisible())
                    .child(if open { Ico::ChevronDown } else { Ico::ChevronRight }.icon().xsmall()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(if m.words || !calls.is_empty() {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(m.heading.clone()),
            )
            .when(!calls.is_empty(), |d| {
                d.child(
                    div()
                        .flex_none()
                        .text_size(px(bench_ui::THREAD_DETAIL))
                        .text_color(faint)
                        .child(bench_ui::plural(calls.len(), "action", "actions", "actions")),
                )
            })
            .when(failed > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .text_size(px(bench_ui::THREAD_DETAIL))
                        .text_color(theme.danger)
                        .child(format!("{failed} failed")),
                )
            })
            .when(judged, |d| {
                d.child(div().flex_none().text_color(faint).child(Ico::Scale.icon().xsmall()))
            })
            .on_click(cx.listener({
                let key = key.to_string();
                move |this, _, _, cx| {
                    this.claim_click(cx);
                    // Opening one folds the one open; a click on the open one folds it.
                    let to = if open { usize::MAX } else { first };
                    this.thread.open_milestone.insert(key.clone(), to);
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    /// A step's row, and its card under it when open.
    fn push_step(
        &mut self,
        key: &str,
        k: usize,
        steps: &[Step],
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let open = self.thread.open_steps.contains(&(key.to_string(), k));
        rows.push(self.render_step_row(key, k, &steps[k], open, cx));
        if open {
            let card = self.render_step_card(key, k, steps, cx);
            rows.push(card);
        }
    }

    /// A step's row: its icon; what it did — the action and, quieter, the agent's own words for
    /// it — on the left; what came back on the right, a failure in the danger's colour; the
    /// evaluator's word a quiet mark. A click opens or closes its card; nothing else moves.
    fn render_step_row(
        &mut self,
        key: &str,
        k: usize,
        step: &Step,
        open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let faint = bench_ui::faint(&theme);
        let said = steps::say(step);
        let failed = matches!(step, Step::Tool { failed: true, .. });
        let what = steps::detail(step).or(said.place.clone());
        let result = steps::result_line(step);
        let toggle = (key.to_string(), k);
        h_flex()
            .id(SharedString::from(format!("steprow-{key}-{k}")))
            .min_h(px(bench_ui::LIST_ROW_H))
            .pl_2()
            .pr_2()
            .gap_2()
            .items_center()
            .rounded_md()
            .text_size(px(bench_ui::THREAD_META))
            .cursor_pointer()
            // On an open milestone's ground a step lights nothing on hover: the pointer says it
            // opens.
            .child(
                div()
                    .flex_none()
                    // Open, its icon in the text's ink: which row the parts below are of.
                    .text_color(if failed {
                        theme.danger
                    } else if open {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(if failed { Ico::CircleAlert } else { step_icon(step) }.icon().xsmall()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .items_baseline()
                    .child(div().flex_none().text_color(theme.foreground).child(said.action))
                    .when_some(what, |d, w| {
                        d.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(theme.muted_foreground)
                                .child(w),
                        )
                    }),
            )
            // What came back, as Finder's side column: one width on every row, small, quiet,
            // to the right edge.
            .child(
                h_flex()
                    .flex_none()
                    .w_1_3()
                    .min_w_0()
                    .justify_end()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(bench_ui::THREAD_SIDE))
                            .text_color(if failed { theme.danger } else { faint })
                            .children(result),
                    ),
            )
            .when(judged(step), |d| {
                d.child(div().flex_none().text_color(faint).child(Ico::Scale.icon().xsmall()))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.claim_click(cx);
                // One step open at a time in a work, as one milestone is: opening one closes
                // the one open; a click on the open one closes it.
                if !this.thread.open_steps.remove(&toggle) {
                    this.thread.open_steps.retain(|(w, _)| *w != toggle.0);
                    this.thread.open_steps.insert(toggle.clone());
                }
                cx.notify();
            }))
            .into_any_element()
    }

    /// A step's parts, under its row as a changed file's diff is under its row: no frame of
    /// their own, aligned with the row's text — why, the evaluator's word, what was sent, what
    /// came back; code as the answers' code blocks, cut at its first lines with "Show all".
    fn render_step_card(
        &mut self,
        key: &str,
        k: usize,
        steps: &[Step],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        // (title, body, the code block's language — empty for plain output; none: prose)
        let mut parts: Vec<(&str, String, Option<&str>)> = vec![];
        match &steps[k] {
            Step::Tool {
                name,
                args,
                output,
                diff,
                ..
            } => {
                parts.push(("Why", steps::reasoning_before(steps, k).join("\n\n"), None));
                if let Some(word) = output.as_deref().and_then(steps::evaluator_word) {
                    parts.push(("Evaluator", word.to_string(), None));
                }
                match args.get("command").and_then(|c| c.as_str()) {
                    Some(c) if Kind::of(name) == Kind::Command => {
                        parts.push(("Command", c.to_string(), Some("bash")))
                    }
                    _ => parts.push(("Sent", pretty(args), Some("json"))),
                }
                if let Some(d) = diff {
                    parts.push(("Changes", d.clone(), Some("diff")));
                }
                parts.push(("Received", output.clone().unwrap_or_default(), Some("")));
            }
            Step::Note(t) => parts.push(("The agent said", t.clone(), None)),
            Step::Thought(t) => parts.push(("Thinking", t.clone(), None)),
        }
        parts.retain(|(_, body, _)| !body.trim().is_empty());
        let whole = self.thread.step_whole.contains(&(key.to_string(), k));
        let mut cut = false;
        if !whole {
            for (_, body, code) in parts.iter_mut() {
                let long = body.lines().count() > WHY_LINES || body.chars().count() > WHY_CHARS;
                if code.is_none() && long {
                    let head = body.lines().take(WHY_LINES).collect::<Vec<_>>().join("\n");
                    *body = match head.char_indices().nth(WHY_CHARS) {
                        Some((i, _)) => format!("{}…", head[..i].trim_end()),
                        None => head,
                    };
                    cut = true;
                }
            }
        }
        let mut sections: Vec<AnyElement> = vec![];
        for (n, (title, body, code)) in parts.into_iter().enumerate() {
            let id = format!("stepcard-{key}-{k}-{title}");
            // Code as the answers show it: the language on the left, Copy on the right, a long
            // one folded to its first lines with "Show all · N lines".
            let shown = match code {
                Some(lang) => {
                    let md = format!("```{lang}\n{}\n```", body.trim_end());
                    let view = self.thread.texts.markdown(&id, &md, cx);
                    let lines = body.lines().count();
                    let long = lines > FOLD_AFTER_LINES;
                    let opened = self.thread.open_fields.contains(&id);
                    let more = long.then(|| {
                        let id = id.clone();
                        div()
                            .id(SharedString::from(format!("{id}-more")))
                            .px_3()
                            .py_1()
                            .text_size(px(bench_ui::THREAD_DETAIL))
                            .text_color(theme.muted_foreground)
                            .border_t_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .hover(|s| s.text_color(theme.foreground))
                            .child(if opened { "Show less".to_string() } else { format!("Show all · {lines} lines") })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.claim_click(cx);
                                if !this.thread.open_fields.remove(&id) {
                                    this.thread.open_fields.insert(id.clone());
                                }
                                cx.notify();
                            }))
                            .into_any_element()
                    });
                    // The block's head names the part ("Command", "Received"); the fence's
                    // language only colours it.
                    render_code(view, &format!("step-{key}-{k}"), n, title.to_string(), body, long && !opened, more, cx)
                }
                // Words: the model's reasoning as it wrote it, with no label of ours; the
                // evaluator's under its name. A long one cut with "Show all".
                None => {
                    let text = self.thread.texts.plain(&id, &body, cx);
                    let toggle = (key.to_string(), k);
                    v_flex()
                        .gap_1()
                        .when(title != "Why", |d| {
                            d.child(
                                div()
                                    .text_size(px(bench_ui::THREAD_DETAIL))
                                    .text_color(theme.muted_foreground)
                                    .child(title),
                            )
                        })
                        .child(div().text_size(px(bench_ui::THREAD_META)).child(text))
                        .when(cut || whole, |d| {
                            d.child(
                                div()
                                    .id(SharedString::from(format!("{id}-more")))
                                    .text_size(px(bench_ui::THREAD_DETAIL))
                                    .text_color(theme.muted_foreground)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(theme.foreground))
                                    .child(if whole { "Show less" } else { "Show all" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.claim_click(cx);
                                        if !this.thread.step_whole.remove(&toggle) {
                                            this.thread.step_whole.insert(toggle.clone());
                                        }
                                        cx.notify();
                                    })),
                            )
                        })
                        .into_any_element()
                }
            };
            sections.push(shown);
        }
        // The same room on both sides as the row's own; air above and below, as a milestone's
        // steps have.
        v_flex()
            .px_2()
            .pt_2()
            .pb_4()
            .gap_2p5()
            .children(sections)
            .into_any_element()
    }
}

/// Whether the evaluator put its word in this call's result.
fn judged(step: &Step) -> bool {
    matches!(step, Step::Tool { output: Some(o), .. } if steps::evaluator_word(o).is_some())
}

/// A step's icon by what its call does; the agent's words a bubble, reasoning a brain.
fn step_icon(step: &Step) -> Ico {
    match step {
        Step::Tool { name, .. } => match Kind::of(name) {
            Kind::Command => Ico::Terminal,
            Kind::Read => Ico::FileText,
            Kind::Edit => Ico::Pencil,
            Kind::Search => Ico::Search,
            Kind::Web | Kind::Browser => Ico::Globe,
            Kind::Plan => Ico::ListChecks,
            Kind::Other => Ico::Wrench,
        },
        Step::Note(_) => Ico::MessageSquare,
        Step::Thought(_) => Ico::Brain,
    }
}
