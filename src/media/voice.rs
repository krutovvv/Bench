//! Dictation and reading an answer aloud. Dictation goes whole when it stops; with "Text while
//! speaking" on, the whole speech so far is also recognized at pauses while the person goes on.

use std::time::Duration;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rest;
use crate::media::audio::{Playback, Recorder};
use crate::media::pauses::{Cut, worth_a_run};
use crate::media::state::LiveDictation;

/// Ticks of the waveform's repaint (33 ms) between two looks for a pause.
const PAUSE_LOOK: u32 = 6;
/// Speech after the last run, in seconds, that stopping still sends.
const LAST_WORDS: f32 = 0.3;

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
            match rec.finish() {
                Ok(wav) => {
                    self.media.transcribing = true;
                    self.rest(
                        window,
                        cx,
                        move |c| rest::transcribe(c, &wav),
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

    /// While recording: at a pause after speech, the whole speech so far is recognized again — when
    /// no run is out and the new speech is worth the run's cost.
    fn at_pause(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(rec), Some(live)) = (self.media.recorder.as_ref(), self.media.live.as_ref()) else {
            return;
        };
        // One run at a time: a pause meanwhile waits, and the next run takes the speech as it is then.
        if live.running {
            return;
        }
        let (looked, ran, speed) = (live.looked, live.ran, live.speed);
        match rec.cut_after(looked) {
            Some(Cut::Speech(at)) => {
                let (len, rate) = (rec.len(), rec.rate() as f32);
                let worth = worth_a_run((len - ran) as f32 / rate, len as f32 / rate, speed);
                let wav = worth.then(|| rec.wav_between(0, len));
                if let Some(live) = self.media.live.as_mut() {
                    live.looked = at;
                    if worth {
                        live.ran = len;
                    }
                }
                if let Some(Ok(wav)) = wav {
                    self.run_live(wav, false, window, cx);
                }
            }
            Some(Cut::Silence(at)) => {
                if let Some(live) = self.media.live.as_mut() {
                    live.looked = at;
                }
            }
            None => {}
        }
    }

    /// The recording stopped: one more run of the whole speech when some came after the last run
    /// (now, or once the run out is back); else the dictation ends with what is shown.
    fn stop_live(&mut self, rec: Recorder, window: &mut Window, cx: &mut Context<Self>) {
        let (len, rate) = (rec.len(), rec.rate() as f32);
        let Some(live) = self.media.live.as_mut() else {
            return;
        };
        let more = (len.saturating_sub(live.ran)) as f32 / rate >= LAST_WORDS;
        live.finishing = true;
        let wav = if more { rec.finish().ok() } else { None };
        match (wav, live.running) {
            (Some(wav), false) => self.run_live(wav, true, window, cx),
            (Some(wav), true) => live.last = Some(wav),
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

    /// One run of the whole speech so far (`last`: at the recording's end). Its text replaces the
    /// last shown unless a later run's is already there; after the recording stopped, the run out
    /// is followed by the whole recording's when speech came after it, else the dictation ends.
    fn run_live(&mut self, wav: Vec<u8>, last: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.media.live.as_mut() else {
            return;
        };
        let seq = live.next;
        live.next += 1;
        live.running = true;
        // Seconds of speech in it: a 16-bit mono WAV, its header 44 bytes.
        let secs = wav_secs(&wav);
        let started = std::time::Instant::now();
        self.media.transcribing = true;
        self.rest(
            window,
            cx,
            move |c| rest::transcribe(c, &wav),
            move |this, r, window, cx| {
                let took = started.elapsed().as_secs_f32();
                let Some(live) = this.media.live.as_mut() else {
                    return;
                };
                live.running = false;
                if secs > 1. {
                    live.speed = took / secs;
                }
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
                    let composer = this.active_thread.read(cx).composer.clone();
                    let live = this.media.live.as_mut().expect("checked above");
                    let base = live.base.get_or_insert_with(|| composer.read(cx).value().to_string()).clone();
                    let joined = if base.trim().is_empty() { text } else if text.trim().is_empty() { base } else { format!("{} {text}", base.trim_end()) };
                    composer.update(cx, |s, cx| s.set_value(joined, window, cx));
                }
                let live = this.media.live.as_mut().expect("checked above");
                if last || (live.finishing && live.last.is_none()) {
                    this.end_live(window, cx);
                } else if let Some(wav) = live.last.take() {
                    this.run_live(wav, true, window, cx);
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
