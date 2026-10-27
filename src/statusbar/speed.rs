//! Prefill and generation speed, measured from the event stream.
//!
//! The agent reports token counts per API call (each `session.usage` frame follows one call) but
//! only a blended `avg_tps`. Timing each call ourselves splits it:
//! prefill = new input tokens / time to the first streamed token;
//! generation = output tokens / the span from that call's first token to its last (a tool
//! boundary closes the span, so tool time is not the model's). Deliberately the whole span, not
//! the sum of short gaps: bursty chunk delivery would turn gap-summing into rates the hardware
//! cannot do. A call that cannot be timed cleanly (nothing streamed, too few tokens) leaves the
//! last value.
//!
//! Live speed: while text streams, once a second the characters of that second are turned into
//! tokens with a characters-per-token ratio calibrated on each finished call (the agent reports
//! the exact token count only per call), and divided by the writing span of that token train —
//! first token to last — rather than by the whole second: the second a tool ran in says nothing
//! about the model, and neither says the part before the train began. A train whose tokens came
//! too fast to measure (one burst delivered together) waits for more tokens instead of reading
//! as hundreds a second. The per-second samples of the current answer feed a chart.

use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use crate::app::model::Usage;

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
    /// Characters of the open token train (since its first token, or since the last sample).
    window_chars: usize,
    /// The train's first token, and the longest wait between its neighbours.
    window_first: Option<Instant>,
    window_gap: Duration,
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

/// The shortest wait between a train's tokens that makes the train a pause — the model waiting
/// on something else (a tool, arguments, the next call) rather than writing. Its sample is
/// skipped; the train after it measures on its own.
const PAUSE: Duration = Duration::from_millis(1000);

/// A train this short spans one burst of chunks delivered together: its span is delivery time,
/// not writing time, so it proves nothing. The characters wait for more tokens instead.
const MIN_SPAN: Duration = Duration::from_millis(150);

/// Below this many tokens a train is too short to read as a rate; they wait for more.
const MIN_TOKENS: f64 = 8.;

/// Tokens per second over a token train: its characters with the learned ratio, over the span
/// from its first token to its last. None when the train cannot be read as writing: a wait
/// inside it (a tool), a burst too short to time, or too few tokens to mean anything.
pub fn train_rate(chars: usize, span: Duration, max_gap: Duration, ratio: f64) -> Option<f64> {
    let tokens = chars as f64 / ratio;
    if max_gap > PAUSE || span < MIN_SPAN || tokens < MIN_TOKENS || ratio <= 0. {
        None
    } else {
        Some(tokens / span.as_secs_f64())
    }
}

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
        self.window_first = None;
        self.window_gap = Duration::ZERO;
        self.last_tick = None;
        self.last_token = None;
    }

    /// Streamed text (answer or reasoning) of `chars` characters.
    pub fn token(&mut self, chars: usize) {
        let now = Instant::now();
        if self.window_first.is_some() {
            // The wait inside the open train: the longest one makes it a pause.
            if let Some(t) = self.last_token {
                self.window_gap = self.window_gap.max(now.duration_since(t));
            }
        } else {
            self.window_first = Some(now);
            self.window_chars = 0;
            self.window_gap = Duration::ZERO;
        }
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
        // The rate of the train, when it can be read: its own span, not the second around it.
        // A train that is still too short, or held a pause, keeps its characters for the next
        // reading instead of lying or charting a dip.
        if streaming {
            if let Some(first) = self.window_first {
                // The train's own span, last token to first — not up to this tick, which can
                // sit up to half a second after the writing stopped.
                let span = self
                    .last_token
                    .unwrap_or(now)
                    .saturating_duration_since(first);
                if let Some(rate) = train_rate(self.window_chars, span, self.window_gap, self.ratio)
                {
                    self.live = Some(rate);
                    self.samples.push(rate);
                    self.window_chars = 0;
                    self.window_first = None;
                    self.window_gap = Duration::ZERO;
                }
            }
        } else {
            self.live = None;
        }
        self.last_tick = streaming.then_some(now);
    }

    /// The answer is complete: no live reading any more (the samples stay for the chart).
    pub fn finish(&mut self) {
        self.live = None;
        self.last_tick = None;
        self.last_token = None;
        self.window_chars = 0;
        self.window_first = None;
        self.window_gap = Duration::ZERO;
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
                // The call's whole writing span, first token to last — waits inside the call
                // included, since the person waited too. Summing only the short gaps instead
                // flatters bursty delivery into rates the hardware cannot do. A tool boundary
                // ends `end`, so tool time stays out of the span.
                let decode = end.duration_since(first).as_secs_f64();
                if output >= 16 && decode > 0.25 {
                    self.generation = Some(output as f64 / decode);
                }
                // Learn characters per token from this call. Half-weight each reading: prose
                // that is mostly Russian runs near 2 characters a token, and a slow learner
                // reads such writing almost twice as fast as it is until it converges.
                if output >= 16 && self.chars_call > 0 {
                    let r = (self.chars_call as f64 / output as f64).clamp(0.8, 8.);
                    self.ratio = 0.5 * self.ratio + 0.5 * r;
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

/// Writing time in human terms: seconds while they are seconds, then minutes, then hours.
pub fn fmt_secs(n: usize) -> String {
    match (n / 3600, n / 60 % 60, n % 60) {
        (0, 0, s) => format!("{s} s"),
        (0, m, s) => format!("{m} min {s:02} s"),
        (h, m, _) => format!("{h} h {m:02} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_time_reads_like_a_person_says_it() {
        assert_eq!(fmt_secs(47), "47 s");
        assert_eq!(fmt_secs(219), "3 min 39 s");
        assert_eq!(fmt_secs(60), "1 min 00 s");
        assert_eq!(fmt_secs(4325), "1 h 12 min");
    }

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

    #[test]
    fn a_burst_train_reads_as_throughput_not_as_the_speed_of_light() {
        // A second that holds one honest train: 300 tokens over its 0.9 s span, even though
        // the chunks inside bursts arrived milliseconds apart.
        let r = train_rate(
            960,
            Duration::from_millis(900),
            Duration::from_millis(40),
            3.2,
        )
        .unwrap();
        assert!((r - 333.3).abs() < 1., "960 chars / 3.2 / 0.9 s: {r}");
        // One burst of chunks delivered together proves nothing: it waits for more tokens.
        assert_eq!(
            train_rate(
                400,
                Duration::from_millis(60),
                Duration::from_millis(3),
                3.2
            ),
            None
        );
        // Too few tokens to mean anything, same.
        assert_eq!(
            train_rate(
                20,
                Duration::from_millis(400),
                Duration::from_millis(50),
                3.2
            ),
            None
        );
        // A wait inside the train is the model waiting on something else: no reading.
        assert_eq!(
            train_rate(
                900,
                Duration::from_secs(3),
                Duration::from_millis(1200),
                3.2
            ),
            None
        );
    }
}
