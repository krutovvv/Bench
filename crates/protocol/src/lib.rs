//! What Bench and its agent say to each other about a thread (agent/src/thread.ts).
//!
//! Gives: `ThreadItem`, `QueuedItem`, `ThreadOpen` (the answer to `thread.open`) and
//! `ThreadEvent` (the `thread.*` events). Takes: nothing; no GPUI, no I/O.

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
    pub mode: String,
    pub text: String,
}

/// The answer to `thread.open {session_id, since}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct ThreadOpen {
    pub items: Vec<ThreadItem>,
    #[serde(default)]
    pub queue: Vec<QueuedItem>,
    #[ts(type = "Record<string, unknown>")]
    #[serde(default)]
    pub inserts: Map<String, Value>,
    #[serde(default)]
    pub running: bool,
    /// The live run's holds (the person's pauses), oldest first; the last one open while it
    /// stands held. Empty when no run lives.
    #[serde(default)]
    pub holds: Vec<Hold>,
    #[serde(default)]
    #[ts(type = "number")]
    pub head: u64,
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

/// A `thread.*` event.
#[derive(Clone, Debug, PartialEq)]
pub enum ThreadEvent {
    /// New journal items, in order.
    Append(Vec<ThreadItem>),
    /// The answer being written grew; its entry's `Append` replaces it.
    Stream(StreamKind, String),
    /// The whole queue, after every change.
    Queue(Vec<QueuedItem>),
    /// The inserts' state document, whole.
    Inserts(Map<String, Value>),
    /// A run started or ended.
    Run(bool),
    /// The person held the run or let it go on: the run's holds, whole.
    Holds(Vec<Hold>),
}

impl ThreadEvent {
    /// Read an event from its type and payload; None for other events or a malformed one.
    pub fn parse(kind: &str, payload: &Value) -> Option<Self> {
        let items = || payload.get("items").cloned().unwrap_or(Value::Null);
        Some(match kind {
            "thread.append" => ThreadEvent::Append(serde_json::from_value(items()).ok()?),
            "thread.stream" => ThreadEvent::Stream(
                serde_json::from_value(payload.get("kind")?.clone()).ok()?,
                payload.get("delta")?.as_str()?.to_string(),
            ),
            "thread.queue" => ThreadEvent::Queue(serde_json::from_value(items()).ok()?),
            "thread.inserts" => ThreadEvent::Inserts(items().as_object()?.clone()),
            "thread.run" => ThreadEvent::Run(payload.get("running")?.as_bool()?),
            "thread.pause" => {
                ThreadEvent::Holds(serde_json::from_value(payload.get("holds")?.clone()).ok()?)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_what_the_agent_sends() {
        let open: ThreadOpen = serde_json::from_value(json!({
            "items": [
                {"key": "5.0", "seq": 5, "kind": "user", "text": "fix it", "at": 1005},
                {"key": "9.2", "seq": 9, "kind": "tool", "call_id": "c1", "name": "bash", "args": {"command": "ls"}},
                {"key": "10.0", "seq": 10, "kind": "result", "call_id": "c1", "name": "bash", "output": "a", "failed": false},
                {"key": "11", "seq": 11, "kind": "insert", "id": "i1", "insert_key": "tasks"},
                {"key": "12", "seq": 12, "kind": "fail", "text": "no model"}
            ],
            "queue": [{"key": "q31", "sub": "31", "mode": "followUp", "text": "next"}],
            "inserts": {"i1": {"key": "tasks"}},
            "running": true,
            "head": 12
        }))
        .unwrap();
        assert_eq!(open.items.len(), 5);
        assert_eq!(open.items[1].order(), (9, 2));
        assert_eq!(open.items[3].order(), (11, 0));
        assert_eq!(open.queue[0].text, "next");
        assert!(open.running);
    }

    #[test]
    fn events() {
        assert_eq!(
            ThreadEvent::parse("thread.stream", &json!({"kind": "text", "delta": "Hel"})),
            Some(ThreadEvent::Stream(StreamKind::Text, "Hel".into()))
        );
        assert_eq!(
            ThreadEvent::parse("thread.run", &json!({"running": false})),
            Some(ThreadEvent::Run(false))
        );
        assert_eq!(
            ThreadEvent::parse("thread.pause", &json!({"holds": [{"from": 5.0}]})),
            Some(ThreadEvent::Holds(vec![Hold { from: 5., to: None }]))
        );
        assert_eq!(
            ThreadEvent::parse("thread.queue", &json!({"items": []})),
            Some(ThreadEvent::Queue(vec![]))
        );
        assert_eq!(ThreadEvent::parse("message.delta", &json!({})), None);
        assert!(
            matches!(ThreadEvent::parse("thread.append", &json!({"items": [{"key": "1.0", "seq": 1, "kind": "user", "text": "x"}]})), Some(ThreadEvent::Append(v)) if v.len() == 1)
        );
    }
}

/// The TypeScript the agent imports (agent/src/protocol.gen.ts): the same types, from here.
pub fn typescript() -> String {
    let mut out = String::from(
        "// Generated from crates/protocol (cargo test -p protocol). Do not edit: change the Rust types.\n\n",
    );
    for decl in [
        ThreadItem::decl(),
        PlanStep::decl(),
        QueuedItem::decl(),
        ThreadOpen::decl(),
        Hold::decl(),
        StreamKind::decl(),
    ] {
        out.push_str("export ");
        out.push_str(&decl);
        out.push_str("\n\n");
    }
    out.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod generated {
    /// The agent's copy of the types is current; when it is not, it is written again and the
    /// test fails once, so the new file is seen and committed.
    #[test]
    fn agent_types_are_current() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../agent/src/protocol.gen.ts"
        );
        let want = super::typescript();
        let have = std::fs::read_to_string(path).unwrap_or_default();
        if have != want {
            std::fs::write(path, &want).unwrap();
            panic!("agent/src/protocol.gen.ts was out of date: written again, commit it");
        }
    }
}
