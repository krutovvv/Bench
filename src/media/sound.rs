//! Bench's few quiet sounds: the mic going on and off, a message sent. A run's end has none: macOS
//! says it with the notification. Named as Zed's `Sound`; the files are written by
//! `scripts/make-sounds.py`. Bench's own choice whether they play, kept in `sounds.json` in its
//! state folder. A bare build stays silent, as with notifications: copies run for pictures and
//! tests beside the person's Bench.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sound {
    MicOn,
    MicOff,
    MessageSent,
}

impl Sound {
    fn bytes(self) -> &'static [u8] {
        match self {
            Sound::MicOn => include_bytes!("../../assets/sounds/mic_on.wav"),
            Sound::MicOff => include_bytes!("../../assets/sounds/mic_off.wav"),
            Sound::MessageSent => include_bytes!("../../assets/sounds/message_sent.wav"),
        }
    }

    pub fn play(self) {
        if enabled() && crate::system::bundled() {
            super::audio::play_out(self.bytes().to_vec());
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SoundPrefs {
    on: bool,
}

const FILE: &str = "sounds.json";

static ON: LazyLock<AtomicBool> = LazyLock::new(|| {
    let prefs = std::fs::read(crate::client::connection::state_dir().join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<SoundPrefs>(&b).ok());
    AtomicBool::new(prefs.is_none_or(|p| p.on))
});

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
    let dir = crate::client::connection::state_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = serde_json::to_vec_pretty(&SoundPrefs { on }) {
        let _ = std::fs::write(dir.join(FILE), b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_short_quiet_and_decodable() {
        for sound in [Sound::MicOn, Sound::MicOff, Sound::MessageSent] {
            let bytes = sound.bytes();
            assert!(super::super::audio::decodable(bytes), "{sound:?}");
            let mut wav = hound::WavReader::new(bytes).unwrap();
            let rate = wav.spec().sample_rate as f32;
            let samples: Vec<i16> = wav.samples::<i16>().map(Result::unwrap).collect();
            assert!(samples.len() as f32 / rate < 0.25, "{sound:?} is long");
            let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!(f32::from(peak) / 32768. < 0.12, "{sound:?} is loud");
        }
    }
}
