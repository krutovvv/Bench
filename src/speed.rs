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
