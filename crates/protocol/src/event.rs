//! What the agent tells Bench without being asked: a thread changed, the list of threads
//! changed, the tasks changed, a project's environment changed. Every event names what it is
//! about.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::thread::ThreadEvent;
use crate::ThreadId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A thread was made, renamed, deleted, or started or ended a run: the list is read again.
    ThreadsChanged,
    /// A change of one thread.
    Thread { thread: ThreadId, event: ThreadEvent },
    /// The person's tasks changed (the `tasks` tool, another client).
    TasksChanged,
    /// A project's environment changed by itself (an MCP server connected after its sign-in in
    /// the browser): `project` is its id at the agent, "general" for the agent's Host.
    EnvChanged { project: String },
}
