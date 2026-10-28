//! The plain data every area uses: where an agent runs (`Target`), a project's key and where it
//! lives (`ProjectKey`, `Location`, `ProjectEntry`), the general agent settings, the reasoning
//! levels and approval modes. What one area keeps is in that area's folder.

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Local,
    Ssh(String),
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Target::Local => "This Mac".into(),
            Target::Ssh(host) => format!("{host} (SSH)"),
        }
    }
}

/// The agent settings that are no project's: the agent's general ones (`~/.bench/agents/default`).
/// A REST call about them names no settings at all (`client::rest::url`).
pub const GENERAL_AGENT: &str = "default";

/// A project as Bench addresses it (`docs/remote-projects.md`): the agent that keeps it — "" for
/// this Mac's, else a server's ~/.ssh/config alias — and its id there; `HOST` is an agent's own
/// environment (the machine's Host project). Written as one string where a plain key is needed
/// (sessions, marks, folds): the id for this Mac's, `<agent>:<id>` for a server's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProjectKey {
    pub agent: String,
    pub id: String,
}

impl ProjectKey {
    /// An agent's own environment: its Host project.
    pub const HOST: &'static str = "general";

    pub fn new(agent: &str, id: &str) -> Self {
        Self {
            agent: agent.to_string(),
            id: id.to_string(),
        }
    }

    /// The Host project of the agent under `agent`.
    pub fn host_of(agent: &str) -> Self {
        Self::new(agent, Self::HOST)
    }

    pub fn parse(key: &str) -> Self {
        match key.split_once(':') {
            Some((agent, id)) => Self::new(agent, id),
            None => Self::new("", key),
        }
    }

    pub fn is_host(&self) -> bool {
        self.id == Self::HOST
    }
}

impl std::fmt::Display for ProjectKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.agent.is_empty() {
            f.write_str(&self.id)
        } else {
            write!(f, "{}:{}", self.agent, self.id)
        }
    }
}

/// Where a project's work is: the agent that runs its threads, the machine its files and commands
/// are on (None: the agent's own), and its folder there (None: a folder the agent keeps for it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Location {
    pub agent: String,
    pub machine: Option<String>,
    pub folder: Option<String>,
}

impl Location {
    /// The machine as this Mac reaches it — "" this Mac, else an ~/.ssh/config alias: a server's
    /// agent's own machine, or the one this Mac's agent works on over SSH.
    pub fn machine_here(&self) -> String {
        self.machine.clone().unwrap_or_else(|| self.agent.clone())
    }

    /// The machine the agent is asked to reach for files (`/api/fs?machine=`): only when it is not
    /// the agent's own.
    pub fn files_machine(&self) -> Option<String> {
        self.machine.clone()
    }
}

/// What the person works on: the sidebar's unit. A folder on disk (a codebase) is optional.
#[derive(Clone, Debug, Default)]
pub struct ProjectEntry {
    /// `ProjectKey` as one string.
    pub id: String,
    pub name: String,
    /// The agent that keeps it: "" this Mac's, else a server's alias.
    pub agent: String,
    pub folder: Option<String>,
    /// The machine its work is done on when not its agent's own (an ~/.ssh/config host): the
    /// agent works there over SSH.
    pub machine: Option<String>,
    /// Has the watchable server browser among its own MCP servers: /browser is offered there.
    pub browser: bool,
    /// Has a Telegram bot of its own: its threads are also written to from Telegram.
    pub telegram: bool,
}

impl ProjectEntry {
    pub fn location(&self) -> Location {
        Location {
            agent: self.agent.clone(),
            machine: self.machine.clone(),
            folder: self.folder.clone(),
        }
    }
}

pub const APPROVAL_MODES: &[&str] = &["off", "smart", "manual"];

pub const EFFORTS: &[&str] = &["low", "medium", "xhigh"];

