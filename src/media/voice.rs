//! Dictation and reading an answer aloud.

use std::time::Duration;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rest;
use crate::media::audio::{Playback, Recorder};

use crate::app::BenchApp;

impl BenchApp {
    pub fn toggle_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rec) = self.recorder.take() {
            match rec.finish() {
                Ok(wav) => {
                    self.transcribing = true;
                    self.rest(
                        window,
                        cx,
                        move |c, p| rest::transcribe(c, p, &wav),
                        |this, r, window, cx| {
                            this.transcribing = false;
                            let send = std::mem::take(&mut this.send_after_transcribe);
                            match r {
                                Ok(text) if !text.is_empty() => {
                                    let current = this.composer.read(cx).value().to_string();
                                    let joined = if current.trim().is_empty() {
                                        text
                                    } else {
                                        format!("{} {text}", current.trim_end())
                                    };
                                    this.composer
                                        .update(cx, |s, cx| s.set_value(joined, window, cx));
                                    if send {
                                        this.send(window, cx);
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
                    self.send_after_transcribe = false;
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
                    self.recorder = Some(r);
                    self.wave_ticker = Some(cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(33))
                                .await;
                            let alive = this.update(cx, |this, cx| {
                                cx.notify();
                                this.recorder.is_some()
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

    pub fn toggle_speak(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.playing.as_ref().is_some_and(|(j, _)| j == key) {
            self.playing = None;
            cx.notify();
            return;
        }
        self.playing = None;
        self.media_playing = None;
        let Some(text) = self.answer_text(key) else {
            return;
        };
        let key = key.to_string();
        self.speaking = Some(key.clone());
        self.rest(
            window,
            cx,
            move |c, p| rest::speak(c, p, &text),
            move |this, r, window, cx| {
                this.speaking = None;
                match r {
                    Ok(bytes) => this.playing = Some((key, Playback::start(bytes))),
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
