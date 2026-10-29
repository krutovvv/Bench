//! The opened work as a tree in one column, as the Explorer's: calls of one kind in a row folded
//! into a run as a Finder folder is ("Ran 6 commands"), the last run open — what the agent does
//! now — each call an action and where, quiet; a click opens a call's card in place under it:
//! why the agent made it, what the evaluator said, what was sent, what came back. Nothing
//! scrolls but a long command or output inside its own field. What a call is said as is
//! `::thread::steps`.

use ::thread::Step;
use ::thread::steps::{self, Kind, Line};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use crate::thread::rows::tool_labels::pretty;
use bench_ui::clip;
use bench_ui::icons::Ico;

/// How much of the reasoning a card shows before "Show all".
const WHY_LINES: usize = 5;
/// The lines a command or output field holds before it scrolls inside.
const FIELD_LINES: usize = 10;

impl ThreadView {
    pub(crate) fn render_steps(
        &mut self,
        key: &str,
        steps: &[Step],
        _live: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let lines = steps::runs(steps);
        let last = lines.len().saturating_sub(1);
        let mut rows: Vec<AnyElement> = vec![];
        for (i, line) in lines.into_iter().enumerate() {
            match line {
                Line::Step(k) => self.push_step(key, k, steps, false, &mut rows, cx),
                Line::Run { kind, calls } => {
                    let flipped = self
                        .thread
                        .flipped_runs
                        .contains(&(key.to_string(), calls[0]));
                    let open = (i == last) != flipped;
                    rows.push(self.render_run_row(key, steps, kind, &calls, open, cx));
                    if open {
                        for k in calls {
                            self.push_step(key, k, steps, true, &mut rows, cx);
                        }
                    }
                }
            }
        }
        v_flex().p_1p5().children(rows).into_any_element()
    }

    /// A step's row, and its card under it when open; `inner` under a run.
    fn push_step(
        &mut self,
        key: &str,
        k: usize,
        steps: &[Step],
        inner: bool,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let open = self.thread.open_steps.contains(&(key.to_string(), k));
        rows.push(self.render_step_row(key, k, &steps[k], inner, open, cx));
        if open {
            let card = self.render_step_card(key, k, steps, cx);
            rows.push(
                div()
                    .when(inner, |d| d.pl_6())
                    .pl_6()
                    .pr_2()
                    .pt_0p5()
                    .pb_2()
                    .child(card)
                    .into_any_element(),
            );
        }
    }

    /// A run, as a Finder folder: the disclosure, what was done ("Ran 6 commands"), how many
    /// failed, whether the evaluator spoke to any.
    fn render_run_row(
        &mut self,
        key: &str,
        steps: &[Step],
        kind: Kind,
        calls: &[usize],
        open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let faint = bench_ui::faint(&theme);
        let failed = calls
            .iter()
            .filter(|k| matches!(steps[**k], Step::Tool { failed: true, .. }))
            .count();
        let judged = calls.iter().any(|k| judged(&steps[*k]));
        let flip = (key.to_string(), calls[0]);
        h_flex()
            .id(SharedString::from(format!("steprun-{key}-{}", calls[0])))
            .min_h(px(bench_ui::LIST_ROW_H))
            .px_2()
            .gap_2()
            .items_center()
            .rounded_md()
            .text_size(px(bench_ui::THREAD_META))
            .cursor_pointer()
            .hover(|s| s.bg(theme.list_hover))
            .child(
                div()
                    .flex_none()
                    .text_color(faint)
                    .child(if open { Ico::ChevronDown } else { Ico::ChevronRight }.icon().xsmall()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.foreground)
                    .child(steps::title(steps, kind, calls)),
            )
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
            .on_click(cx.listener(move |this, _, _, cx| {
                this.claim_click(cx);
                if !this.thread.flipped_runs.remove(&flip) {
                    this.thread.flipped_runs.insert(flip.clone());
                }
                cx.notify();
            }))
            .into_any_element()
    }

