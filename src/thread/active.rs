//! A thread's state: which thread of which agent it is, its journal and the list showing it, its
//! run and how fast it writes, its project. Its `ThreadView` (`view.rs`) holds it with the
//! composer; the app routes each of the thread's events to the views showing it (`app/events.rs`).

use std::collections::HashSet;

use protocol::{StreamKind, ThreadEvent, ThreadId, ThreadItem};

use crate::app::Live;
use crate::thread::speed::Meter;

pub struct ActiveThread {
    /// The agent that keeps it ("" = this Mac's).
    pub host: String,
    /// Its id there; `None` for a new chat not made yet.
    pub id: Option<ThreadId>,
    /// The open thread: its journal, what is being written, its queue (`crates/thread`).
    pub journal: ::thread::Thread,
    /// The open thread on screen: its list and rows (`ui::thread`).
    pub tv: crate::thread::View,
    /// Opened pieces of work, by the work row's key; unfolded steps (work key, step); the source
    /// host whose pages are listed (work key, host).
    pub open_work: HashSet<String>,
    /// The live work this view opened on its own, so the agent's words show where they will
    /// settle. The person touching it (either way) takes it over; at the run's end it folds.
    pub(crate) auto_work: Option<String>,
    pub open_steps: HashSet<(String, usize)>,
    pub open_source: Option<(String, String)>,
    /// The open thread's texts, parsed once (`ui::media::Texts`).
    pub texts: crate::media::text::Texts,
    /// The open thread's history could not be read: why (shown with Retry instead of "Loading").
    pub history_error: Option<String>,
    /// Its history came from the agent (or it was made here): "Loading…" until then.
    pub loaded: bool,
    pub live: Option<Live>,
    /// The first message of a thread not made yet: its request id and words, sent once it is.
    pub queued: Option<(String, String)>,
    /// The "Changed files" list under the last turn is unfolded (folded by default).
    pub files_open: bool,
    /// The changed files' list shows them all, past its first ten.
    pub files_all: bool,
    /// The works whose background processes' card is unfolded (`thread/processes.rs`), and
    /// the process whose output is shown.
    pub processes_open: HashSet<String>,
    pub process_open: Option<String>,
    /// How fast the model writes in it, measured from its stream.
    pub speed: Meter,
    pub effort: usize,
    /// A level was chosen before the session existed: it goes with the first message.
    pub effort_pending: bool,
    /// The open session's effort when it is none of EFFORTS (set elsewhere, e.g. `high`; "" is the
    /// provider's default): shown as it is, nothing checked.
    pub effort_other: Option<String>,
    /// The choices ticked so far in the question the agent asks (multi-select).
    pub clarify_picked: Vec<usize>,
    /// Which plans of the open thread are unfolded, by their journal id (the cards themselves
    /// are rows' data: `crates/thread`).
    pub plan_open: HashSet<String>,
    /// The git repo it works in (None for a thread that touches no repo).
    pub repo: Option<crate::app::repo::Repo>,
    /// The project it belongs to (a new one is created in it); "" = its agent's Host.
    pub project: String,
}

impl Default for ActiveThread {
    fn default() -> Self {
        Self {
            host: String::new(),
            id: None,
            journal: Default::default(),
            tv: Default::default(),
            open_work: Default::default(),
            auto_work: None,
            open_steps: Default::default(),
            open_source: None,
            texts: Default::default(),
            history_error: None,
            loaded: false,
            live: None,
            queued: None,
            files_open: false,
            files_all: false,
            processes_open: HashSet::new(),
            process_open: None,
            speed: Meter::fresh(),
            effort: crate::app::model::EFFORTS.len() - 1,
            effort_pending: false,
            effort_other: None,
            clarify_picked: Vec::new(),
            plan_open: Default::default(),
            repo: None,
            project: String::new(),
        }
    }
}

/// One thread as the app acts on it for its view: which thread of which agent it is, where it
/// lives, whether it runs. The view makes it (`ActiveThread::thread_ref`) and hands it to what it
/// asks of the app — an insert's block, an answer's media, sending — so the app never has to read
/// a view, which may be drawing at that moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadRef {
    /// The agent that keeps it ("" = this Mac's).
    pub host: String,
    /// Its id there; `None` for a new chat not made yet.
    pub id: Option<ThreadId>,
    /// The project it belongs to; "" = none (its agent's Host).
    pub project: String,
    /// A run is live in it.
    pub live: bool,
}

