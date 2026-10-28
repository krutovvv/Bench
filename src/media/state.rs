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
        }
    }
}