    /// A step's row: its icon, the action, where, quieter; a failure in the danger's colour with
    /// the first line of what went wrong at the end; the evaluator's word a quiet mark. A click
    /// opens or closes its card; nothing else moves.
    fn render_step_row(
        &mut self,
        key: &str,
        k: usize,
        step: &Step,
        inner: bool,
        open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let faint = bench_ui::faint(&theme);
        let said = steps::say(step);
        let failed = matches!(step, Step::Tool { failed: true, .. });
        let why_failed = match step {
            Step::Tool {
                failed: true,
                output: Some(o),
                ..
            } => o
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.contains("[Evaluator"))
                .map(|l| clip(l, 80)),
            _ => None,
        };
        let (text, quiet) = if failed {
            (theme.danger, theme.danger.opacity(0.75))
        } else if matches!(step, Step::Thought(_) | Step::Note(_)) {
            (theme.muted_foreground, faint)
        } else {
            (theme.foreground, faint)
        };
        let toggle = (key.to_string(), k);
        h_flex()
            .id(SharedString::from(format!("steprow-{key}-{k}")))
            .min_h(px(bench_ui::LIST_ROW_H))
            .when(inner, |d| d.pl_6())
            .when(!inner, |d| d.pl_2())
            .pr_2()
            .gap_2()
            .items_center()
            .rounded_md()
            .text_size(px(bench_ui::THREAD_META))
            .cursor_pointer()
            .when(open, |d| d.bg(theme.accent))
            .when(!open, |d| d.hover(|s| s.bg(theme.list_hover)))
            .child(
                div()
                    .flex_none()
                    .text_color(if failed { quiet } else { theme.muted_foreground })
                    .child(if failed { Ico::CircleAlert } else { step_icon(step) }.icon().xsmall()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .items_baseline()
                    .child(div().min_w_0().truncate().text_color(text).child(said.action))
                    .when_some(said.place, |d, p| {
                        d.child(
                            div()
                                .flex_none()
                                .max_w_1_3()
                                .truncate()
                                .text_size(px(bench_ui::THREAD_DETAIL))
                                .text_color(quiet)
                                .child(p),
                        )
                    }),
            )
            .when_some(why_failed, |d, w| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .max_w_2_5()
                        .truncate()
                        .text_size(px(bench_ui::THREAD_DETAIL))
                        .text_color(quiet)
                        .child(w),
                )
            })
            .when(judged(step), |d| {
                d.child(div().flex_none().text_color(faint).child(Ico::Scale.icon().xsmall()))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.claim_click(cx);
                if !this.thread.open_steps.remove(&toggle) {
                    this.thread.open_steps.insert(toggle.clone());
                }
                cx.notify();
            }))
            .into_any_element()
    }

    /// A step's card, under its row: why, the evaluator's word, what was sent, what came back —
    /// the journal's own text, selectable. A long command or output scrolls in its field.
    fn render_step_card(
        &mut self,
        key: &str,
        k: usize,
        steps: &[Step],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let mut parts: Vec<(&str, String, bool)> = vec![];
        match &steps[k] {
            Step::Tool {
                name,
                args,
                output,
                diff,
                ..
            } => {
                parts.push(("Why", steps::reasoning_before(steps, k).join("\n\n"), false));
                if let Some(word) = output.as_deref().and_then(steps::evaluator_word) {
                    parts.push(("Evaluator", word.to_string(), false));
                }
                match args.get("command").and_then(|c| c.as_str()) {
                    Some(c) if Kind::of(name) == Kind::Command => {
                        parts.push(("Command", c.to_string(), true))
                    }
                    _ => parts.push(("Sent", pretty(args), true)),
                }
                if let Some(d) = diff {
                    parts.push(("Changes", d.clone(), true));
                }
                parts.push(("Received", output.clone().unwrap_or_default(), true));
            }
            Step::Note(t) => parts.push(("The agent said", t.clone(), false)),
            Step::Thought(t) => parts.push(("Thinking", t.clone(), false)),
        }
        parts.retain(|(_, body, _)| !body.trim().is_empty());
        let whole = self.thread.step_whole.contains(&(key.to_string(), k));
        let mut cut = false;
        if !whole {
            for (_, body, code) in parts.iter_mut() {
                if !*code && body.lines().count() > WHY_LINES {
                    *body = body.lines().take(WHY_LINES).collect::<Vec<_>>().join("\n");
                    cut = true;
                }
            }
        }
        let sections: Vec<AnyElement> = parts
            .into_iter()
            .map(|(title, body, code)| {
                let id = format!("stepcard-{key}-{k}-{title}");
                let tall = code && body.lines().count() > FIELD_LINES;
                let text = self.thread.texts.plain(&id, &body, cx);
                // A field: the command, the output. Past its height it scrolls inside with its
                // padding kept, as a text field does; the thread around it never does.
                let field = v_flex()
                    .bg(theme.secondary)
                    .border_1()
                    .rounded_md()
                    .border_color(theme.border.opacity(0.7))
                    .font_family(bench_ui::theme::MONO)
                    .text_size(theme.mono_font_size)
                    .text_color(theme.foreground);
                let shown = if tall {
                    let scroll = self.work_scroll.entry(id.clone()).or_default().clone();
                    field
                        .h(px(bench_ui::STEP_FIELD_MAX_H))
                        .child(crate::inserts::scroll_pane(
                            SharedString::from(id),
                            &scroll,
                            div().px_2p5().py_2().child(text),
                        ))
                        .into_any_element()
                } else if code {
                    field.child(div().px_2p5().py_2().child(text)).into_any_element()
                } else {
                    div()
                        .text_size(px(bench_ui::THREAD_META))
                        .text_color(theme.foreground)
                        .child(text)
                        .into_any_element()
                };
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(bench_ui::THREAD_DETAIL))
                            .text_color(theme.muted_foreground)
                            .child(title),
                    )
                    .child(shown)
                    .into_any_element()
            })
            .collect();
        let toggle = (key.to_string(), k);
        v_flex()
            .px_3p5()
            .py_3()
            .gap_3()
            .bg(theme.background)
            .border_1()
            .rounded_lg()
            .border_color(theme.border.opacity(0.7))
            .children(sections)
            .when(cut || whole, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("stepwhole-{key}-{k}")))
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
