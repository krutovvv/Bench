//! What the window keeps of the answers' media: the files fetched and the diagrams drawn, what is
//! opened in full, what plays (an answer read aloud, a delivered audio file, a video), and the
//! microphone while the person dictates.

use std::collections::{HashMap, HashSet};

use gpui_kit::*;
use web_time::Instant;

use super::audio::{Playback, Recorder};
use super::video::Player;
use super::{Diagram, Fetched, Loaded};

pub struct MediaState {
    /// Files answers delivered, by target and path; diagrams, by source and dark theme.
    pub files: HashMap<String, Loaded<Fetched>>,
    pub diagrams: HashMap<(String, bool), Loaded<Diagram>>,
    /// Markdown files and long code blocks opened in full rather than their first lines.
    pub open: HashSet<String>,
    /// A media key whose fresh copy to open in its app once it arrives (a click on a remote
    /// file re-fetches first — its name alone doesn't say the file is the same file).
    pub open_after_fetch: Option<String>,
    /// The answer read aloud (its row key), and the one whose audio is being made.
    pub playing: Option<(String, Playback)>,
    pub speaking: Option<String>,
    /// The delivered audio file playing, by its media key, and since when.
    pub audio: Option<(String, Playback)>,
    pub audio_since: Instant,
    /// The video playing in the thread, the one starting, and the loop that shows its frames.
    pub video: Option<Player>,
    pub video_loading: Option<String>,
    pub video_task: Option<Task<()>>,
    /// Dictation: the microphone, the waveform's repaint, the speech being recognized, and
    /// Send pressed meanwhile (sent once the words are in).
    pub recorder: Option<Recorder>,
    pub wave_ticker: Option<Task<()>>,
    pub transcribing: bool,
    pub send_after_transcribe: bool,
    /// The gear's "Text while speaking": each pause sends what was said (`voice.rs`). Bench's own
    /// choice, kept in `dictation.json` in its state folder.
    pub live_pref: bool,
    /// A dictation sent by pauses, while it lasts.
    pub live: Option<LiveDictation>,
}

/// Dictation sent piece by piece: where the next piece starts, and each piece's text in order (None:
/// not back yet), shown in the composer as the pieces before it are in.
#[derive(Default)]
pub struct LiveDictation {
    pub sent: usize,
    pub next: u64,
    pub texts: std::collections::BTreeMap<u64, Option<String>>,
    pub shown: u64,
    pub failed: bool,
}

/// Bench's dictation preference (`dictation.json`).
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct DictationPrefs {
    #[serde(default)]
    pub live: bool,
}

impl DictationPrefs {
    pub fn load() -> Self {
        std::fs::read(crate::client::connection::state_dir().join("dictation.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = crate::client::connection::state_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(dir.join("dictation.json"), b);
        }
    }
}

impl Default for MediaState {
    fn default() -> Self {
        Self {
            files: HashMap::new(),
            diagrams: HashMap::new(),
            open: HashSet::new(),
            open_after_fetch: None,
            playing: None,
            speaking: None,
            audio: None,
            audio_since: Instant::now(),
            video: None,
            video_loading: None,
            video_task: None,
            recorder: None,
            wave_ticker: None,
            transcribing: false,
            send_after_transcribe: false,
            live_pref: DictationPrefs::load().live,
            live: None,
        }
    }
}
