//! A fake agent, as Zed's `FakeFs`: what answers instead of a real agent and its machine when one
//! is installed at start (`use_fake`, the demo's). `Client`, `Connection` and the few readers of
//! the machine (git, `~/.ssh/config`, a terminal's program, the browser's gateway) ask it first;
//! nothing else in Bench knows it is there.

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use serde_json::Value;

/// A browser tab the fake's gateway says is open, and its picture.
pub struct FakeTab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub jpeg: &'static [u8],
}

pub trait FakeAgent: Send + Sync {
    /// A JSON-RPC method (`crates/protocol`), its answer as the wire has it.
    fn rpc(&self, method: &str, params: Value) -> Result<Value>;
    /// A REST call: "GET", "POST"…, the path with its query, the body.
    fn rest(&self, method: &str, path: &str, body: Value) -> Result<Value>;
    /// `git -C <dir> <args>` on the agent's machine.
    fn git(&self, dir: &str, args: &[&str]) -> Result<String>;
    /// What `~/.ssh/config` lists.
    fn ssh_servers(&self) -> Vec<crate::system::sshconfig::Server>;
    /// The `sh -c` script a terminal on `host` runs instead of a shell.
    fn terminal_script(&self, host: &str) -> String;
    /// The tab the browser's gateway has open.
    fn browser_tab(&self) -> FakeTab;
    /// The agent's log's last lines.
    fn agent_log(&self) -> Vec<String>;
}

static FAKE: OnceLock<Arc<dyn FakeAgent>> = OnceLock::new();

/// From now on no real agent, machine or gateway is reached. Once, before the first window.
pub fn use_fake(agent: Arc<dyn FakeAgent>) {
    let _ = FAKE.set(agent);
}

pub fn fake() -> Option<Arc<dyn FakeAgent>> {
    FAKE.get().cloned()
}
