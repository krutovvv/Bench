//! A thread's meters, in the title bar by its repo: how full its context is (what it used and
//! did, in a panel), how fast the model writes in it (the chart of its last answer), and the
//! fallback model answering. They are the thread's — its view's journal and speed — so another
//! thread opened shows its own, never a global bar's.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::ThreadView;
use ::thread::{Row, Step};
use bench_ui::compact;
use bench_ui::icons::Ico;

impl ThreadView {
    /// The meters to show: context, speed, fallback — each only once it has something to say.
    /// `context_length` is the agent's window, for a thread that has not been told its own.
    pub(crate) fn render_meters(&self, context_length: u64, cx: &App) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        // Context: the agent's numbers once a turn has run; before that (a reopened chat) a rough
        // estimate from the transcript, marked "~". The limit falls back to model.context_length.
        let usage = self.thread.journal.usage().cloned();
        let real = usage.as_ref().filter(|u| u.context_used > 0);
        let max = real
            .map(|u| u.context_max)
            .filter(|m| *m > 0)
            .unwrap_or(context_length);
        let (used, estimated) = match real {
            Some(u) => (u.context_used, false),
            None => (self.estimate_tokens(), true),
        };
        let percent = if max > 0 {
            used as f64 * 100. / max as f64
        } else {
            0.
        };
        let stats = self.context_stats(used, max, estimated);
        let context = (max > 0
            && (self.thread.id.is_some() || !self.thread.journal.is_empty()))
        .then(|| {
            let color = if percent > 85. {
                theme.danger
            } else if percent > 60. {
                theme.warning
            } else {
                muted
            };
            let label = format!(
                "{}{} / {} · {:.0}%",
                if estimated { "~" } else { "" },
                compact(used),
                compact(max),
                percent
            );
            gpui_kit::component::popover::Popover::new("context-pop")
                .anchor(gpui_kit::Anchor::TopRight)
                .trigger(
                    Button::new("context")
                        .ghost()
                        .small()
                        .icon(Ico::MemoryStick.icon())
                        .child(div().text_size(px(bench_ui::BAR_TEXT)).child(label))
                        .text_color(color),
                )
                .content(move |_, _, cx| {
                    let theme = cx.theme();
                    v_flex()
                        .w(px(bench_ui::POPOVER_S))
                        .py_1()
                        .children(stats.iter().map(|(k, v)| {
                            h_flex()
                                .px_3()
                                .py_0p5()
                                .gap_4()
                                .text_sm()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(theme.muted_foreground)
                                        .child(k.clone()),
                                )
                                .child(div().child(v.clone()))
                        }))
                })
        });

        // Speed: how fast the model writes, in the bar's own muted colour.
        let rate = |v: f64| {
            if v >= 1000. {
                format!("{:.1}k", v / 1000.)
            } else {
                format!("{v:.0}")
            }
        };
        // Speed: the model's real pace, not a running average. While text streams: the newest
        // train's measured rate — its own span from first token to last, which is what makes a
        // single reading honest (the burst guards live in the measure); a three-second mean
        // lags the tempo and reads as someone else's number. When the stream pauses (a tool,
        // the next call's prefill) the last train's reading of THIS answer stays on the chip —
        // it vanishing between bursts read as the meter breaking; else the last call's measured
        // generation, else the agent's blended average. Prefill is not shown here: it is in the
        // panel this opens. Click: the chart of this answer.
        let meter = &self.thread.speed;
        let live = meter.live;
        let recent = meter.samples.last().copied();
        let label = match (live.or(recent), meter.generation) {
            (Some(l), _) => Some(format!("{} t/s", rate(l))),
            (None, Some(g)) => Some(format!("{} t/s", rate(g))),
            _ => self
                .thread
                .journal
                .usage()
                .and_then(|u| u.avg_tps)
                .map(|t| format!("{} t/s", rate(t))),
        };
        let meter = &self.thread.speed;
        let samples = meter.samples.clone();
        let (prefill, generation) = (meter.prefill, meter.generation);
        let speed = label.map(|label| {
            gpui_kit::component::popover::Popover::new("speed-pop")
                .anchor(gpui_kit::Anchor::TopRight)
                .trigger(
                    Button::new("speed")
                        .ghost()
                        .small()
                        .icon(Ico::Gauge.icon())
                        .child(div().text_size(px(bench_ui::BAR_TEXT)).child(label))
                        .text_color(muted),
                )
                .content(move |_, _, cx| speed_panel(&samples, live, prefill, generation, cx))
        });

        // The fallback answering, in red: the main model failed, and why, until it answers again.
        let fallback = self.thread.journal.info().fallback.clone().map(|protocol::Fallback { model, reason: why }| {
                let tip = if why.is_empty() {
                    "The main model is unavailable: the fallback answers until it is back.".to_string()
                } else {
                    format!("The main model is unavailable ({why}): the fallback answers until it is back.")
                };
            h_flex()
                .id("fallback")
                .px_2()
                .gap_1()
                .items_center()
                .text_color(theme.danger)
                .text_size(px(bench_ui::BAR_TEXT))
                .child(Ico::CircleAlert.icon().small())
                .child(format!("Main model down · on fallback {model}"))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        });
        [context.map(IntoElement::into_any_element), speed.map(IntoElement::into_any_element), fallback.map(IntoElement::into_any_element)]
            .into_iter()
            .flatten()
            .collect()
    }

    /// Rough tokens of the open chat: its text at ~3.5 characters a token.
    fn estimate_tokens(&self) -> u64 {
        let chars: usize = self
            .thread
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
    fn context_stats(
        &self,
        used: u64,
        max: u64,
        estimated: bool,
    ) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> = vec![];
        let tilde = if estimated { "~" } else { "" };
        // Which model this thread answers with; the project sets it in its environment.
        if let Some(m) = self
            .thread
            .journal
            .usage()
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
            .thread
            .tv
            .rows
            .iter()
            .filter(|r| matches!(r, Row::User { .. }))
            .count();
        rows.push(("Messages".into(), messages.to_string()));
        let tools = self
            .thread
            .tv
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Work { steps, .. } => Some(steps.iter().filter(|s| s.is_tool()).count()),
                _ => None,
            })
            .sum::<usize>();
        rows.push(("Tool calls".into(), tools.to_string()));
        let files = self.changed_files();
        if !files.is_empty() {
            rows.push(("Files changed".into(), files.len().to_string()));
        }
        if let Some(u) = self.thread.journal.usage() {
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
        let speed = &self.thread.speed;
        if let Some(p) = speed.prefill {
            rows.push(("Prefill".into(), format!("{p:.0} tok/s")));
        }
        if let Some(g) = speed.generation {
            rows.push(("Generation".into(), format!("{g:.0} tok/s")));
        }
        if estimated {
            rows.push(("".into(), "~ estimated until the next reply".into()));
        }
        rows
    }
}

