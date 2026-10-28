//! What Bench and its agent say to each other: the contract of the wire, in one place.
//!
//! - `rpc`: what Bench asks (`Request`: a method, its params, its answer);
//! - `event`: what the agent tells unasked (`Event`);
//! - `thread`: a thread as the agent tells it (`ThreadOpen`, `ThreadEvent`, `ThreadItem`, …).
//!
//! The agent imports the same types from agent/src/protocol.gen.ts, written from here by
//! `cargo test -p protocol`. No GPUI, no I/O.

pub mod event;
pub mod rpc;
pub mod thread;

pub use event::Event;
pub use rpc::*;
pub use thread::*;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A thread's id, as the agent keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(type = "string")]
pub struct ThreadId(pub String);

impl ThreadId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ThreadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ThreadId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ThreadId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// The TypeScript the agent imports (agent/src/protocol.gen.ts): the same types, from here.
pub fn typescript() -> String {
    let mut out = String::from(
        "// Generated from crates/protocol (cargo test -p protocol). Do not edit: change the Rust types.\n\n",
    );
    let mut decls = vec![
        ThreadItem::decl(),
        PlanStep::decl(),
        QueuedItem::decl(),
        ThreadOpen::decl(),
        InsertState::decl(),
        InsertPatch::decl(),
        Ask::decl(),
        AskKind::decl(),
        ThreadInfo::decl(),
        Fallback::decl(),
        Usage::decl(),
        SpeedStat::decl(),
        Hold::decl(),
        Process::decl(),
        ProcessState::decl(),
        StreamKind::decl(),
        ThreadEvent::decl(),
        Event::decl(),
        "type ThreadId = string;".to_string(),
        "type Inserts = Record<string, InsertState>;".to_string(),
        Done::decl(),
        ThreadSummary::decl(),
        Created::decl(),
        CommandResult::decl(),
        Command::decl(),
    ];
    decls.extend(rpc::declarations());
    for decl in decls {
        out.push_str("export ");
        out.push_str(&decl);
        out.push_str("\n\n");
    }
    out.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod tests;
