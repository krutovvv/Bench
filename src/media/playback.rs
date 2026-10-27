//! Sound and video playing in place in an answer: a delivered audio file, a video's frames and
//! sound (ffmpeg).

use std::path::PathBuf;
use gpui_kit::*;
use crate::app::BenchApp;
use crate::media::audio::{self, Playback};

impl BenchApp {
    pub(super) fn toggle_media_audio(&mut self, key: String, local: PathBuf, cx: &mut Context<Self>) {
        if self.media_playing.as_ref().is_some_and(|(k, _)| *k == key) {
            self.media_playing = None;
            cx.notify();
            return;
        }
        self.media_playing = None;
        self.playing = None;
        cx.spawn(async move |this, cx| {
            let file = local.clone();
            let bytes = cx
                .background_spawn(async move {
                    std::fs::read(&file).ok().filter(|b| audio::decodable(b))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match bytes {
                    Some(bytes) => {
                        this.stop_video(None, cx);
                        this.media_since = web_time::Instant::now();
                        this.media_playing = Some((key, Playback::start(bytes)));
                    }
                    // A format playback cannot decode: the system player can.
                    None => cx.open_with_system(&local),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Play a video in place: frames at its picture's size (twice, for Retina), with sound.
    pub(super) fn start_video(
        &mut self,
        key: String,
        file: PathBuf,
        (w, h): (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_video(Some(window), cx);
        self.media_playing = None;
        self.playing = None;
        self.video_loading = Some(key.clone());
        let scale = window.scale_factor().clamp(1., 2.);
        let (pw, ph) = ((w * scale) as u32, (h * scale) as u32);
        self.video_task = Some(cx.spawn(async move |this, cx| {
            let (k, fallback) = (key.clone(), file.clone());
            let player = cx
                .background_spawn(async move { crate::media::video::Player::start(k, &file, pw, ph) })
                .await;
            let ok = this.update(cx, |this, cx| {
                this.video_loading = None;
                match player {
                    Ok(p) => this.video = Some(p),
                    // Could not decode it here: the system player can.
                    Err(_) => cx.open_with_system(&fallback),
                }
                cx.notify();
            });
            if ok.is_err() {
                return;
            }
            // Frames: each tick shows the one due and frees the one it replaced.
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(15))
                    .await;
                let go = this.update(cx, |this, cx| {
                    let Some(p) = this.video.as_mut() else {
                        return false;
                    };
                    let (changed, old) = p.tick();
                    if let Some(old) = old {
                        cx.drop_image(old, None);
                    }
                    if p.finished() {
                        this.stop_video(None, cx);
                        return false;
                    }
                    if changed {
                        cx.notify();
                    }
                    true
                });
                if !matches!(go, Ok(true)) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// `window`: the one being updated, if any (its frame is freed there too).
    pub(crate) fn stop_video(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if let Some(mut p) = self.video.take() {
            if let Some(frame) = p.frame.take() {
                cx.drop_image(frame, window);
            }
        }
        self.video_loading = None;
        cx.notify();
    }
}
