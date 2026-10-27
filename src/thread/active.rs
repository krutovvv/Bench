//! A thread's state: which session it is, its journal and the list showing it, its run, what
//! waits in its queue, what the agent asks in it, its project. Its `ThreadView` (`view.rs`) holds
//! it with the composer; the app routes each event to the view of its thread (`app/events.rs`).

use std::collections::HashSet;

use crate::app::Live;
use crate::app::model::Usage;
use crate::client::rpc::Evt;

pub struct ActiveThread {
    /// Stored id of the open session (what the session list shows).
    pub session: Option<String>,
    /// The id the gateway uses for prompts and events of the open session.
    pub runtime: Option<String>,
    /// The open thread: its journal, what is being written, its queue (`crates/thread`).
    pub journal: ::thread::Thread,
    /// The open thread on screen: its list and rows (`ui::thread`).
    pub tv: crate::thread::View,
    /// Opened pieces of work, by the work row's key; unfolded steps (work key, step); the source
    /// host whose pages are listed (work key, host).
    pub open_work: HashSet<String>,
    pub open_steps: HashSet<(String, usize)>,
    pub open_source: Option<(String, String)>,
    /// The open thread's texts, parsed once (`ui::media::Texts`).
    pub texts: crate::media::text::Texts,
    /// The open thread's history could not be read: why (shown with Retry instead of "Loading").
    pub history_error: Option<String>,
    pub live: Option<Live>,
    pub queued: Option<String>,
    /// The "Changed files" list under the last turn is unfolded (folded by default).
    pub files_open: bool,
    /// The open session's background processes (`thread/processes.rs`), the card's fold, and
    /// the process whose output is shown.
    pub processes: Vec<crate::app::model::BgProcess>,
    pub processes_open: bool,
    pub process_open: Option<String>,
    pub usage: Option<Usage>,
    /// The fallback model is answering the open thread: (its model, why the main one failed).
    pub fallback_on: Option<(String, String)>,
    pub effort: usize,
    /// A level was chosen before the session existed: it goes with the first message.
    pub effort_pending: bool,
    /// The open session's effort when it is none of EFFORTS (set elsewhere, e.g. `high`; "" is the
    /// provider's default): shown as it is, nothing checked.
    pub effort_other: Option<String>,
    /// The agent waits for an answer (approval mode manual/smart).
    pub approval: Option<crate::app::model::ApprovalReq>,
    /// A question the agent asks, the choices ticked so far (multi-select), the free answer.
    pub clarify: Option<crate::app::model::ClarifyReq>,
    pub clarify_picked: Vec<usize>,
    /// The agent's plan for this chat, and whether its list is unfolded.
    pub todos: Vec<crate::app::model::Todo>,
    pub plan_open: bool,
    /// The open session's project (None for chats that touch no repo).
    pub project: Option<crate::app::model::Project>,
    /// The project the open thread belongs to (a new one is created in it); "" = none.
    pub thread_project: String,
}

impl Default for ActiveThread {
    fn default() -> Self {
        Self {
            session: None,
            runtime: None,
            journal: Default::default(),
            tv: Default::default(),
            open_work: Default::default(),
            open_steps: Default::default(),
            open_source: None,
            texts: Default::default(),
            history_error: None,
            live: None,
            queued: None,
            files_open: false,
            processes: Vec::new(),
            processes_open: false,
            process_open: None,
            usage: None,
            fallback_on: None,
            effort: crate::app::model::EFFORTS.len() - 1,
            effort_pending: false,
            effort_other: None,
            approval: None,
            clarify: None,
            clarify_picked: Vec::new(),
            todos: Vec::new(),
            plan_open: false,
            project: None,
            thread_project: String::new(),
        }
    }
}

/// One thread as the app acts on it for its view: which session it is, where it lives, whether
/// it runs. The view makes it (`ActiveThread::thread_ref`) and hands it to what it asks of the app
/// — an insert's block, an answer's media, sending — so the app never has to read a view, which
/// may be drawing at that moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadRef {
    pub session: Option<String>,
    pub runtime: Option<String>,
    /// The project it belongs to; "" = none (its agent's Host).
    pub project: String,
    /// A run is live in it.
    pub live: bool,
}

