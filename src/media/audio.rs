//! Microphone capture (-> WAV for STT) and playback of TTS audio.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct Recorder {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

fn push_mono(buf: &Mutex<Vec<f32>>, data: impl Iterator<Item = f32>, channels: usize, len: usize) {
    let mut out = buf.lock().unwrap();
    let frame: Vec<f32> = data.collect();
    out.reserve(len / channels.max(1));
    for chunk in frame.chunks(channels.max(1)) {
        out.push(chunk.iter().sum::<f32>() / chunk.len() as f32);
    }
}

impl Recorder {
    pub fn start() -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| anyhow!("no microphone"))?;
        let config = device.default_input_config()?;
        let rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let samples = Arc::new(Mutex::new(Vec::new()));
        let sink = samples.clone();
        let on_err = |e| eprintln!("[audio] {e}");
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |d: &[f32], _| push_mono(&sink, d.iter().copied(), channels, d.len()),
                on_err,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |d: &[i16], _| {
                    push_mono(
                        &sink,
                        d.iter().map(|s| *s as f32 / i16::MAX as f32),
                        channels,
                        d.len(),
                    )
                },
                on_err,
                None,
            )?,
            other => bail!("microphone format {other:?} is not supported"),
        };
        stream.play()?;
        Ok(Self {
            stream,
            samples,
            rate,
        })
    }

    /// The loudness of the last `n` 25 ms slices, oldest first, each 0..1 (perceptual curve).
    pub fn levels(&self, n: usize) -> Vec<f32> {
        let win = (self.rate as usize / 40).max(1);
        let samples = self.samples.lock().unwrap();
        let usable = samples.len() / win * win;
        let start = usable.saturating_sub(n * win);
        let mut out: Vec<f32> = samples[start..usable]
            .chunks(win)
            .map(|c| {
                let rms = (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt();
                (rms * 6.).sqrt().min(1.)
            })
            .collect();
        while out.len() < n {
            out.insert(0, 0.);
        }
        out
    }

    /// The loudness now, 0..1: the mean of the last four 25 ms slices, so the mic's little bar
    /// moves without flickering on every consonant.
    pub fn level(&self) -> f32 {
        let v = self.levels(4);
        v.iter().sum::<f32>() / v.len() as f32
    }

    /// Stop and return a 16-bit mono WAV.
    pub fn finish(self) -> Result<Vec<u8>> {
        drop(self.stream);
        let samples = std::mem::take(&mut *self.samples.lock().unwrap());
        if samples.len() < self.rate as usize / 4 {
            bail!("recording too short");
        }
        wav(&samples, self.rate)
    }

    /// How many samples are recorded, and at what rate.
    pub fn len(&self) -> usize {
        self.samples.lock().unwrap().len()
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Where to cut the speech recorded since sample `from` (`pauses.rs`); None: not yet.
    pub fn cut_after(&self, from: usize) -> Option<super::pauses::Cut> {
        super::pauses::cut(&self.samples.lock().unwrap(), self.rate, from)
    }

    /// How long the silence at sample `from` lasts so far, in seconds (`pauses.rs`).
    pub fn silence_from(&self, from: usize) -> f32 {
        super::pauses::silence_from(&self.samples.lock().unwrap(), self.rate, from)
    }

    /// Whether there was speech since sample `from` (`pauses.rs`).
    pub fn spoke_since(&self, from: usize) -> bool {
        super::pauses::spoke_since(&self.samples.lock().unwrap(), self.rate, from)
    }

    /// The recording's samples `from..to` as a WAV.
    pub fn wav_between(&self, from: usize, to: usize) -> Result<Vec<u8>> {
        let samples = self.samples.lock().unwrap();
        wav(&samples[from.min(samples.len())..to.min(samples.len())], self.rate)
    }

}

/// Samples as a 16-bit mono WAV.
fn wav(samples: &[f32], rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec)?;
        for s in samples {
            w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        w.finalize()?;
    }
    Ok(cursor.into_inner())
}

/// Whether playback can decode these bytes (Opus, for one, it cannot).
pub fn decodable(bytes: &[u8]) -> bool {
    rodio::Decoder::new(Cursor::new(bytes.to_vec())).is_ok()
}

/// Plays one clip on a background thread; `stop()` or drop ends it.
pub struct Playback {
    stop: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
}

impl Playback {
    pub fn start(bytes: Vec<u8>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let (s, d) = (stop.clone(), done.clone());
        std::thread::spawn(move || {
            let run = || -> Result<()> {
                let (_stream, handle) = rodio::OutputStream::try_default()?;
                let sink = rodio::Sink::try_new(&handle)?;
                sink.append(rodio::Decoder::new(Cursor::new(bytes))?);
                while !sink.empty() && !s.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(50));
                }
                sink.stop();
                Ok(())
            };
            if let Err(e) = run() {
                eprintln!("[audio] playback: {e}");
            }
            d.store(true, Ordering::Relaxed);
        });
        Self { stop, done }
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop();
    }
}
