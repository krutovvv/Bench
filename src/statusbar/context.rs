//! The context and speed panels: what the open thread has used and done, roughly counted when
//! the agent has not said, and how fast the last answer came.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::BenchApp;
use ::thread::{Row, Step};
use bench_ui::compact;

impl BenchApp {
    /// Rough tokens of the open chat: its text at ~3.5 characters a token.
    pub(super) fn estimate_tokens(&self, cx: &App) -> u64 {
        let chars: usize = self
            .thread(cx)
            .tv
            .rows
            .iter()
            .map(|r| match r {
                Row::User { text, .. } | Row::Answer { text, .. } => text.len(),
                Row::Work { steps, .. } => steps
                    .iter()
                    .map(|s| match s {
                        Step::Tool { args, output, .. } => {
                            args.to_string().len() + output.as_ref().map_or(0, String::len)
                        }
                        Step::Note(x) | Step::Thought(x) => x.len(),
                    })
                    .sum(),
                _ => 0,
            })
            .sum();
        (chars as f64 / 3.5) as u64
    }

    /// The rows of the context panel: what this chat has used and done.
    pub(super) fn session_stats(
        &self,
        used: u64,
        max: u64,
        estimated: bool,
        cx: &App,
    ) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> = vec![];
        let tilde = if estimated { "~" } else { "" };
        // Which model this thread answers with; the project sets it in its environment.
        if let Some(m) = self
            .thread(cx)
            .usage
            .as_ref()
            .map(|u| u.model.clone())
            .filter(|m| !m.is_empty())
        {
            rows.push(("Model".into(), m));
        }
        rows.push((
            "Context".into(),
            if max > 0 {
                format!("{tilde}{} of {}", compact(used), compact(max))
            } else {
                format!("{tilde}{}", compact(used))
            },
        ));
        if max > 0 {
            rows.push(("Left".into(), compact(max.saturating_sub(used))));
        }
        let messages = self
            .thread(cx)
            .tv
            .rows
            .iter()
            .filter(|r| matches!(r, Row::User { .. }))
            .count();
        rows.push(("Messages".into(), messages.to_string()));
        let tools = self
            .thread(cx)
            .tv
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Work { steps, .. } => Some(steps.iter().filter(|s| s.is_tool()).count()),
                _ => None,
            })
            .sum::<usize>();
        rows.push(("Tool calls".into(), tools.to_string()));
        let files = self.active_thread.read(cx).session_files();
        if !files.is_empty() {
            rows.push(("Files changed".into(), files.len().to_string()));
        }
        if let Some(u) = self.thread(cx).usage.as_ref() {
            if u.calls > 0 {
                rows.push(("Model calls".into(), u.calls.to_string()));
            }
            if u.input + u.output > 0 {
                rows.push((
                    "Tokens in / out".into(),
                    format!("{} / {}", compact(u.input), compact(u.output)),
                ));
            }
            if let Some(c) = u.cache_hit_pct {
                rows.push(("Cache hit".into(), format!("{c}%")));
            }
            rows.push(("Compactions".into(), u.compressions.to_string()));
        }
        if let Some(p) = self.speed.prefill {
            rows.push(("Prefill".into(), format!("{p:.0} tok/s")));
        }
        if let Some(g) = self.speed.generation {
            rows.push(("Generation".into(), format!("{g:.0} tok/s")));
        }
        if estimated {
            rows.push(("".into(), "~ estimated until the next reply".into()));
        }
        rows
    }
}

/// The speed panel: per-second generation speed of the current/last answer as bars, its range,
/// and the last call's measured prefill and generation.
pub(super) fn speed_panel(
    samples: &[f64],
    prefill: Option<f64>,
    generation: Option<f64>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let row = |k: &str, v: String| {
        h_flex()
            .px_3()
            .py_0p5()
            .gap_4()
            .text_sm()
            .child(
                div()
                    .flex_1()
                    .text_color(theme.muted_foreground)
                    .child(k.to_string()),
            )
            .child(div().child(v))
    };
    let mut panel = v_flex().w(px(300.)).py_2();
    if samples.is_empty() {
        panel = panel.child(
            div()
                .px_3()
                .pb_1()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("The chart fills in while the next answer is written."),
        );
    } else {
        let max = samples.iter().cloned().fold(0., f64::max).max(1.);
        let min = samples.iter().cloned().fold(f64::MAX, f64::min);
        let avg = samples.iter().sum::<f64>() / samples.len() as f64;
        // The last 68 seconds at most (as many 3px bars as fit); each bar is one second.
        let shown = &samples[samples.len().saturating_sub(68)..];
        // One 3px bar per second from the left: a short answer is a few bars, not one slab.
        let bars = shown.iter().map(|v| {
            div()
                .w(px(3.))
                .flex_none()
                .h(px((v / max * 56.).max(1.) as f32))
                .bg(theme.primary.opacity(0.75))
        });
        panel = panel
            .child(
                h_flex()
                    .mx_3()
                    .mb_2()
                    .h(px(56.))
                    .gap(px(1.))
                    .items_end()
                    .border_b_1()
                    .border_color(theme.border)
                    .children(bars),
            )
            .child(row(
                "Now / average",
                format!(
                    "{:.0} / {:.0} tok/s",
                    shown.last().copied().unwrap_or(0.),
                    avg
                ),
            ))
            .child(row("Min – max", format!("{min:.0} – {max:.0} tok/s")))
            // One sample is a second the model was writing; waits for tools and prefill are left
            // out of every figure here, so this row counts writing, not the turn's wall time.
            .child(row("Writing", format!("{} s", samples.len())));
    }
    if let Some(p) = prefill {
        panel = panel.child(row("Prefill (last call)", format!("{p:.0} tok/s")));
    }
    if let Some(g) = generation {
        panel = panel.child(row("Generation (last call)", format!("{g:.0} tok/s")));
    }
    panel
}