impl ThreadRef {
    /// Its key for what is kept per thread (inserts, references): its session, or "draft" for a
    /// new chat.
    pub fn key(&self) -> String {
        self.session.clone().unwrap_or_else(|| "draft".into())
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
            session: self.session.clone(),
            runtime: self.runtime.clone(),
            project: self.thread_project.clone(),
            live: self.live.is_some(),
        }
    }

    /// The level chosen before the session existed, once: it goes with the first message.
    pub fn take_effort(&mut self, efforts: &[String]) -> Option<String> {
        std::mem::take(&mut self.effort_pending)
            .then(|| efforts.get(self.effort).cloned())
            .flatten()
    }

    /// What an event of this thread does to its own state (its journal, its run, what the agent
    /// asks). What it does to the window — the status bar's speed, a notification, a field — is
    /// the open thread's alone (`app/events.rs`).
    pub fn on_event(&mut self, evt: &Evt) {
        let label = |live: &mut Option<Live>, text: &str| {
            if let Some(live) = live.as_mut() {
                live.label = text.to_string().into();
            }
        };
        match evt {
            Evt::Thread(_, e) => self.journal.apply(e.clone()),
            Evt::Fallback(_, note) => self.fallback_on = note.clone(),
            Evt::Started(_) => {
                if self.live.is_none() {
                    self.live = Some(Live {
                        label: "Thinking…".into(),
                    });
                }
            }
            Evt::Delta(..) => label(&mut self.live, "Writing…"),
            Evt::Reasoning(..) => label(&mut self.live, "Thinking…"),
            Evt::ToolStart { name, detail, .. } => label(
                &mut self.live,
                &crate::thread::work::tool_label(name, detail),
            ),
            // A command that printed something (no prompt to send): the run is over.
            Evt::Dispatched { send: None, .. } => self.live = None,
            Evt::Approval(req) => {
                label(&mut self.live, "Waiting for your approval…");
                self.approval = Some(req.clone());
            }
            Evt::ApprovalDone(sid) => {
                if self.approval.as_ref().is_some_and(|a| a.session == *sid) {
                    self.approval = None;
                }
            }
            Evt::Clarify(req) => {
                label(&mut self.live, "Waiting for your answer…");
                self.clarify = Some(req.clone());
                self.clarify_picked.clear();
            }
            Evt::ClarifyDone(sid) => {
                if self.clarify.as_ref().is_some_and(|c| c.session == *sid) {
                    self.clarify = None;
                }
            }
            // The plan row shows work still ahead: once no step is pending or in progress (the
            // agent finished the plan, or dropped it as wrong), the row goes by itself — a plan
            // that is over tells nothing the thread does not already say.
            Evt::Todos(_, todos, _at) => {
                let ahead = todos
                    .iter()
                    .any(|t| t.status == "pending" || t.status == "in_progress");
                self.todos = if ahead { todos.clone() } else { Vec::new() };
                self.journal.set_plan(ahead);
            }
            Evt::Usage(_, usage) => self.usage = Some(usage.clone()),
            // Running ones first, then the newest.
            Evt::Processes(_, list) => {
                let mut list = list.clone();
                list.sort_by(|a, b| {
                    b.running.cmp(&a.running).then(
                        b.started
                            .unwrap_or(0.)
                            .partial_cmp(&a.started.unwrap_or(0.))
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                });
                self.processes = list;
            }
            Evt::Complete { .. } => self.live = None,
            _ => {}
        }
    }

    /// Its history arrived (`thread.open`): a thread already on screen keeps what it has and only
    /// catches up. False: it is another thread's.
    pub fn on_history(
        &mut self,
        session: &str,
        runtime: String,
        open: protocol::ThreadOpen,
    ) -> bool {
        if self.session.as_deref() != Some(session) {
            return false;
        }
        let running = open.running;
        if self.runtime.as_deref() == Some(runtime.as_str()) || !self.journal.is_empty() {
            self.journal.catch_up(open);
        } else {
            self.journal = ::thread::Thread::open(open);
        }
        self.runtime = Some(runtime);
        self.history_error = None;
        self.live = running.then(|| Live {
            label: "Working…".into(),
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveThread;
    use crate::app::model::Todo;
    use crate::client::rpc::Evt;

    fn todo(status: &str) -> Todo {
        Todo {
            content: "step".into(),
            status: status.into(),
            depth: 0,
        }
    }

    #[test]
    fn a_thread_follows_its_own_run() {
        let mut t = ActiveThread::default();
        t.on_event(&Evt::Started("r".into()));
        assert_eq!(
            t.live.as_ref().map(|l| l.label.to_string()).as_deref(),
            Some("Thinking…")
        );
        t.on_event(&Evt::Delta("r".into(), "Hi".into()));
        assert_eq!(
            t.live.as_ref().map(|l| l.label.to_string()).as_deref(),
            Some("Writing…")
        );
        t.on_event(&Evt::Todos("r".into(), vec![todo("in_progress")], 0.));
        assert_eq!(t.todos.len(), 1);
        t.on_event(&Evt::Todos("r".into(), vec![todo("completed")], 0.));
        assert!(t.todos.is_empty(), "a plan that is over goes");
        t.on_event(&Evt::Complete {
            session: "r".into(),
            text: "done".into(),
        });
        assert!(t.live.is_none());
    }
}
