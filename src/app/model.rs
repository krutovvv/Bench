//! Plain data shared by the gateway thread and the UI.

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

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub label: String,
    pub model: String,
}

#[derive(Clone, Debug)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub messages: u64,
    pub running: bool,
    /// The person paused its run: it stands still until resumed.
    pub paused: bool,
    /// Where the session lives: "local" (desktop/CLI), "telegram", "cron", …
    pub source: String,
    /// What the thread is about, its live block: "tasks:today", "ssh:local"; "" = a plain chat.
    pub subject: String,
    /// The project the thread belongs to (its id); "" = none.
    pub project: String,
    /// The server whose agent keeps it (an ~/.ssh/config alias); "" = this Mac's.
    pub host: String,
}

/// A piece the person put into a thread with "/" (the terminal, the tasks, the utilities, the
/// browser's tab): kept by the agent as a Pi entry where it was put and a
/// document of its state (agent/src/inserts.ts). Its look and behaviour: `crate::inserts`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Insert {
    pub id: String,
    /// What it is: "ssh:local" (the terminal on the thread's machine), "tasks", "tools",
    /// "browser".
    pub key: String,
    /// Its own state (the list, the tool, the browser's tab).
    pub state: serde_json::Map<String, Value>,
    pub folded: bool,
    /// After how many of the person's messages it sits.
    pub after: usize,
}

impl Insert {
    pub fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: v["id"].as_str()?.to_string(),
            key: v["key"].as_str()?.to_string(),
            state: v["state"].as_object().cloned().unwrap_or_default(),
            folded: v["folded"].as_bool().unwrap_or(false),
            after: v["after"].as_u64().unwrap_or(0) as usize,
        })
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

/// A slash command from the gateway's catalog, a saved prompt, or a tool to attach to the
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

/// The agent asks before running something (approval mode manual/smart): which pending
/// prompt it is. The card — command and choices — is the journal's (`AskCard`).
#[derive(Clone, Debug)]
pub struct ApprovalReq {
    pub session: String,
    /// The id of the call that waits for the approval.
    pub prompt_id: String,
    /// What it wants to run, why it asks, and what the person may answer.
    pub command: String,
    pub description: String,
    pub choices: Vec<String>,
}

pub const APPROVAL_MODES: &[&str] = &["off", "smart", "manual"];

/// The agent asks the user a question (the `clarify` tool): the live prompt, so an answer
/// reaches it. The question itself is the journal's (`AskCard`); these choices are what a
/// multi-select answer picks from, by index.
#[derive(Clone, Debug)]
pub struct ClarifyReq {
    pub session: String,
    pub prompt_id: String,
    pub generation: Option<i64>,
    pub choices: Vec<String>,
}

/// Context/usage readout for the status bar.
#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub model: String,
    pub context_used: u64,
    pub context_max: u64,
    pub compressions: u64,
    pub cache_hit_pct: Option<u64>,
    pub input: u64,
    pub output: u64,
    pub calls: u64,
    /// The agent's own average: output tokens over whole-call time (prefill included).
    pub avg_tps: Option<f64>,
}

impl Usage {
    pub fn from_json(v: &Value) -> Self {
        let n = |k: &str| {
            v[k].as_u64()
                .or_else(|| v[k].as_f64().map(|f| f as u64))
                .unwrap_or(0)
        };
        Self {
            model: v["model"].as_str().unwrap_or_default().to_string(),
            context_used: n("context_used"),
            context_max: n("context_max"),
            compressions: n("compressions"),
            cache_hit_pct: v["cache_hit_pct"].as_u64(),
            input: n("input"),
            output: n("output"),
            calls: n("calls"),
            avg_tps: v["avg_tps"].as_f64().filter(|t| *t > 0.),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkillRow {
    pub name: String,
    pub category: String,
}

pub const EFFORTS: &[&str] = &["low", "medium", "xhigh"];

/// Where a session works: its folder and, when that folder is in a git repo, the project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    pub cwd: String,
    /// The repo's top folder as the agent recorded it (for a branch folder, that folder itself).
    pub repo: String,
    pub branch: String,
}

/// A git project a session belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    /// The main checkout: `~/code/app`, also for chats in `~/code/app/.worktrees/x`.
    pub root: String,
    /// Where this chat's files are: the main checkout or its branch folder.
    pub dir: String,
    pub branch: String,
}

impl Project {
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
/// file `<profile home>/prompts/<title>.md` on the gateway's machine.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    pub title: String,
    pub body: String,
    /// Where it lives; empty until first saved.
    pub path: String,
}
