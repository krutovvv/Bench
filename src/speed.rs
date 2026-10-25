//! Prefill and generation speed, measured from the event stream.
//!
//! The agent reports token counts per API call (each `session.usage` frame follows one call) but
//! only a blended `avg_tps`. Timing each call ourselves splits it:
//! prefill = new input tokens / time to the first streamed token;
//! generation = output tokens / time from the first token to the end of the call.
//! A call that cannot be timed cleanly (nothing streamed, too few tokens) leaves the last value.
//!
//! Live speed: while text streams, once a second the characters of that second are turned into
//! tokens with a characters-per-token ratio calibrated on each finished call (the agent reports
//! the exact token count only per call). The per-second samples of the current answer feed a chart.

use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use crate::model::Usage;

#[derive(Default)]
pub struct Meter {
    /// When the current API call was requested (message sent, tools finished, or last usage).
    request: Option<Instant>,
    first: Option<Instant>,
    end: Option<Instant>,
    /// A tool call ended the stream; later deltas belong to the next call.
    closed: bool,
    /// (input, output, calls) as of the last usage frame.
    base: Option<(u64, u64, u64)>,
    pub prefill: Option<f64>,
    pub generation: Option<f64>,
    /// Characters streamed in the current call (for calibrating `ratio`).
    chars_call: usize,
    /// Characters per token, learned from finished calls.
    ratio: f64,
    /// Characters since the last one-second sample, and when that sample was taken.
    window_chars: usize,
    last_tick: Option<Instant>,
    last_token: Option<Instant>,
    /// Tokens/second over the last second while text is streaming; None between bursts.
    pub live: Option<f64>,
    /// One-second samples of the current (or last) answer.
    pub samples: Vec<f64>,
}

/// How many of the per-second samples a thread keeps (the chart shows the last minute or so).
pub const KEEP: usize = 180;

/// What a thread keeps of the measurements: the chart of its last answer and the last speeds.
/// Bench measures these, the agent keeps them with the thread (its store is what survives a
/// restart), so reopening the thread or the app brings the numbers back.
#[derive(Default, Clone, Debug)]
pub struct Stat {
    pub samples: Vec<f64>,
    pub prefill: Option<f64>,
    pub generation: Option<f64>,
    pub ratio: Option<f64>,
}

impl Stat {
    /// What the agent kept (`speed` of `session.resume`). Readings that are not readings — zero,
    /// negative, a word — are dropped rather than trusted.
    pub fn from_json(v: &Value) -> Self {
        let num = |k: &str| v[k].as_f64().filter(|f| *f > 0.);
        Self {
            samples: v["samples"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().filter(|f| f.is_finite() && *f > 0.))
                        .collect()
                })
                .unwrap_or_default(),
            prefill: num("prefill"),
            generation: num("generation"),
            ratio: num("ratio"),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "samples": self.samples,
            "prefill": self.prefill,
            "generation": self.generation,
            "ratio": self.ratio,
        })
    }

    /// A thread that never ran has nothing to show.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty() && self.prefill.is_none() && self.generation.is_none()
    }
}

/// Mixed Russian/English prose and code averages about this many characters per token.
const DEFAULT_RATIO: f64 = 3.2;

impl Meter {
    /// A fresh session: counters start at zero.
    pub fn fresh() -> Self {
        Self {
            base: Some((0, 0, 0)),
            ratio: DEFAULT_RATIO,
            ..Default::default()
        }
    }

    /// A new chat: counters from zero, the last measured speeds kept.
    pub fn restart(&mut self) {
        *self = Self {
            prefill: self.prefill,
            generation: self.generation,
            ratio: self.ratio,
            ..Self::fresh()
        };
    }

    pub fn request(&mut self) {
        self.request = Some(Instant::now());
        self.first = None;
        self.end = None;
        self.closed = false;
        self.chars_call = 0;
    }

    /// The user sent a new message: the chart starts over.
    pub fn begin_answer(&mut self) {
        self.samples.clear();
        self.live = None;
        self.window_chars = 0;
        self.last_tick = None;
    }

    /// Streamed text (answer or reasoning) of `chars` characters.
    pub fn token(&mut self, chars: usize) {
        let now = Instant::now();
        self.window_chars += chars;
        self.last_token = Some(now);
        self.last_tick.get_or_insert(now);
        if self.closed {
            return;
        }
        self.chars_call += chars;
        self.first.get_or_insert(now);
        self.end = Some(now);
    }

    /// Called about twice a second: once a full second has passed, close a sample. A pause in
    /// the stream (tools, prefill) ends the live reading instead of charting zeros.
    pub fn tick(&mut self) {
        let now = Instant::now();
        let Some(start) = self.last_tick else { return };
        let dt = now.duration_since(start);
        if dt < Duration::from_secs(1) {
            return;
        }
        let streaming = self
            .last_token
            .is_some_and(|t| now.duration_since(t) < Duration::from_millis(1500));
        if streaming && self.window_chars > 0 {
            let rate = self.window_chars as f64 / self.ratio / dt.as_secs_f64();
            self.live = Some(rate);
            self.samples.push(rate);
        } else {
            self.live = None;
        }
        self.window_chars = 0;
        self.last_tick = streaming.then_some(now);
    }

