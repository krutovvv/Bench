//! Dictation and reading an answer aloud.

use std::time::Duration;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rest;
use crate::media::audio::{Playback, Recorder};

use crate::app::BenchApp;

impl BenchApp {
    pub fn toggle_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rec) = self.media.recorder.take() {
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
                                // Into the open thread's composer; sent by its view once
                                // this is done (the view reads the app as it sends).
                                Ok(text) if !text.is_empty() => {
                                    let view = this.active_thread.clone();
                                    let composer = view.read(cx).composer.clone();
                                    let current = composer.read(cx).value().to_string();
                                    let joined = if current.trim().is_empty() {
                                        text
                                    } else {
                                        format!("{} {text}", current.trim_end())
                                    };
                                    composer.update(cx, |s, cx| s.set_value(joined, window, cx));
                                    if send {
                                        window.defer(cx, move |window, cx| {
                                            view.update(cx, |v, cx| v.send(window, cx))
                                        });
                                    }
                                }
                                Ok(_) => this.toast(
                                    NotificationType::Warning,
                                    "Nothing recognized",
                                    window,
                                    cx,
                                ),
                                Err(e) => {
                                    let e = format!("{e:#}");
                                    let msg = if crate::client::launch::ask_local_network(&e) {
                                        "Transcription: allow Bench on the local network (macOS asks now), then dictate again".to_string()
                                    } else {
                                        format!("Transcription: {e}")
                                    };
                                    this.toast(NotificationType::Error, msg, window, cx)
                                }
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
                    self.media.wave_ticker = Some(cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(33))
                                .await;
                            let alive = this.update(cx, |this, cx| {
                                cx.notify();
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
