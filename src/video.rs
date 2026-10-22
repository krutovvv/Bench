//! Videos play inside the chat: ffmpeg decodes frames (shown as images, 25 a second) and the
//! sound track (played the way read-aloud audio is). No ffmpeg: the system player opens them.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::Instant;

use anyhow::{Context as _, Result, anyhow};
use gpui_kit::RenderImage;

use crate::audio::{self, Playback};

const FPS: f64 = 25.;

pub fn ffmpeg() -> Option<PathBuf> {
    // An app started from Finder has no Homebrew on its PATH.
    let path = std::env::var("PATH").unwrap_or_default();
    ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"]
        .into_iter()
        .map(PathBuf::from)
        .chain(std::env::split_paths(&path))
        .map(|d| d.join("ffmpeg"))
        .find(|p| p.is_file())
}

pub struct Player {
    /// The media key of the file playing.
    pub key: String,
    pub frame: Option<Arc<RenderImage>>,
    size: (u32, u32),
    started: Instant,
    shown: Option<usize>,
    next: Option<(usize, Vec<u8>)>,
    frames: Receiver<Vec<u8>>,
    decoded: usize,
    ended: bool,
    decoder: Child,
    _sound: Option<Playback>,
}

impl Player {
    /// Decode `file` at `w`×`h` pixels (even numbers) and start playing. Blocking (it reads the
    /// sound track first): call off the UI thread.
    pub fn start(key: String, file: &Path, w: u32, h: u32) -> Result<Self> {
        let ffmpeg = ffmpeg().ok_or_else(|| anyhow!("no ffmpeg"))?;
        let (w, h) = (w.max(2) & !1, h.max(2) & !1);
        let sound = Command::new(&ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(file)
            .args(["-vn", "-ac", "2", "-ar", "44100", "-f", "wav", "-"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .context("ffmpeg")?
            .stdout;
        let mut decoder = Command::new(&ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(file)
            .args([
                "-an",
                "-vf",
                &format!("fps={FPS},scale={w}:{h}"),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "bgra",
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("ffmpeg")?;
        let mut out = decoder.stdout.take().unwrap();
        // A few frames ahead, no more: the decoder waits for the screen.
        let (tx, frames) = sync_channel::<Vec<u8>>(6);
        let len = (w * h * 4) as usize;
        std::thread::spawn(move || {
            loop {
                let mut buf = vec![0u8; len];
                if out.read_exact(&mut buf).is_err() || tx.send(buf).is_err() {
                    break;
                }
            }
        });
        let sound = (sound.len() > 4096 && audio::decodable(&sound)).then(|| Playback::start(sound));
        Ok(Self {
            key,
            frame: None,
            size: (w, h),
            started: Instant::now(),
            shown: None,
            next: None,
            frames,
            decoded: 0,
            ended: false,
            decoder,
            _sound: sound,
        })
    }

    /// Show the frame due now. Returns whether the picture changed, and the image it replaced
    /// (the caller frees it from the GPU with `window.drop_image`).
    pub fn tick(&mut self) -> (bool, Option<Arc<RenderImage>>) {
        let due = (self.started.elapsed().as_secs_f64() * FPS) as usize;
        let mut latest: Option<Vec<u8>> = None;
        loop {
            if self.next.is_none() {
                match self.frames.try_recv() {
                    Ok(buf) => {
                        self.next = Some((self.decoded, buf));
                        self.decoded += 1;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.ended = true;
                        break;
                    }
                }
            }
            match self.next.take() {
                Some((i, buf)) if i <= due => {
                    self.shown = Some(i);
                    latest = Some(buf); // late frames are skipped
                }
                other => {
                    self.next = other;
                    break;
                }
            }
        }
        let Some(buf) = latest else {
            return (false, None);
        };
        let (w, h) = self.size;
        let Some(img) = image::ImageBuffer::from_raw(w, h, buf) else {
            return (false, None);
        };
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(img)]));
        (true, self.frame.replace(image))
    }

    pub fn finished(&self) -> bool {
        self.ended && self.next.is_none()
    }

    /// Seconds played.
    pub fn elapsed(&self) -> u64 {
        self.shown.map_or(0, |i| (i as f64 / FPS) as u64)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.decoder.kill();
        let _ = self.decoder.wait();
    }
}