    /// The answer is complete: no live reading any more (the samples stay for the chart).
    pub fn finish(&mut self) {
        self.live = None;
        self.last_tick = None;
        self.window_chars = 0;
    }

    /// The thread's numbers as they stand: its chart's tail and the last measured speeds.
    pub fn stat(&self) -> Stat {
        Stat {
            samples: self
                .samples
                .iter()
                .copied()
                .rev()
                .take(KEEP)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            prefill: self.prefill,
            generation: self.generation,
            ratio: (self.ratio > 0.).then_some(self.ratio),
        }
    }

    /// What the thread kept, back in the meter: its chart always (it is the thread's own answer),
    /// the speeds only if the thread measured them (a chat that has not run yet keeps the speeds
    /// of the model, which describe the model and its server rather than the chat).
    pub fn restore(&mut self, s: Stat) {
        self.samples = s.samples;
        if s.prefill.is_some() {
            self.prefill = s.prefill;
        }
        if s.generation.is_some() {
            self.generation = s.generation;
        }
        if let Some(r) = s.ratio {
            self.ratio = r.clamp(DEFAULT_RATIO / 2., DEFAULT_RATIO * 2.);
        }
    }

    /// The model finished generating a tool call (its arguments are not streamed).
    pub fn tool_call(&mut self) {
        if self.first.is_some() && !self.closed {
            self.end = Some(Instant::now());
            self.closed = true;
        }
    }

    pub fn usage(&mut self, u: &Usage) {
        let now = (u.input, u.output, u.calls);
        if let (Some(base), Some(first), Some(end)) = (self.base, self.first, self.end) {
            if now.2 == base.2 + 1 {
                let (input, output) = (now.0.saturating_sub(base.0), now.1.saturating_sub(base.1));
                let decode = end.duration_since(first).as_secs_f64();
                if output >= 16 && decode > 0.25 {
                    self.generation = Some(output as f64 / decode);
                }
                // Learn characters per token from this call (smoothed, sane bounds).
                if output >= 32 && self.chars_call > 0 {
                    let r = (self.chars_call as f64 / output as f64).clamp(1., 8.);
                    self.ratio = 0.7 * self.ratio + 0.3 * r;
                }
                if let Some(req) = self.request {
                    let ttft = first.saturating_duration_since(req).as_secs_f64();
                    if input >= 256 && ttft > 0.05 {
                        self.prefill = Some(input as f64 / ttft);
                    }
                }
            }
        }
        if now.2 > 0 || self.base.is_none() {
            self.base = Some(now);
        }
        if self.first.is_some() {
            self.request();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_threads_numbers_survive_the_wire() {
        let m = || Stat {
            samples: vec![10., 20., 30.],
            prefill: Some(1200.5),
            generation: Some(990.25),
            ratio: Some(3.1),
        };
        assert_eq!(Stat::from_json(&m().to_json()).samples, vec![10., 20., 30.]);
        assert_eq!(Stat::from_json(&m().to_json()).generation, Some(990.25));
    }

    #[test]
    fn a_reading_that_is_not_a_reading_is_not_kept() {
        let s = Stat::from_json(
            &json!({"samples": ["x", -5, 0, 40, null], "prefill": "fast", "generation": 0}),
        );
        assert_eq!(s.samples, vec![40.]);
        assert_eq!(s.prefill, None);
        assert_eq!(s.generation, None);
        assert_eq!(Stat::from_json(&json!({})).samples, Vec::<f64>::new());
        assert!(Stat::default().is_empty());
    }

    #[test]
    fn the_chart_keeps_its_tail_and_a_chat_without_one_keeps_the_speeds() {
        let mut meter = Meter::fresh();
        meter.samples = (0..400).map(|k| k as f64).collect();
        meter.generation = Some(900.);
        let st = meter.stat();
        assert_eq!(st.samples.len(), KEEP);
        assert_eq!(st.samples.last(), Some(&399.));
        // A thread that never ran: no chart, but the model's speed stays.
        meter.restore(Stat::default());
        assert!(meter.samples.is_empty());
        assert_eq!(meter.generation, Some(900.));
        // A thread that ran: its own chart and its own numbers.
        meter.restore(Stat {
            samples: vec![5., 6.],
            prefill: None,
            generation: Some(120.),
            ratio: Some(4.),
        });
        assert_eq!(meter.samples, vec![5., 6.]);
        assert_eq!(meter.generation, Some(120.));
        assert_eq!(meter.prefill, None);
    }
}
