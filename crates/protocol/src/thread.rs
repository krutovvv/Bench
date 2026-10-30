//! A thread as the agent tells it: its journal's items, its queue, its inserts, its live run
//! (`ThreadOpen`, the answer to `thread.open`) and how each changes (`ThreadEvent`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

/// One item of the thread's journal. `key` is unique and stable; `seq` is the Pi entry id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ThreadItem {
    User {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        text: String,
        #[ts(optional)]
        #[serde(default)]
        at: Option<f64>,
        /// The Pi submission that brought it, when the agent's records say.
        #[ts(optional)]
        #[serde(default)]
        sub: Option<String>,
        /// Bench's id for the message (the `request_id` it was sent with): the row shown on
        /// Enter is this one (docs/thread-objects.md).
        #[ts(optional)]
        #[serde(default)]
        request: Option<String>,
    },
    Thought {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        text: String,
        #[ts(optional)]
        #[serde(default)]
        at: Option<f64>,
    },
    Text {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        text: String,
        /// The words of an assistant message that also called a tool: the agent's narrative
        /// of what it is about to do, not an answer. Pi's own signal — the message's blocks.
        /// `None` is an agent too old to mark them; such texts are judged by their place.
        #[ts(optional)]
        #[serde(default)]
        narration: Option<bool>,
        #[ts(optional)]
        #[serde(default)]
        at: Option<f64>,
        #[ts(optional)]
        #[serde(default)]
        error: Option<String>,
    },
    Tool {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        call_id: String,
        name: String,
        #[ts(type = "unknown")]
        #[serde(default)]
        args: Value,
        /// What it acts on, in one line (a command, a path).
        #[serde(default)]
        preview: String,
        #[ts(optional)]
        #[serde(default)]
        at: Option<f64>,
    },
    Result {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        call_id: String,
        name: String,
        output: String,
        failed: bool,
        /// An edit's unified patch.
        #[ts(optional)]
        #[serde(default)]
        diff: Option<String>,
        #[ts(optional)]
        #[serde(default)]
        at: Option<f64>,
    },
    Insert {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        id: String,
        insert_key: String,
    },
    Fail {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        text: String,
    },
    /// The agent's plan (the `todo` tool): a whole snapshot every time it is rewritten. The rows
    /// merge snapshots of one `id` into the plan of the work that wrote them.
    Plan {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        id: String,
        todos: Vec<PlanStep>,
    },
    /// The agent puts a question (the `clarify` tool) or an approval: a card of its own, in the
    /// work that asked. `id` is the prompt's, so the answer can be brought back to it.
    Ask {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        id: String,
        /// "clarify" or "approve".
        ask_kind: String,
        question: String,
        /// Extra text under the question: the command an approval wants to run.
        #[ts(optional)]
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        choices: Vec<String>,
        #[serde(default)]
        multi: bool,
    },
    /// What a turn changed in the repository the thread works in (`agent/src/runtime/changes.ts`):
    /// the trees at its start and end, and each file that differs. Written after the turn's answer.
    Changes {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        root: String,
        base: String,
        head: String,
        files: Vec<FileChange>,
    },
    /// A turn's review: findings on the changes of item `of` (`agent/src/runtime/review.ts`);
    /// `error` when the review could not be made.
    Review {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        of: String,
        findings: Vec<Finding>,
        #[ts(optional)]
        #[serde(default)]
        error: Option<String>,
    },
    /// A question's answer ("" is a decline): it settles the [`ThreadItem::Ask`] of the same id.
    Asked {
        key: String,
        #[ts(type = "number")]
        seq: u64,
        id: String,
        #[ts(optional)]
        #[serde(default)]
        answer: Option<String>,
    },
}

/// A file a turn changed: its absolute path and lines added and removed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct FileChange {
    pub path: String,
    #[ts(type = "number")]
    pub add: u64,
    #[ts(type = "number")]
    pub del: u64,
    #[ts(optional)]
    #[serde(default)]
    pub binary: Option<bool>,
    /// The file is a spec (`agent/src/specs.ts`): its title.
    #[ts(optional)]
    #[serde(default)]
    pub spec: Option<String>,
}

/// A reviewer's finding: a file (absolute), its lines ("12-14"), what is wrong.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Finding {
    pub path: String,
    #[ts(optional)]
    #[serde(default)]
    pub lines: Option<String>,
    pub body: String,
}

/// One step of the agent's plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct PlanStep {
    pub id: String,
    pub content: String,
    /// "pending", "in_progress", "completed", "cancelled".
    pub status: String,
    #[ts(optional)]
    #[serde(default)]
    pub parent: Option<String>,
}

impl ThreadItem {
    pub fn key(&self) -> &str {
        match self {
            ThreadItem::User { key, .. }
            | ThreadItem::Thought { key, .. }
            | ThreadItem::Text { key, .. }
            | ThreadItem::Tool { key, .. }
            | ThreadItem::Result { key, .. }
            | ThreadItem::Insert { key, .. }
            | ThreadItem::Fail { key, .. }
            | ThreadItem::Plan { key, .. }
            | ThreadItem::Ask { key, .. }
            | ThreadItem::Changes { key, .. }
            | ThreadItem::Review { key, .. }
            | ThreadItem::Asked { key, .. } => key,
        }
    }

