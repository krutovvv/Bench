//! The window's threads beyond the ones on screen: every agent's list (the sidebar's rows), the
//! journals of threads seen in this run, the drafts and the code references waiting in their
//! composers, the thread to reopen, and a new chat being made.

use std::collections::HashMap;

use crate::thread::composer::attachments::Reference;

pub struct ThreadStore {
    /// Every agent's threads, as each last listed them.
    pub list: Vec<ThreadRow>,
    /// Each thread opened in this run, as last shown: switching back shows it at once.
    pub cache: HashMap<String, ::thread::Thread>,
    /// Unsent text per thread, kept on disk (`drafts.rs`).
    pub drafts: HashMap<String, String>,
    /// The project's code sent to a thread from the Explorer, waiting with its message.
    pub references: HashMap<String, Vec<Reference>>,
    /// Open this thread (its agent, its id) once its agent lists it.
    pub reopen: Option<(String, protocol::ThreadId)>,
    /// A new chat's thread is being made (an insert or a message came before it existed).
    pub creating: bool,
}

impl ThreadStore {
    pub fn new(reopen: Option<(String, protocol::ThreadId)>) -> Self {
        Self {
            list: vec![],
            cache: HashMap::new(),
            drafts: super::composer::drafts::load_drafts(),
            references: HashMap::new(),
            reopen,
            creating: false,
        }
    }
}

/// A thread as the sidebar lists it: what its agent says of it (`protocol::ThreadSummary`), and
/// which agent that is.
#[derive(Clone, Debug, Default)]
pub struct ThreadRow {
    pub id: protocol::ThreadId,
    pub title: String,
    pub running: bool,
    /// The person paused its run: it stands still until resumed.
    pub paused: bool,
    /// Its run waits for the person's answer (a question, an approval).
    pub waiting: bool,
    /// Where it was started: "local" (Bench), "telegram", "cron", …
    pub source: String,
    /// The project it belongs to, as Bench keys projects (`ProjectKey`); its agent's Host when
    /// it has none.
    pub project: String,
    /// The server whose agent keeps it (an ~/.ssh/config alias); "" = this Mac's.
    pub host: String,
    /// The folder it works in, and the top of its git repo.
    pub cwd: String,
    pub repo: Option<String>,
    /// Who it is with (a Telegram thread's person).
    pub with: Option<String>,
    /// The thread it was forked from: the sidebar shows it under that one.
    pub parent: Option<protocol::ThreadId>,
}

impl ThreadRow {
    /// The agent `host`'s summary of a thread, placed in the project `project`.
    pub fn new(host: &str, project: String, t: protocol::ThreadSummary) -> Self {
        Self {
            id: t.id,
            title: t.title,
            running: t.running,
            paused: t.paused,
            waiting: t.waiting,
            source: t.source,
            project,
            host: host.to_string(),
            cwd: t.cwd,
            repo: t.repo,
            with: t.with,
            parent: t.parent,
        }
    }
}

/// Human name of a session source, for grouping the session list.
pub fn source_label(source: &str) -> String {
    match source {
        "" | "local" | "desktop" | "tui" | "cli" | "dashboard" => "Bench".into(),
        "telegram" => "Telegram".into(),
        "cron" => "Cron".into(),
        "api_server" | "api" => "API".into(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
    }
}
