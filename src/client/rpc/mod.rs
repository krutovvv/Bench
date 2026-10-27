//! The JSON-RPC session on its own thread. The UI sends `Cmd`s and receives `Evt`s.

mod events;
mod session;

use std::sync::{Arc, mpsc};

use serde_json::Value;

use crate::app::model::{Profile, SessionRow, Target, Usage};
use crate::client::connection::Connection;

pub enum Cmd {
    Sessions(String),
    Open {
        profile: String,
        session: String,
    },
    Create {
        profile: String,
        /// The working folder (a project, or a branch folder of one); None = the agent's default.
        cwd: Option<String>,
        /// What the thread is about ("tasks:today", "ssh:local"); "" = a plain chat.
        subject: String,
        /// The project it belongs to (its id); "" = none.
        project: String,
    },
    /// An insert put into the thread where it is now (agent/src/inserts.ts).
    Insert {
        session: String,
        id: String,
        key: String,
        state: Value,
    },
    /// An insert changed: {key?, state?, folded?, removed?}.
    PatchInsert {
        session: String,
        id: String,
        patch: Value,
    },
    /// A message sent while the agent works: it waits in the conversation's queue (Pi's inbox)
    /// until the agent answers.
    /// A message pressed while the agent works: it waits in the queue. `request` is Bench's id
    /// for it, the row shown on Enter is matched by it (docs/thread-objects.md).
    Queue {
        session: String,
        text: String,
        context: String,
        request: String,
    },
    /// A waiting message taken back, or sent now (into the running work).
    QueueWithdraw {
        session: String,
        id: String,
    },
    QueueSend {
        session: String,
        id: String,
    },
    /// The words of a message that still waits: it has not gone anywhere, so they may be fixed.
    QueueEdit {
        session: String,
        id: String,
        text: String,
    },
    Send {
        profile: String,
        session: String,
        text: String,
        effort: Option<String>,
        /// What the thread's live block shows now (terminal lines, the tool's input…), for the
        /// agent only: it is not part of the message the person sees.
        context: String,
        /// Bench's id for the message, when a row shows it before the agent has it.
        request: Option<String>,
    },
    Interrupt {
        profile: String,
        session: String,
    },
    /// The person's hold: stand the run at its next step boundary (`true`), let it go on
    /// (`false`). The answer being written is let through — only new steps wait.
    SetPaused {
        session: String,
        paused: bool,
    },
    /// How fast the model wrote in the answer that just ended, as Bench measured it from the
    /// stream (`crate::statusbar::speed::Stat`). The agent keeps it with the thread.
    SaveSpeed {
        session: String,
        speed: Value,
    },
    /// `runtime` set when this client has the session open: it is closed first,
    /// since the gateway refuses to delete an active session.
    Delete {
        profile: String,
        session: String,
        runtime: Option<String>,
    },
    /// Only the reasoning effort, no prompt (diagnostics).
    SetEffort {
        session: String,
        effort: String,
    },
    /// The person's name for a thread.
    Rename {
        session: String,
        title: String,
    },
    /// The profile's slash commands (and skills).
    Catalog {
        profile: String,
    },
    /// The session's background processes (polled while it is open).
    Processes {
        profile: String,
        session: String,
    },
    /// Stop one of them.
    KillProcess {
        profile: String,
        session: String,
        id: String,
    },
    /// Run a slash command in a session.
    Dispatch {
        profile: String,
        session: String,
        name: String,
        arg: String,
    },
    /// Answer a question (`answer` empty = declined).
    Clarify {
        profile: String,
        req: crate::app::model::ClarifyReq,
        answer: String,
    },
    /// Answer an approval request.
    Approve {
        profile: String,
        req: crate::app::model::ApprovalReq,
        choice: String,
    },
    Shutdown,
}

pub enum Evt {
    Catalog(Vec<crate::app::model::SlashCommand>),
    /// (runtime session, its background processes)
    Processes(String, Vec<crate::app::model::BgProcess>),
    /// A slash command ran: either it printed something, or it wants a prompt sent.
    Dispatched {
        session: String,
        output: Option<String>,
        send: Option<String>,
    },
    Approval(crate::app::model::ApprovalReq),
    Clarify(crate::app::model::ClarifyReq),
    /// The pending question was answered, timed out or withdrawn.
    ClarifyDone(String),
    /// What the thread keeps of the speed of its last answer (its chart and its last speeds).
    Speed(String, crate::statusbar::speed::Stat),
    /// The pending approval was answered or withdrawn.
    ApprovalDone(String),
    Connected(Arc<Connection>),
    Error(String),
    /// A session's reasoning effort as the gateway has it ("" = the provider's default).
    Effort(String, String),
    /// The fallback model answered the session's last call: (its model, why the main one failed);
    /// None once the main model answers.
    Fallback(String, Option<(String, String)>),
    Profiles(Vec<Profile>),
    Sessions(String, Vec<SessionRow>),
    /// `session` is the stored id (session list); `runtime` the id prompts and events use.
    History {
        session: String,
        runtime: String,
        open: protocol::ThreadOpen,
    },
    /// A change of an open thread: (runtime session, the change).
    Thread(String, protocol::ThreadEvent),
    Created {
        session: String,
        runtime: String,
    },
    /// A thread's inserts as the agent keeps them (the whole list, in order).
    Inserts(String, Vec<crate::app::model::Insert>),
    /// The agent refused an insert's change: the thread's list is read again.
    InsertFailed(String, String),
    /// A new thread's session could not be made (why): its inserts and message wait for nothing.
    CreateFailed(String),
    /// A thread's history could not be read (session id, why): it stops "loading".
    ResumeFailed(String, String),
    /// The gateway says the session list changed (new session, generated title, …).
    SessionsChanged,
    Deleted,
    DeleteFailed(String),
    Started(String),
    Delta(String, String),
    Reasoning(String, String),
    /// A tool started: what the status says the agent is doing (name, what it acts on).
    ToolStart {
        session: String,
        name: String,
        detail: String,
    },
    ToolDone {
        session: String,
    },
    Usage(String, Usage),
    Complete {
        session: String,
        text: String,
    },
}

impl Evt {
    /// The runtime session an event of one thread is about (None: the app's, or a thread's by
    /// its stored id — its history, its creation, its inserts).
    pub fn runtime(&self) -> Option<&str> {
        match self {
            Evt::Processes(s, _)
            | Evt::ClarifyDone(s)
            | Evt::Speed(s, _)
            | Evt::ApprovalDone(s)
            | Evt::Effort(s, _)
            | Evt::Fallback(s, _)
            | Evt::Thread(s, _)
            | Evt::Started(s)
            | Evt::Delta(s, _)
            | Evt::Reasoning(s, _)
            | Evt::Usage(s, _) => Some(s),
            Evt::Dispatched { session, .. }
            | Evt::ToolStart { session, .. }
            | Evt::ToolDone { session }
            | Evt::Complete { session, .. } => Some(session),
            Evt::Approval(req) => Some(&req.session),
            Evt::Clarify(req) => Some(&req.session),
            _ => None,
        }
    }
}

/// Owns the gateway thread; dropping it shuts the session down.
pub struct Gateway {
    tx: mpsc::Sender<Cmd>,
}

impl Drop for Gateway {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
    }
}

impl Gateway {
    pub fn start(target: Target, evts: async_channel::Sender<Evt>) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(err) = session::run(&target, &rx, &evts) {
                let _ = evts.try_send(Evt::Error(format!("{err:#}")));
            }
        });
        Self { tx }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}
