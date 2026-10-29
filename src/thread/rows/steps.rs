//! The opened work, as a Mac list beside its detail (Mail, Finder): its calls in order on the
//! left, each an action and where, quiet, calls of one kind in a row folded into a run as a
//! Finder folder is; the chosen one whole on the right — why the agent
//! made it, what the evaluator said, what was sent, what came back. None chosen, the newest:
//! while the work runs, what the agent does now. What a call is said as is `::thread::steps`.

use ::thread::Step;
use ::thread::steps::{self, Kind, Line};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use crate::thread::rows::tool_labels::pretty;
use bench_ui::clip;
use bench_ui::icons::Ico;

/// How much of each part of a call its detail shows before "Show all": the reasoning's lines and
/// the output's.
const WHY_LINES: usize = 6;
const OUTPUT_LINES: usize = 24;
const COMMAND_LINES: usize = 6;

impl ThreadView {
    pub(crate) fn render_steps(
        &mut self,
        key: &str,
        steps: &[Step],
        live: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let listed = steps::listed(steps);
        let chosen = self
            .thread
            .chosen_step
            .get(key)
            .copied()
            .filter(|k| listed.contains(k))
            .or(listed.last().copied());
        // The list: runs folded but the one holding the chosen call, the person's toggles on
        // top. Folded, it stays short, so the detail beside it is in view.
        let mut rows: Vec<AnyElement> = vec![];
        for line in steps::runs(steps) {
            match line {
                Line::Step(k) => {
                    let row = self.render_step_row(key, k, &steps[k], chosen == Some(k), cx);
                    rows.push(row);
                }
                Line::Run { kind, calls } => {
                    let holds = chosen.is_some_and(|c| calls.contains(&c));
                    let flipped = self.thread.flipped_runs.contains(&(key.to_string(), calls[0]));
                    let open = holds != flipped;
                    rows.push(self.render_run_row(key, steps, kind, &calls, open, cx));
                    if open {
                        for k in calls {
                            let row = self.render_step_row(key, k, &steps[k], chosen == Some(k), cx);
                            rows.push(div().pl_5().child(row).into_any_element());
                        }
                    }
                }
            }
        }
        let (list_scroll, detail_scroll) = self
            .work_scroll
            .entry(key.to_string())
            .or_insert_with(|| (ScrollHandle::new(), ScrollHandle::new()))
            .clone();
        // While the work runs and the person has chosen nothing, the list keeps to its newest
        // call: what the agent does now is always in view.
        if live && !self.thread.chosen_step.contains_key(key) {
            list_scroll.scroll_to_bottom();
        }
        let rows_h = rows.len() as f32 * bench_ui::LIST_ROW_H + bench_ui::LIST_ROW_H / 2.;
        let height = rows_h.clamp(bench_ui::STEPS_MIN_H, bench_ui::STEPS_MAX_H);
        let detail = chosen.map(|k| self.render_step_detail(key, k, steps, listed.len(), &detail_scroll, cx));
        h_flex()
            .h(px(height))
            .items_stretch()
            .child(
                v_flex()
                    .flex_none()
                    .w(px(bench_ui::STEPS_LIST_W))
                    .border_r_1()
                    .border_color(theme.border)
                    .child(crate::inserts::scroll_pane(
                        SharedString::from(format!("steplist-{key}")),
                        &list_scroll,
                        v_flex().p_1().children(rows),
                    )),
            )
            .child(v_flex().flex_1().min_w_0().children(detail))
            .into_any_element()
    }

    /// A run in the list, as a Finder folder: the disclosure, what was done ("Ran 4 commands"),
    /// how many failed, whether the evaluator spoke to any.
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
        let judged = calls.iter().any(
            |k| matches!(&steps[*k], Step::Tool { output: Some(o), .. } if steps::evaluator_word(o).is_some()),
        );
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
            .child(div().flex_1().min_w_0().truncate().text_color(theme.foreground).child(steps::title(steps, kind, calls)))
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