    pub fn seq(&self) -> u64 {
        match self {
            ThreadItem::User { seq, .. }
            | ThreadItem::Thought { seq, .. }
            | ThreadItem::Text { seq, .. }
            | ThreadItem::Tool { seq, .. }
            | ThreadItem::Result { seq, .. }
            | ThreadItem::Insert { seq, .. }
            | ThreadItem::Fail { seq, .. }
            | ThreadItem::Plan { seq, .. }
            | ThreadItem::Ask { seq, .. }
            | ThreadItem::Changes { seq, .. }
            | ThreadItem::Review { seq, .. }
            | ThreadItem::Asked { seq, .. } => *seq,
        }
    }

    /// When it happened, Unix milliseconds, if the entry says.
    pub fn at(&self) -> Option<f64> {
        match self {
            ThreadItem::User { at, .. }
            | ThreadItem::Thought { at, .. }
            | ThreadItem::Text { at, .. }
            | ThreadItem::Tool { at, .. }
            | ThreadItem::Result { at, .. } => *at,
            ThreadItem::Insert { .. }
            | ThreadItem::Fail { .. }
            | ThreadItem::Plan { .. }
            | ThreadItem::Ask { .. }
            | ThreadItem::Changes { .. }
            | ThreadItem::Review { .. }
            | ThreadItem::Asked { .. } => None,
        }
    }

    /// The order key: (entry id, place inside the entry). "9.2" → (9, 2), "11" → (11, 0).
    pub fn order(&self) -> (u64, u32) {
        let sub = self
            .key()
            .split_once('.')
            .and_then(|(_, s)| s.parse().ok())
            .unwrap_or(0);
        (self.seq(), sub)
    }
}

/// A message waiting in the inbox, in the order the agent will take them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct QueuedItem {
    pub key: String,
    pub sub: String,
    pub mode: QueueMode,
    pub text: String,
    /// Bench's id for the message, when Bench sent it.
    #[ts(optional)]
    #[serde(default)]
    pub request: Option<String>,
}

/// How a waiting message joins the run (Pi's inbox modes): after the current tools, or when the
/// run has answered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum QueueMode {
    #[serde(rename = "steer")]
    Steer,
    #[default]
    #[serde(rename = "followUp")]
    FollowUp,
}

/// The answer to `thread.open {thread, since}`: the thread as it is now. What changes after it
/// comes as `ThreadEvent`s.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct ThreadOpen {
    pub items: Vec<ThreadItem>,
    #[serde(default)]
    pub queue: Vec<QueuedItem>,
    #[serde(default)]
    pub inserts: Inserts,
    #[serde(default)]
    pub running: bool,
    /// The live run's holds (the person's pauses), oldest first; the last one open while it
    /// stands held. Empty when no run lives.
    #[serde(default)]
    pub holds: Vec<Hold>,
    /// The thread's background processes, each placed at the call that started it.
    #[serde(default)]
    pub processes: Vec<Process>,
    /// What the live run waits for the person to answer.
    #[serde(default)]
    pub asks: Vec<Ask>,
    /// Its place in the line of threads waiting for their turn to work, from 1; `None`: not in it.
    #[ts(optional)]
    #[serde(default)]
    pub in_line: Option<u32>,
    #[serde(default)]
    pub info: ThreadInfo,
    /// Its numbers so far; `None` before its first answer.
    #[ts(optional)]
    #[serde(default)]
    pub usage: Option<Usage>,
    /// How fast the model wrote its last answer, as Bench measured and the agent kept it.
    #[serde(default)]
    pub speed: SpeedStat,
    #[serde(default)]
    #[ts(type = "number")]
    pub head: u64,
}

/// The thread's inserts by id (agent/src/runtime/inserts.ts, the `bench.inserts` document).
pub type Inserts = BTreeMap<String, InsertState>;

/// One insert's state: what it is, what it shows, how it sits in the thread.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct InsertState {
    /// What it is: "ssh:<machine>", "tools", "browser".
    pub key: String,
    /// Its own state (the list, the tool, the browser's tab).
    #[ts(type = "Record<string, unknown>")]
    #[serde(default)]
    pub state: Map<String, Value>,
    #[serde(default)]
    pub folded: bool,
    #[serde(default)]
    pub removed: bool,
    /// After how many of the person's messages it sits, when not where its entry was written.
    #[ts(optional, type = "number")]
    #[serde(default)]
    pub after: Option<u64>,
}

/// A change to an insert: each field present replaces that field.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct InsertPatch {
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[ts(optional, type = "Record<string, unknown>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Map<String, Value>>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folded: Option<bool>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
    #[ts(optional, type = "number")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
}

/// What the live run waits for the person to answer: a question (`clarify`) or a command to
/// approve. `id` is the waiting call's: the card in the thread stands at that call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Ask {
    pub id: String,
    pub kind: AskKind,
    /// What the person may answer (a question's choices; an approval's once/session/always/deny).
    #[serde(default)]
    pub choices: Vec<String>,
    /// An approval's command and why the agent wants it.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum AskKind {
    #[default]
    Clarify,
    Approve,
}