/// The speed panel: per-second generation speed of the current/last answer as bars, its
/// average, the writing time, and the last call's prefill and generation. Two things live
/// here — how fast the model writes, and how fast it reads — nothing else.
fn speed_panel(
    samples: &[f64],
    live: Option<f64>,
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
    let mut panel = v_flex().w(px(bench_ui::POPOVER_M)).py_2();
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
        let avg = samples.iter().sum::<f64>() / samples.len() as f64;
        // The last 68 seconds at most (as many 3px bars as fit); each bar is one second.
        let shown = &samples[samples.len().saturating_sub(68)..];
        // One 3px bar per second from the left: a short answer is a few bars, not one slab.
        let bars = shown.iter().map(|v| {
            div()
                .w(px(bench_ui::SPEED_BAR_W))
                .flex_none()
                .h(px((v / max * f64::from(bench_ui::SPEED_CHART_H)).max(1.) as f32))
                .bg(theme.primary.opacity(0.75))
        });
        panel = panel.child(
            h_flex()
                .mx_3()
                .mb_2()
                .h(px(bench_ui::SPEED_CHART_H))
                .gap(px(bench_ui::SPEED_BAR_GAP))
                .items_end()
                .border_b_1()
                .border_color(theme.border)
                .children(bars),
        );
        // Now: the newest train's honest reading — its own span, first token to last. The
        // burst guards are in the measure, so this is the tempo itself, not a mean over it.
        if let Some(now) = live {
            panel = panel.child(row("Generation · now", format!("{now:.0} tok/s")));
        }
        // One sample is a second the model was writing; waits for tools are left out of the
        // chart, so the average and the time speak of writing, not of the turn's wall time.
        panel = panel
            .child(row("Generation · average", format!("{avg:.0} tok/s")))
            .child(row("Time writing", super::speed::fmt_secs(samples.len())));
    }
    if let Some(p) = prefill {
        panel = panel.child(row("Prefill · last call", format!("{p:.0} tok/s")));
    }
    if let Some(g) = generation {
        panel = panel.child(row("Generation · last call", format!("{g:.0} tok/s")));
    }
    panel
}