    /// A call in the list: its icon, the action, where, quieter; a failure in the danger's
    /// colour, the evaluator's word a quiet mark. The chosen one is lit, as a Mac list's.
    fn render_step_row(
        &mut self,
        key: &str,
        k: usize,
        step: &Step,
        chosen: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let said = steps::say(step);
        let failed = matches!(step, Step::Tool { failed: true, .. });
        let judged = matches!(step, Step::Tool { output: Some(o), .. } if steps::evaluator_word(o).is_some());
        let (text, quiet) = if failed {
            (theme.danger, theme.danger.opacity(0.7))
        } else {
            (theme.foreground, bench_ui::faint(&theme))
        };
        let icon = if failed { Ico::CircleAlert } else { step_icon(step) };
        let pick = (key.to_string(), k);
        h_flex()
            .id(SharedString::from(format!("steprow-{key}-{k}")))
            .min_h(px(bench_ui::LIST_ROW_H))
            .px_2()
            .gap_2()
            .items_center()
            .rounded_md()
            .text_size(px(bench_ui::THREAD_META))
            .cursor_pointer()
            // The chosen one lit as the sidebar's chosen row: grey, never the accent.
            .map(|d| {
                if chosen {
                    d.bg(theme.accent)
                } else {
                    d.hover(|s| s.bg(theme.list_hover))
                }
            })
            .child(
                div()
                    .flex_none()
                    .text_color(if failed { quiet } else { theme.muted_foreground })
                    .child(icon.icon().xsmall()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .items_baseline()
                    .child(div().min_w_0().truncate().text_color(text).child(said.action))
                    .when_some(said.place, |d, p| {
                        d.child(
                            div()
                                .flex_shrink_0()
                                .max_w_1_3()
                                .truncate()
                                .text_size(px(bench_ui::THREAD_DETAIL))
                                .text_color(quiet)
                                .child(p),
                        )
                    }),
            )
            .when(judged, |d| {
                d.child(div().flex_none().text_color(quiet).child(Ico::Scale.icon().xsmall()))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.claim_click(cx);
                this.thread.chosen_step.insert(pick.0.clone(), pick.1);
                cx.notify();
            }))
            .into_any_element()
    }

    /// The chosen call, whole: the action and where, then why, the evaluator's word, what was
    /// sent and what came back — the journal's own text, selectable.
    fn render_step_detail(
        &mut self,
        key: &str,
        k: usize,
        steps: &[Step],
        count: usize,
        scroll: &ScrollHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let step = &steps[k];
        let said = steps::say(step);
        let n = steps::listed(steps).iter().position(|i| *i == k).unwrap_or(0) + 1;
        let mut parts: Vec<(&str, String, bool)> = vec![];
        match step {
            Step::Tool {
                name,
                args,
                output,
                diff,
                ..
            } => {
                let why = steps::reasoning_before(steps, k).join("\n\n");
                parts.push(("Why", why, false));
                if let Some(word) = output.as_deref().and_then(steps::evaluator_word) {
                    parts.push(("Evaluator", word.to_string(), false));
                }
                match args.get("command").and_then(|c| c.as_str()) {
                    Some(c) if Kind::of(name) == Kind::Command => parts.push(("Command", c.to_string(), true)),
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
            for (title, body, code) in parts.iter_mut() {
                let n = if *code { OUTPUT_LINES } else { WHY_LINES };
                let n = if *title == "Command" { COMMAND_LINES } else { n };
                if body.lines().count() > n {
                    *body = body.lines().take(n).collect::<Vec<_>>().join("\n");
                    cut = true;
                }
            }
        }
        let failed = matches!(step, Step::Tool { failed: true, .. });
        let sub = [
            said.place.clone(),
            Some(format!("step {n} of {count}")),
            failed.then(|| "failed".to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let bodies: Vec<AnyElement> = parts
            .into_iter()
            .map(|(title, body, code)| {
                let text = self
                    .thread
                    .texts
                    .plain(&format!("stepdetail-{key}-{k}-{title}"), &body, cx);
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(px(bench_ui::THREAD_DETAIL))
                            .text_color(theme.muted_foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .when(code, |d| {
                                d.p_2()
                                    .bg(theme.secondary)
                                    .border_1()
                                    .rounded_md()
                                    .border_color(theme.border)
                                    .font_family(bench_ui::theme::MONO)
                                    .text_size(theme.mono_font_size)
                            })
                            .when(!code, |d| d.text_size(px(bench_ui::THREAD_META)))
                            .text_color(theme.foreground)
                            .child(text),
                    )
                    .into_any_element()
            })
            .collect();
        // A document's head, as the Explorer's: the action, and quieter where and which step;
        // under its hairline the body scrolls on its own.
        let head = v_flex()
            .flex_none()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                bench_ui::title(cx)
                    .text_size(px(bench_ui::THREAD_META))
                    .truncate()
                    .when(failed, |d| d.text_color(theme.danger))
                    .child(clip(&said.action, 200)),
            )
            .child(
                div()
                    .text_size(px(bench_ui::THREAD_DETAIL))
                    .text_color(bench_ui::faint(&theme))
                    .child(sub),
            );
        let body = v_flex()
            .px_3()
            .py_2()
            .gap_3()
            .children(bodies)
            .when(cut || whole, |d| {
                let toggle = (key.to_string(), k);
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
            });
        v_flex()
            .size_full()
            .child(head)
            .child(crate::inserts::scroll_pane(
                SharedString::from(format!("stepdetail-{key}")),
                scroll,
                body,
            ))
            .into_any_element()
    }
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