/// The thread's settings as the agent has them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct ThreadInfo {
    /// Its reasoning effort; "" is the provider's default.
    #[serde(default)]
    pub effort: String,
    /// The fallback model answered its last call because the main one failed.
    #[ts(optional)]
    #[serde(default)]
    pub fallback: Option<Fallback>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Fallback {
    pub model: String,
    pub reason: String,
}

/// A thread's numbers: its context, its tokens, its calls.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Usage {
    pub model: String,
    #[ts(type = "number")]
    pub context_used: u64,
    #[ts(type = "number")]
    pub context_max: u64,
    #[ts(type = "number")]
    pub compressions: u64,
    #[ts(optional, type = "number")]
    #[serde(default)]
    pub cache_hit_pct: Option<u64>,
    #[ts(type = "number")]
    pub input: u64,
    #[ts(type = "number")]
    pub output: u64,
    #[ts(type = "number")]
    pub calls: u64,
    /// The agent's own average: output tokens over whole-call time (prefill included).
    #[ts(optional)]
    #[serde(default)]
    pub avg_tps: Option<f64>,
    /// The evaluator's requests for this thread (Settings → Agents → Evaluator); none while it is off.
    #[ts(optional, type = "number")]
    #[serde(default)]
    pub judge_calls: Option<u64>,
    /// How often its answer changed the course: a search reordered, a nudge, a turn sent back.
    #[ts(optional, type = "number")]
    #[serde(default)]
    pub judge_nudges: Option<u64>,
}

/// How fast the model wrote in a thread: Bench measures it from the stream (only it sees when
/// each token arrived) and the agent keeps it with the thread.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct SpeedStat {
    /// Tokens per second, one sample for each second of the last answer.
    #[serde(default)]
    pub samples: Vec<f64>,
    #[ts(optional)]
    #[serde(default)]
    pub prefill: Option<f64>,
    #[ts(optional)]
    #[serde(default)]
    pub generation: Option<f64>,
    /// Characters per token, as measured.
    #[ts(optional)]
    #[serde(default)]
    pub ratio: Option<f64>,
}

impl SpeedStat {
    /// A thread that never ran has nothing to show.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty() && self.prefill.is_none() && self.generation.is_none()
    }
}

/// A background process the agent started (`process start`): a Pi task of the conversation,
/// read from its record. It stands in the work at the call `call_id`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Process {
    /// The Pi task's id: what Stop names.
    pub id: String,
    pub call_id: String,
    pub command: String,
    pub state: ProcessState,
    #[ts(optional, type = "number")]
    #[serde(default)]
    pub exit_code: Option<i64>,
    /// ms, like an item's `at`.
    #[ts(optional)]
    #[serde(default)]
    pub started: Option<f64>,
    #[ts(optional)]
    #[serde(default)]
    pub ended: Option<f64>,
    /// The last lines of its output.
    #[serde(default)]
    pub tail: String,
    /// Why it could not run, when it failed.
    #[ts(optional)]
    #[serde(default)]
    pub error: Option<String>,
}

/// Where a background process is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ProcessState {
    #[default]
    Running,
    /// Its command ended by itself, with `exit_code`.
    Exited,
    /// The person or the agent stopped it.
    Stopped,
    /// The agent restarted while it ran: it ended with the agent and was not started again.
    Restarted,
    /// It could not run.
    Failed,
}

/// A stretch of a run the person held (paused): `from` and `to` in ms like an item's `at`;
/// `to` is `None` while it still holds. Its time is not the work's: the work's clock skips it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Hold {
    pub from: f64,
    #[ts(optional)]
    #[serde(default)]
    pub to: Option<f64>,
}

/// Which part of the answer being written a stream delta extends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum StreamKind {
    Text,
    Thought,
    /// The text being streamed is narration, not an answer. An empty delta says the text
    /// streamed so far turned out to be narration: the model began a tool call mid-message,
    /// which is Pi's sign that these words belong to the work, not to the answer.
    Narration,
}

/// A change of a thread, as Pi committed it or the live run made it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ThreadEvent {
    /// New journal items, in order.
    Append(Vec<ThreadItem>),
    /// The answer being written grew; its entry's `Append` replaces it.
    Stream(StreamKind, String),
    /// The whole queue, after every change.
    Queue(Vec<QueuedItem>),
    /// The inserts' state document, whole.
    Inserts(Inserts),
    /// A run started or ended.
    Run(bool),
    /// The person held the run or let it go on: the run's holds, whole.
    Holds(Vec<Hold>),
    /// Its place in the line of threads waiting for their turn to work, from 1; `None`: out of it.
    Line(Option<u32>),
    /// A background process started, wrote output or ended: its record, whole.
    Process(Process),
    /// What the live run waits for the person to answer, whole.
    Asks(Vec<Ask>),
    /// Its settings changed (effort, the fallback answering).
    Info(ThreadInfo),
    /// Its numbers after a call.
    Usage(Usage),
}