impl ThreadRef {
    /// Its key for what is kept per thread (inserts, references): its id, or "draft" for a new
    /// chat.
    pub fn key(&self) -> String {
        self.id
            .as_ref()
            .map_or_else(|| "draft".into(), |id| id.to_string())
    }

    /// Its project as the Explorer and the environment name it: its own, or the Host.
    pub fn project_or_host(&self) -> String {
        if self.project.is_empty() {
            crate::app::model::ProjectKey::HOST.to_string()
        } else {
            self.project.clone()
        }
    }
}

impl ActiveThread {
    pub fn thread_ref(&self) -> ThreadRef {
        ThreadRef {
            host: self.host.clone(),
            id: self.id.clone(),
            project: self.project.clone(),
            live: self.live.is_some(),
        }
    }

    /// The level chosen before the session existed, once: it goes with the first message.
    pub fn take_effort(&mut self, efforts: &[String]) -> Option<String> {
        std::mem::take(&mut self.effort_pending)
            .then(|| efforts.get(self.effort).cloned())
            .flatten()
    }

    /// A change of this thread: its journal takes it; the status line says what the run does;
    /// the speed meter counts what it writes.
    pub fn on_thread_event(&mut self, e: &ThreadEvent) {
        self.journal.apply(e.clone());
        let label = |live: &mut Option<Live>, text: &str| {
            if let Some(live) = live.as_mut() {
                live.label = text.to_string().into();
            }
        };
        match e {
            // A run is a new answer: the chart starts over, the first call is under way.
            ThreadEvent::Run(true) => {
                if self.live.is_none() {
                    self.speed.begin_answer();
                    self.speed.request();
                }
                self.live.get_or_insert(Live {
                    label: "Thinking…".into(),
                });
            }
            ThreadEvent::Run(false) => {
                self.live = None;
                self.speed.finish();
            }
            ThreadEvent::Stream(kind, delta) => {
                label(
                    &mut self.live,
                    if *kind == StreamKind::Thought {
                        "Thinking…"
                    } else {
                        "Writing…"
                    },
                );
                self.speed.token(delta.chars().count());
            }
            ThreadEvent::Append(items) => {
                for it in items {
                    match it {
                        ThreadItem::Tool { name, preview, .. } => {
                            label(
                                &mut self.live,
                                &crate::thread::rows::tool_labels::tool_label(name, preview),
                            );
                            self.speed.tool_call();
                        }
                        ThreadItem::Result { .. } => self.speed.request(),
                        _ => {}
                    }
                }
            }
            ThreadEvent::Asks(asks) if !asks.is_empty() => {
                let approve = asks.iter().any(|a| a.kind == protocol::AskKind::Approve);
                label(
                    &mut self.live,
                    if approve {
                        "Waiting for your approval…"
                    } else {
                        "Waiting for your answer…"
                    },
                );
                self.clarify_picked.clear();
            }
            ThreadEvent::Usage(u) => self.speed.usage(u),
            _ => {}
        }
    }

    /// Its history arrived (`thread.open`): a thread already on screen keeps what it has and only
    /// catches up. False: it is another thread's.
    pub fn on_history(&mut self, id: &ThreadId, open: protocol::ThreadOpen) -> bool {
        if self.id.as_ref() != Some(id) {
            return false;
        }
        let running = open.running;
        self.speed.restore(open.speed.clone());
        if !self.journal.is_empty() {
            self.journal.catch_up(open);
        } else {
            self.journal = ::thread::Thread::open(open);
        }
        self.history_error = None;
        self.loaded = true;
        self.live = running.then(|| Live {
            label: "Working…".into(),
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveThread;
    use protocol::{StreamKind, ThreadEvent};

    #[test]
    fn a_thread_follows_its_own_run() {
        let mut t = ActiveThread::default();
        let label = |t: &ActiveThread| t.live.as_ref().map(|l| l.label.to_string());
        t.on_thread_event(&ThreadEvent::Run(true));
        assert_eq!(label(&t).as_deref(), Some("Thinking…"));
        t.on_thread_event(&ThreadEvent::Stream(StreamKind::Text, "Hi".into()));
        assert_eq!(label(&t).as_deref(), Some("Writing…"));
        t.on_thread_event(&ThreadEvent::Run(false));
        assert!(t.live.is_none());
    }
}
