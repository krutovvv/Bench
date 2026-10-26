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
    /// Where the session lives: "local" (desktop/CLI), "telegram", "cron", …
    pub source: String,
    /// What the thread is about, its live block: "tasks:today", "ssh:<host>"; "" = a plain chat.
    pub subject: String,
    /// The project the thread belongs to (its id); "" = none.
    pub project: String,
    /// The server whose agent keeps it (an ~/.ssh/config alias); "" = this Mac's.
    pub host: String,
}

/// A piece the person put into a thread with "/" (a server's terminal, a file host, the tasks,
/// the utilities, the browser's tab): kept by the agent as a Pi entry where it was put and a
/// document of its state (agent/src/inserts.ts). Its look and behaviour: `crate::inserts`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Insert {
    pub id: String,
    /// What it is: "ssh:<alias>" (a terminal; "ssh:local" this Mac's), "files", "tasks",
    /// "tools", "browser".
    pub key: String,
    /// Its own state (the host and folder a file browser is on, the list, the tool).
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

/// What the person works on: the sidebar's unit. A folder on disk (a codebase) is optional.
#[derive(Clone, Debug, Default)]
pub struct ProjectEntry {
    pub id: String,
    pub name: String,
    pub folder: Option<String>,
    /// A server whose agent the project is (its threads, environment and files live there).
    pub host: Option<String>,
    /// Has the watchable server browser among its own MCP servers: /browser is offered there.
    pub browser: bool,
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

/// A background process the agent started in a session (`process.list`).
#[derive(Clone, Debug, PartialEq)]
pub struct BgProcess {
    pub id: String,
    pub command: String,
    pub running: bool,
    pub exit_code: Option<i64>,
    /// Unix seconds.
    pub started: Option<f64>,
    pub exited: Option<f64>,
    /// Why it ended, when the agent says (e.g. killed, timeout).
    pub reason: Option<String>,
    /// The last part of its output.
    pub tail: String,
}

/// A slash command from the gateway's catalog, a saved prompt, or a tool to attach to the
/// thread (`/ssh web-1`).
#[derive(Clone, Debug, Default)]
pub struct SlashCommand {
    /// "/goal".
    pub name: String,
    pub desc: String,
    pub category: String,
    /// A tool's subject key ("ssh:web-1"); empty for commands and prompts.
    pub key: String,
}

/// The agent asks before running something (approval mode manual/smart).
#[derive(Clone, Debug)]
pub struct ApprovalReq {
    pub session: String,
    pub prompt_id: String,
    pub command: String,
    pub description: String,
    /// Offered answers: "once", "session", "always", "deny".
    pub choices: Vec<String>,
}

pub const APPROVAL_MODES: &[&str] = &["off", "smart", "manual"];

/// The agent asks the user a question (the `clarify` tool): pick a choice or answer freely.
#[derive(Clone, Debug)]
pub struct ClarifyReq {
    pub session: String,
    pub prompt_id: String,
    pub generation: Option<i64>,
    pub question: String,
    pub choices: Vec<String>,
    /// Several choices may be picked (sent comma-separated).
    pub multi_select: bool,
}

/// One step of the agent's plan (the `todo` tool).
#[derive(Clone, Debug, PartialEq)]
pub struct Todo {
    pub content: String,
    /// "pending", "in_progress", "completed" or "cancelled".
    pub status: String,
    /// Nesting (sub-steps of a step).
    pub depth: usize,
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
