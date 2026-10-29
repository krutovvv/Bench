//! Dictation and reading an answer aloud. Dictation goes whole when it stops; with "Text while
//! speaking" on, the whole speech so far is also recognized at pauses while the person goes on.

use std::time::Duration;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rest;
use crate::media::audio::{Playback, Recorder};
use crate::media::pauses::{Cut, commit_pause, ends_sentence, predict, worth_a_run};
use crate::media::state::{KEPT_RUNS, LiveDictation};

/// The longest dictation, in seconds: then it stops as if the mic were clicked.
const MAX_RECORDING: f32 = 300.;

/// Ticks of the waveform's repaint (33 ms) between two looks for a pause.
const PAUSE_LOOK: u32 = 6;


use crate::app::BenchApp;

impl BenchApp {
    /// The microphone on, or off and what it heard into the composer. With "Text while speaking"
    /// on (the gear's Voice), the whole speech so far is recognized again at each pause — one run at
    /// a time, when the new speech is worth the run (`pauses.rs`) — and its text replaces the last;
    /// stopping is the last pause: the whole speech once more, only when some came after the last run.
    pub fn toggle_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rec) = self.media.recorder.take() {
            if self.media.live.is_some() {
                self.stop_live(rec, window, cx);
                cx.notify();
                return;
            }
            // Only the speech goes, without the silence around it; nothing said, nothing sent (a
            // mic left on in a quiet room sent minutes of silence and held the recognizer up).
            let speech = match rec.speech_span(0) {
                Some((a, b)) => rec.wav_between(a, b),
                None => Err(anyhow::anyhow!("nothing was said")),
            };
            drop(rec);
            match speech {
                Ok(wav) => {
                    self.media.transcribing = true;
                    self.rest(
                        window,
                        cx,
                        move |c| rest::transcribe(c, &wav, ""),
                        |this, r, window, cx| {
                            this.media.transcribing = false;
                            let send = std::mem::take(&mut this.media.send_after_transcribe);
                            match r {
                                Ok(text) if !text.is_empty() => {
                                    this.put_dictated(text, window, cx);
                                    if send {
                                        this.send_dictated(window, cx);
                                    }
                                }
                                Ok(_) => this.toast(
                                    NotificationType::Warning,
                                    "Nothing recognized",
                                    window,
                                    cx,
                                ),
                                Err(e) => this.transcription_failed(e, window, cx),
                            }
                        },
                    );
                }
                Err(e) => {
                    self.media.send_after_transcribe = false;
                    self.media.live = None;
                    self.toast(
                        NotificationType::Warning,
                        format!("Recording: {e:#}"),
                        window,
                        cx,
                    );
                }
            }
        } else {
            match Recorder::start() {
                Ok(r) => {
                    self.media.recorder = Some(r);
                    self.media.live = self.media.live_pref.then(LiveDictation::new);
                    let mut tick = 0u32;
                    self.media.wave_ticker = Some(cx.spawn_in(window, async move |this, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(33))
                                .await;
                            tick += 1;
                            let alive = this.update_in(cx, |this, window, cx| {
                                cx.notify();
                                // A mic left on stops by itself; what was said goes as when stopped.
                                if this.media.recorder.as_ref().is_some_and(|r| r.secs() >= MAX_RECORDING) {
                                    this.toast(NotificationType::Info, "Dictation stopped at 5 minutes", window, cx);
                                    this.toggle_mic(window, cx);
                                    return false;
                                }
                                // A look for a pause every ~0.2 s.
                                if tick % PAUSE_LOOK == 0 && this.media.live.is_some() {
                                    this.at_pause(window, cx);
                                }
                                this.media.recorder.is_some()
                            });
                            if !matches!(alive, Ok(true)) {
                                break;
                            }
                        }
                    }));
                }
                Err(e) => self.toast(
                    NotificationType::Error,
                    format!("Microphone: {e:#}"),
                    window,
                    cx,
                ),
            }
        }
        cx.notify();
    }

    /// While recording: once runs grow slow, a sentence the last run ended at a long pause is
    /// closed; then, at a pause after speech, the open speech is recognized again — when no run is
    /// out and the new speech is worth the run's predicted time.
    fn at_pause(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(rec), Some(live)) = (self.media.recorder.as_ref(), self.media.live.as_mut()) else {
            return;
        };
        // One run at a time: a pause meanwhile waits, and the next run takes the speech as it is then.
        if live.running {
            return;
        }
        let rate = rec.rate() as f32;
        // The open speech's text ends a sentence, the run that read it is the last, and the pause
        // after it is long enough for how slow the open speech has grown: closed.
        if live.open_end == live.ran && live.ran > live.from && ends_sentence(&live.open) {
            let open_secs = (live.ran - live.from) as f32 / rate;
            if let Some(need) = commit_pause(predict(&self.media.runs, open_secs)) {
                if rec.silence_from(live.ran) >= need {
                    let open = std::mem::take(&mut live.open);
                    live.closed = join_text(&live.closed, &open);
                    live.from = live.ran;
                }
            }
        }
        let (looked, ran, from) = (live.looked, live.ran, live.from);
        match rec.cut_after(looked) {
            Some(Cut::Speech(at)) => {
                let len = rec.len();
                let open_secs = (len - from) as f32 / rate;
                let worth = worth_a_run((len - ran) as f32 / rate, predict(&self.media.runs, open_secs));
                // From the speech's first words: the silence before them is not sent.
                let start = rec.speech_span(from).map_or(from, |(a, _)| a);
                let wav = worth.then(|| rec.wav_between(start, len));
                live.looked = at;
                if worth {
                    live.ran = len;
                }
                if let Some(Ok(wav)) = wav {
                    self.run_live(wav, len, false, window, cx);
                }
            }
            Some(Cut::Silence(at)) => live.looked = at,
            None => {}
        }
    }

    /// The recording stopped: one more run of the open speech when words came after the last run
    /// (now, or once the run out is back); else the dictation ends with what is shown.
    fn stop_live(&mut self, rec: Recorder, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.media.live.as_mut() else {
            return;
        };
        // Words after the last run, not just time: silence since is not worth a run; and only the
        // speech goes, without the silence around it.
        let more = rec.spoke_since(live.ran);
        let len = rec.len();
        live.finishing = true;
        let wav = if more { rec.speech_span(live.from).and_then(|(a, b)| rec.wav_between(a, b).ok()) } else { None };
        drop(rec);
        match (wav, live.running) {
            (Some(wav), false) => self.run_live(wav, len, true, window, cx),
            (Some(wav), true) => live.last = Some((wav, len)),
            (None, true) => {}
            (None, false) => self.end_live(window, cx),
        }
    }

    /// The dictation is over: its text stays, and is sent when Send was pressed meanwhile.
    fn end_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.media.live = None;
        self.media.transcribing = false;
        if std::mem::take(&mut self.media.send_after_transcribe) {
            self.send_dictated(window, cx);
        }
        cx.notify();
    }

    /// One run of the open speech (ending at sample `end`; `last`: at the recording's end). Its text replaces the open text shown unless a later
    /// run's is already there; after the recording stopped, the run out is followed by the open
    /// speech's when words came after it, else the dictation ends.
    fn run_live(&mut self, wav: Vec<u8>, end: usize, last: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.media.live.as_mut() else {
            return;
        };
        let seq = live.next;
        live.next += 1;
        live.running = true;
        // No context from the closed text: tried on dev, the 0.6B wrote the speech after it in
        // lower case without commas as often as it helped (`rest::transcribe` keeps the field).
        let prompt = String::new();
        // Seconds of speech in it: a 16-bit mono WAV, its header 44 bytes.
        let secs = wav_secs(&wav);
        let started = std::time::Instant::now();
        self.media.transcribing = true;
        self.rest(
            window,
            cx,
            move |c| rest::transcribe(c, &wav, &prompt),
            move |this, r, window, cx| {
                let took = started.elapsed().as_secs_f32();
                let Some(live) = this.media.live.as_mut() else {
                    return;
                };
                live.running = false;
                if secs > 1. {
                    this.media.runs.push((secs, took));
                    let n = this.media.runs.len();
                    if n > KEPT_RUNS {
                        this.media.runs.drain(..n - KEPT_RUNS);
                    }
                }
                let Some(live) = this.media.live.as_mut() else {
                    return;
                };
                let text = match r {
                    Ok(t) => Some(t),
                    Err(e) => {
                        if !live.failed {
                            live.failed = true;
                            this.transcription_failed(e, window, cx);
                        }
                        None
                    }
                };
                let live = this.media.live.as_mut().expect("checked above");
                let newest = seq + 1 >= live.shown;
                if let Some(text) = text.filter(|_| newest) {
                    live.shown = seq + 1;
                    live.open = text;
                    live.open_end = end;
                    let composer = this.active_thread.read(cx).composer.clone();
                    let live = this.media.live.as_mut().expect("checked above");
                    let base = live.base.get_or_insert_with(|| composer.read(cx).value().to_string()).clone();
                    let joined = join_text(&join_text(&base, &live.closed), &live.open);
                    composer.update(cx, |s, cx| s.set_value(joined, window, cx));
                }
                let live = this.media.live.as_mut().expect("checked above");
                if last || (live.finishing && live.last.is_none()) {
                    this.end_live(window, cx);
                } else if let Some((wav, end)) = live.last.take() {
                    this.run_live(wav, end, true, window, cx);
                } else {
                    this.media.transcribing = false;
                }
                cx.notify();
            },
        );
    }

    /// Dictated words at the end of the open thread's composer, after a space.
    fn put_dictated(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let composer = self.active_thread.read(cx).composer.clone();
        let current = composer.read(cx).value().to_string();
        let joined = if current.trim().is_empty() {
            text
        } else {
            format!("{} {text}", current.trim_end())
        };
        composer.update(cx, |s, cx| s.set_value(joined, window, cx));
    }

    /// Send pressed while dictating: sent once the words are in (the view reads the app as it sends).
    fn send_dictated(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.active_thread.clone();
        window.defer(cx, move |window, cx| view.update(cx, |v, cx| v.send(window, cx)));
    }

    fn transcription_failed(&mut self, e: anyhow::Error, window: &mut Window, cx: &mut Context<Self>) {
        let e = format!("{e:#}");
        let msg = if crate::client::launch::ask_local_network(&e) {
            "Transcription: allow Bench on the local network (macOS asks now), then dictate again".to_string()
        } else {
            format!("Transcription: {e}")
        };
        self.toast(NotificationType::Error, msg, window, cx)
    }

    /// Read the answer `key` (its `text`) aloud, or stop reading it.
    pub fn toggle_speak(
        &mut self,
        key: &str,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.media.playing.as_ref().is_some_and(|(j, _)| j == key) {
            self.media.playing = None;
            cx.notify();
            return;
        }
        self.media.playing = None;
        self.media.audio = None;
        if text.is_empty() {
            return;
        }
        let key = key.to_string();
        self.media.speaking = Some(key.clone());
        self.rest(
            window,
            cx,
            move |c| rest::speak(c, &text),
            move |this, r, window, cx| {
                this.media.speaking = None;
                match r {
                    Ok(bytes) => this.media.playing = Some((key, Playback::start(bytes))),
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("Text-to-speech: {e:#}"),
                        window,
                        cx,
                    ),
                }
            },
        );
        cx.notify();
    }
}

/// Seconds of speech in a 16-bit mono WAV (its header is 44 bytes, its rate at bytes 24..28).
fn wav_secs(wav: &[u8]) -> f32 {
    let rate = wav.get(24..28).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(48_000).max(1);
    (wav.len().saturating_sub(44) / 2) as f32 / rate as f32
}

/// Two texts with a space between, either may be empty.
fn join_text(a: &str, b: &str) -> String {
    match (a.trim().is_empty(), b.trim().is_empty()) {
        (true, _) => b.trim().to_string(),
        (_, true) => a.trim_end().to_string(),
        _ => format!("{} {}", a.trim_end(), b.trim()),
    }
}
