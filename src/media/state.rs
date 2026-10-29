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

/// Dictation recognized while it goes: at each pause, the whole speech so far, one run at a time; its
/// text replaces the last after what the composer held before. Stopping is the last pause.
pub struct LiveDictation {
    /// The composer's text before the dictation's first words: they go after it. Read when the
    /// first run is back, not when the mic is clicked — the thread's view is busy with the click.
    pub base: Option<String>,
    /// Where to look for the next pause from, in samples.
    pub looked: usize,
    /// The speech the last run had, in samples.
    pub ran: usize,
    /// A run is out; the last result shown, and the number the next run gets.
    pub running: bool,
    pub shown: u64,
    pub next: u64,
    /// Seconds of work per second of speech, as the last run measured it.
    pub speed: f32,
    /// The recording stopped: the dictation ends with the run out, or `last` after it.
    pub finishing: bool,
    /// The whole recording, for a run once the one out is back (speech came after it).
    pub last: Option<Vec<u8>>,
    pub failed: bool,
}

impl LiveDictation {
    pub fn new() -> Self {
        Self { base: None, looked: 0, ran: 0, running: false, shown: 0, next: 0, speed: 0.3, finishing: false, last: None, failed: false }
    }
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
