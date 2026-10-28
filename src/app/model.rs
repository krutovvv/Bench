//! Plain data shared by the agent's session and the UI.

use serde_json::Value;

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

/// A thread as the sidebar lists it: what its agent says of it (`protocol::ThreadSummary`), and
/// which agent that is.
#[derive(Clone, Debug, Default)]
pub struct ThreadRow {
    pub id: protocol::ThreadId,
    pub title: String,
    pub messages: u64,
    pub running: bool,
    /// The person paused its run: it stands still until resumed.
    pub paused: bool,
    /// Where it was started: "local" (Bench), "telegram", "cron", …
    pub source: String,
    /// The project it belongs to, as Bench keys projects (`ProjectKey`); its agent's Host when
    /// it has none.
    pub project: String,
    /// The server whose agent keeps it (an ~/.ssh/config alias); "" = this Mac's.
    pub host: String,
    /// The folder it works in, and the top of its git repo.
    pub cwd: String,
    pub repo: Option<String>,
    /// Who it is with (a Telegram thread's person).
    pub with: Option<String>,
}

impl ThreadRow {
    /// The agent `host`'s summary of a thread, placed in the project `project`.
    pub fn new(host: &str, project: String, t: protocol::ThreadSummary) -> Self {
        Self {
            id: t.id,
            title: t.title,
            messages: t.messages,
            running: t.running,
            paused: t.paused,
            source: t.source,
            project,
            host: host.to_string(),
            cwd: t.cwd,
            repo: t.repo,
            with: t.with,
        }
    }
}

/// A piece the person put into a thread with "/" (the terminal, the tasks, the utilities, the
/// browser's tab): kept by the agent as a Pi entry where it was put and a document of its state
/// (agent/src/runtime/inserts.ts), read from the thread's journal (`Thread::insert_list`). Its look and
/// behaviour: `crate::inserts`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Insert {
    pub id: String,
    /// What it is: "ssh:local" (the terminal on the thread's machine), "tasks", "tools",
    /// "browser".
    pub key: String,
    /// Its own state (the list, the tool, the browser's tab).
    pub state: serde_json::Map<String, Value>,
    pub folded: bool,
}

impl Insert {
    pub fn new(id: String, s: protocol::InsertState) -> Self {
        Self {
            id,
            key: s.key,
            state: s.state,
            folded: s.folded,
        }
    }

    /// A string field of its state.
    pub fn get(&self, field: &str) -> String {
        self.state
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
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
}

impl ProjectEntry {
    pub fn key(&self) -> ProjectKey {
        ProjectKey::parse(&self.id)
    }

    /// An agent's own environment (the Host of this Mac or of a server).
    pub fn is_host(&self) -> bool {
        self.key().is_host()
    }

    pub fn location(&self) -> Location {
        Location {
            agent: self.agent.clone(),
            machine: self.machine.clone(),
            folder: self.folder.clone(),
        }
    }
}

/// Human name of a session source, for grouping the session list.
pub fn source_label(source: &str) -> String {
    match source {
        "" | "local" | "desktop" | "tui" | "cli" | "dashboard" => "Bench".into(),
        "telegram" => "Telegram".into(),
        "cron" => "Cron".into(),
        "api_server" | "api" => "API".into(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
    }
}

/// A slash command from the agent's commands, a saved prompt, or a tool to attach to the
/// thread (`/tasks today`).
#[derive(Clone, Debug, Default)]
pub struct SlashCommand {
    /// "/goal".
    pub name: String,
    pub desc: String,
    pub category: String,
    /// A tool's subject key ("tasks:today"); empty for commands and prompts.
    pub key: String,
}

/// What the title bar's repository chip opens: the few things worth knowing about the repo the
/// thread works in, read with git where it lives (this Mac, or over SSH).
#[derive(Clone, Debug)]
pub struct RepoInfo {
    /// Files the working tree has changed since the last commit.
    pub dirty: usize,
    /// The newest commits, newest first.
    pub commits: Vec<CommitBrief>,
    /// The `origin` URL, when the repo has one.
    pub remote: Option<String>,
}

/// One commit as the chip's menu shows it.
#[derive(Clone, Debug)]
pub struct CommitBrief {
    pub sha: String,
    pub subject: String,
    /// "2 hours ago", git's own reckoning.
    pub age: String,
}

pub const APPROVAL_MODES: &[&str] = &["off", "smart", "manual"];

/// A thread's numbers (the status bar's context and speed panels).
pub use protocol::Usage;

pub const EFFORTS: &[&str] = &["low", "medium", "xhigh"];

/// The git repo a thread works in.
#[derive(Clone, Debug, PartialEq)]
pub struct Repo {
    /// The main checkout: `~/code/app`, also for chats in `~/code/app/.worktrees/x`.
    pub root: String,
    /// Where this chat's files are: the main checkout or its branch folder.
    pub dir: String,
    pub branch: String,
}

impl Repo {
    /// From any folder inside a repo (`top` = `git rev-parse --show-toplevel` of it).
    pub fn from_top(top: &str, branch: &str) -> Self {
        let root = match top.find("/.worktrees/") {
            Some(i) => top[..i].to_string(),
            None => top.to_string(),
        };
        Self {
            root,
            dir: top.to_string(),
            branch: branch.to_string(),
        }
    }

    pub fn name(&self) -> String {
        self.root
            .rsplit('/')
            .next()
            .unwrap_or(&self.root)
            .to_string()
    }

    /// The chat works in its own branch folder (not the main checkout).
    pub fn in_branch_folder(&self) -> bool {
        self.dir != self.root
    }
}

/// A saved text snippet (Settings → Prompts), inserted from the composer with `@`: a Markdown
/// file `<agent home>/prompts/<title>.md` on the agent's machine.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    pub title: String,
    pub body: String,
    /// Where it lives; empty until first saved.
    pub path: String,
}
