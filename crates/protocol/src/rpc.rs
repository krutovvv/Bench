//! What Bench asks its agent: one type per method, its params, and what it answers
//! (`Request::Response`). The agent's handlers are typed by the same list (`Methods` in
//! agent/src/protocol.gen.ts), so a method and its answer are one fact in one place.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::thread::{InsertPatch, SpeedStat};
use crate::ThreadId;

/// A method of the agent: its name on the wire and the type of its answer.
pub trait Request: Serialize + Send + 'static {
    const METHOD: &'static str;
    type Response: DeserializeOwned + Send + 'static;
}

/// An answer that says only that it was done.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Done {}

/// One thread as the sidebar lists it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct ThreadSummary {
    pub id: ThreadId,
    pub title: String,
    #[ts(type = "number")]
    pub messages: u64,
    pub running: bool,
    /// The person paused its run.
    pub paused: bool,
    /// Its run waits for the person: a question to answer or a command to approve.
    #[serde(default)]
    pub waiting: bool,
    /// Its place in the line of threads waiting for a stream of the model, from 1.
    #[ts(optional)]
    #[serde(default)]
    pub in_line: Option<u32>,
    /// Where it was started: "local" (Bench), "telegram", "cron".
    pub source: String,
    /// The project it belongs to; `None` for the agent's own (Host) threads.
    #[ts(optional)]
    #[serde(default)]
    pub project: Option<String>,
    /// Unix ms of its last change.
    #[serde(default)]
    pub updated: f64,
    /// The folder it works in, and the top of the git repo that folder is in, if any.
    #[serde(default)]
    pub cwd: String,
    #[ts(optional)]
    #[serde(default)]
    pub repo: Option<String>,
    /// The thread it was forked from: the sidebar shows it under that one.
    #[ts(optional)]
    #[serde(default)]
    pub parent: Option<ThreadId>,
    /// Who it is with, for a thread from Telegram (the person's name there).
    #[ts(optional)]
    #[serde(default)]
    pub with: Option<String>,
}

/// The new thread's id.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Created {
    pub thread: ThreadId,
}

/// What a slash command did: printed something, or became the message to send.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "text", rename_all = "lowercase")]
pub enum CommandResult {
    Output(String),
    Send(String),
}

/// A slash command the composer offers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Command {
    /// "/model".
    pub name: String,
    pub description: String,
}

// The params are declared through the macro, where ts-rs cannot see an `Option` for what it is:
// an optional param reads `T | null` in TypeScript (Bench always sends the key).
macro_rules! requests {
    ($( $(#[$doc:meta])* $name:ident = $method:literal -> $response:ty { $( $(#[$fdoc:meta])* $field:ident : $ty:ty ),* $(,)? } )*) => {
        $(
            $(#[$doc])*
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
            pub struct $name { $( $(#[$fdoc])* pub $field: $ty ),* }

            impl Request for $name {
                const METHOD: &'static str = $method;
                type Response = $response;
            }
        )*

        /// Each method's declaration, and `Methods`: the method → [params, answer] map the
        /// agent types its handlers with.
        pub(crate) fn declarations() -> Vec<String> {
            let mut out = vec![$( $name::decl() ),*];
            out.push(format!(
                "type Methods = {{ {} }};",
                [$( format!("\"{}\": [{}, {}]", $method, stringify!($name), <$response as TS>::name()) ),*].join(", ")
            ));
            out
        }
    };
}

requests! {
    /// Every thread the agent keeps (the sidebar's list).
    ListThreads = "threads.list" -> Vec<ThreadSummary> {}
    /// A new thread in a project (`None`: the agent's own), working in `cwd` or the project's folder.
    CreateThread = "thread.create" -> Created {
        #[serde(default)] project: Option<String>,
        #[serde(default)] cwd: Option<String>,
    }
    /// The thread as it is now, its items after entry `since` (all of them for 0).
    OpenThread = "thread.open" -> crate::thread::ThreadOpen {
        thread: ThreadId,
        #[ts(type = "number")] #[serde(default)] since: u64,
    }
    RenameThread = "thread.rename" -> Done { thread: ThreadId, title: String }
    /// Review the turn whose changes are the journal's item `of`: its findings come into the
    /// journal (`ThreadItem::Review`) when the model has read it.
    ReviewTurn = "thread.review" -> Done { thread: ThreadId, of: String }
    DeleteThread = "thread.delete" -> Done { thread: ThreadId }
    /// A new thread with this one's history through entry `at` (Pi's fork).
    ForkThread = "thread.fork" -> Created { thread: ThreadId, #[ts(type = "number")] at: u64 }
    /// The person's message. `request` is Bench's id for it: the row shown on Enter is matched by
    /// it. `queue`: it waits behind the run (a follow-up) instead of joining it. `context` is what
    /// the thread's inserts show, for the agent only.
    SendMessage = "thread.send" -> Done {
        thread: ThreadId,
        text: String,
        #[serde(default)] context: String,
        #[serde(default)] request: Option<String>,
        #[serde(default)] queue: bool,
        /// The reasoning effort to answer with, when the person changed it.
        #[serde(default)] effort: Option<String>,
    }
    /// Stop the run: what it does now ends, the queue stays.
    StopThread = "thread.stop" -> Done { thread: ThreadId }
    /// Run again the work Pi stopped after too many restarts.
    RetryThread = "thread.retry" -> Done { thread: ThreadId }
    /// Hold the run at its next step (`paused`), or let it go on.
    PauseThread = "thread.pause" -> Done { thread: ThreadId, paused: bool }
    /// Take a waiting message (its submission `sub`) back out of the queue.
    WithdrawQueued = "thread.queue.withdraw" -> Done { thread: ThreadId, sub: String }
    /// Send a waiting message now, into the run.
    SendQueuedNow = "thread.queue.send" -> Done { thread: ThreadId, sub: String }
    /// Fix the words of a message that still waits.
    EditQueued = "thread.queue.edit" -> Done { thread: ThreadId, sub: String, text: String }
    /// An insert put where the thread is now.
    PutInsert = "thread.insert" -> Done {
        thread: ThreadId,
        id: String,
        key: String,
        #[ts(type = "Record<string, unknown>")] state: serde_json::Map<String, serde_json::Value>,
    }
    PatchInsert = "thread.insert.patch" -> Done { thread: ThreadId, id: String, patch: InsertPatch }
    /// The person's answer to what the run asked (`Ask.id`); "" declines a question.
    AnswerAsk = "thread.answer" -> Done { thread: ThreadId, ask: String, answer: String }
    SetEffort = "thread.effort" -> Done { thread: ThreadId, effort: String }
    /// How fast the model wrote the answer that ended, for the agent to keep.
    SaveSpeed = "thread.speed" -> Done { thread: ThreadId, speed: SpeedStat }
    /// A slash command typed in the thread ("/model x").
    RunCommand = "thread.command" -> CommandResult { thread: ThreadId, command: String }
    /// The slash commands a thread offers.
    ListCommands = "commands.list" -> Vec<Command> {}
    /// Stop a background process of the thread.
    StopProcess = "process.stop" -> Done { thread: ThreadId, process: String }
}
