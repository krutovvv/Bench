//! Prefill and generation speed, measured from the event stream.
//!
//! Hermes reports token counts per API call (each `session.usage` frame follows one call) but
//! only a blended `avg_tps`. Timing each call ourselves splits it:
//! prefill = new input tokens / time to the first streamed token;
//! generation = output tokens / time from the first token to the end of the call.
//! A call that cannot be timed cleanly (nothing streamed, too few tokens) leaves the last value.

use std::time::Instant;

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
}

impl Meter {
    /// A fresh session: counters start at zero.
    pub fn fresh() -> Self {
        Self {
            base: Some((0, 0, 0)),
            ..Default::default()
        }
    }

    /// A new chat: counters from zero, the last measured speeds kept.
    pub fn restart(&mut self) {
        *self = Self {
            prefill: self.prefill,
            generation: self.generation,
            ..Self::fresh()
        };
    }

    pub fn request(&mut self) {
        self.request = Some(Instant::now());
        self.first = None;
        self.end = None;
        self.closed = false;
    }

    /// A streamed token (answer or reasoning).
    pub fn token(&mut self) {
        if self.closed {
            return;
        }
        let now = Instant::now();
        self.first.get_or_insert(now);
        self.end = Some(now);
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
