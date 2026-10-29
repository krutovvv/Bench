//! Dictation and reading an answer aloud. Dictation goes whole when it stops, or, with "Text
//! while speaking" on, piece by piece at its pauses (`pauses.rs`) while the person goes on.

use std::time::Duration;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rest;
use crate::media::audio::{Playback, Recorder};
use crate::media::pauses::Cut;
use crate::media::state::LiveDictation;

/// Ticks of the waveform's repaint (33 ms) between two looks for a pause.
const PAUSE_LOOK: u32 = 6;

use crate::app::BenchApp;

impl BenchApp {
    /// The microphone on, or off and what it heard into the composer. With "Text while speaking"
    /// on (the gear's Voice), each pause sends what was said while the next is spoken, and the
    /// words come in as they are recognized; off, the whole recording goes when it stops.
    pub fn toggle_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rec) = self.media.recorder.take() {
            if let Some(live) = self.media.live.as_ref() {
                let from = live.sent;
                match rec.finish_from(from) {
                    Ok(Some(wav)) => self.send_piece(wav, window, cx),
                    Ok(None) => {}
                    Err(e) => self.toast(NotificationType::Warning, format!("Recording: {e:#}"), window, cx),
                }
                self.show_pieces(window, cx);
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
                    self.media.live = self.media.live_pref.then(LiveDictation::default);
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
                                    this.cut_piece(window, cx);
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

    /// While recording: at a pause after speech, the piece since the last goes to be recognized.
    fn cut_piece(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(rec), Some(live)) = (self.media.recorder.as_ref(), self.media.live.as_ref()) else {
            return;
        };
        let from = live.sent;
        match rec.cut_after(from) {
            Some(Cut::Speech(at)) => {
                let wav = rec.wav_between(from, at);
                if let Some(live) = self.media.live.as_mut() {
                    live.sent = at;
                }
                if let Ok(wav) = wav {
                    self.send_piece(wav, window, cx);
                }
            }
            Some(Cut::Silence(at)) => {
                if let Some(live) = self.media.live.as_mut() {
                    live.sent = at;
                }
            }
            None => {}
        }
    }

    /// One piece to be recognized; its text joins the composer in the order the pieces were said.
    fn send_piece(&mut self, wav: Vec<u8>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = self.media.live.as_mut() else {
            return;
        };
        let seq = live.next;
        live.next += 1;
        live.texts.insert(seq, None);
        self.media.transcribing = true;
        self.rest(
            window,
            cx,
            move |c| rest::transcribe(c, &wav),
            move |this, r, window, cx| {
                let text = match r {
                    Ok(t) => t,
                    Err(e) => {
                        // One toast for a dictation, however many pieces fail.
                        if this.media.live.as_ref().is_some_and(|l| !l.failed) {
                            if let Some(l) = this.media.live.as_mut() {
                                l.failed = true;
                            }
                            this.transcription_failed(e, window, cx);
                        }
                        String::new()
                    }
                };
                if let Some(live) = this.media.live.as_mut() {
                    live.texts.insert(seq, Some(text));
                }
                this.show_pieces(window, cx);
            },
        );
    }

    /// The pieces back so far, in order, into the composer; once the recording stopped and every
    /// piece is in, the dictation ends (and is sent when Send was pressed meanwhile).
    fn show_pieces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut ready = vec![];
        if let Some(live) = self.media.live.as_mut() {
            while let Some(Some(text)) = live.texts.get(&live.shown) {
                ready.push(text.clone());
                live.shown += 1;
            }
        }
        for text in ready.into_iter().filter(|t| !t.trim().is_empty()) {
            self.put_dictated(text, window, cx);
        }
        let done = self.media.recorder.is_none() && self.media.live.as_ref().is_some_and(|l| l.shown == l.next);
        self.media.transcribing = !done && self.media.live.as_ref().is_some_and(|l| l.shown < l.next);
        if done {
            self.media.live = None;
            self.media.transcribing = false;
            if std::mem::take(&mut self.media.send_after_transcribe) {
                self.send_dictated(window, cx);
            }
        }
        cx.notify();
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
