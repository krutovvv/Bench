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
    pub model: String,
    pub running: bool,
    /// Where the session lives: "local" (desktop/CLI), "telegram", "cron", …
    pub source: String,
}

/// Human name of a session source, for grouping the session list.
pub fn source_label(source: &str) -> String {
    match source {
        "" | "local" | "desktop" | "tui" | "cli" | "dashboard" => "Hermes".into(),
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

/// One unit of agent work inside a turn.
#[derive(Clone, Debug)]
pub enum Step {
    /// A tool call: name and a short human preview of what it acted on.
    Tool {
        name: String,
        detail: String,
        failed: bool,
        call_id: String,
        /// Full arguments and output, shown when the step is unfolded.
        args: String,
        output: String,
    },
    /// Interim prose the model wrote between tool calls.
    Note(String),
    /// Model reasoning.
    Thought(String),
    /// A subagent the model delegated to: its goal, what it did, how it ended.
    Subagent {
        id: String,
        goal: String,
        /// "running", "completed", "failed", …
        status: String,
        /// Its tool calls and notes, one line each.
        lines: Vec<String>,
        summary: String,
    },
}

/// A slash command from the gateway's catalog.
#[derive(Clone, Debug)]
pub struct SlashCommand {
    /// "/goal".
    pub name: String,
    pub desc: String,
    pub category: String,
}

/// The agent asks before running something (approval mode manual/smart).
#[derive(Clone, Debug)]
pub struct ApprovalReq {
    pub session: String,
    pub prompt_id: String,
    pub generation: Option<i64>,
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

/// One exchange: the user's words, the steps the agent took, the answer.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    pub user: String,
    /// Hermes' background review after the turn ("saved skill X, patched Y"), as it reported.
    pub reviews: Vec<String>,
    pub steps: Vec<Step>,
    pub answer: String,
    pub expanded: bool,
    /// Files the agent edited in this turn.
    pub files: Vec<FileEdit>,
    /// External pages the agent's tool calls touched, in order, deduplicated.
    pub sources: Vec<String>,
    /// Unix seconds: when the user sent it, when the last message of the turn landed.
    pub started: Option<f64>,
    pub finished: Option<f64>,
}

impl Turn {
    pub fn tool_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { .. }))
            .count()
    }
}

/// One file edit by a tool: the file, and the unified diff when the tool reported one
/// (`patch` does; `write_file` rewrites the whole file and reports none).
#[derive(Clone, Debug)]
pub struct FileEdit {
    pub path: String,
    pub diff: Option<String>,
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
    /// Hermes' own average: output tokens over whole-call time (prefill included).
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
    pub description: String,
    pub enabled: bool,
}

pub const EFFORTS: &[&str] = &["low", "medium", "xhigh"];

/// Where a session works: its folder and, when that folder is in a git repo, the project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    pub cwd: String,
    /// The repo's top folder as Hermes recorded it (for a branch folder, that folder itself).
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

/// One machine's Hermes install, for the Gateways settings tab.
#[derive(Clone, Debug, Default)]
pub struct Install {
    /// "v0.22.3 (2026.9.24)".
    pub version: String,
    /// "stable" / "canary".
    pub channel: String,
    /// A newer release, when `hermes update --check` found one: "v0.22.4".
    pub available: Option<String>,
    pub checked: bool,
    /// Working on it: "Checking…", "Updating…".
    pub busy: Option<String>,
    pub error: Option<String>,
}

impl Install {
    /// From `hermes --version`: "Hermes Agent v0.22.3 (2026.9.24) · stable · local 5b147ab036d6"
    /// (a source checkout says "· upstream 10c6188d" instead of a channel).
    pub fn parse_version(&mut self, text: &str) {
        let line = text
            .lines()
            .find(|l| l.contains("Hermes Agent"))
            .unwrap_or_default();
        let parts: Vec<&str> = line.split('·').map(str::trim).collect();
        self.version = parts
            .first()
            .map(|p| p.trim_start_matches("Hermes Agent").trim().to_string())
            .unwrap_or_default();
        if let Some(ch) = parts.get(1).filter(|p| !p.contains(' ')) {
            self.channel = ch.to_string();
        }
    }

    /// From `hermes update --check`: "Update channel: stable", then either
    /// "Selected release available: stable v0.22.4 (…)" or
    /// "Update available: 7587 commits behind origin/main." or nothing new.
    pub fn parse_check(&mut self, text: &str) {
        self.checked = true;
        if let Some(ch) = text.lines().find_map(|l| l.split_once("Update channel:")) {
            self.channel = ch.1.trim().to_string();
        }
        self.available = text.lines().find_map(|l| {
            let (_, rest) = l.split_once("available:")?;
            let rest = rest.trim().trim_end_matches('.');
            Some(
                rest.split_whitespace()
                    .find(|w| {
                        w.starts_with('v') && w[1..].starts_with(|c: char| c.is_ascii_digit())
                    })
                    .map(str::to_string)
                    .unwrap_or_else(|| rest.to_string()),
            )
        });
    }
}

#[cfg(test)]
mod install_tests {
    use super::Install;

    #[test]
    fn parses_version_and_check() {
        let mut i = Install::default();
        i.parse_version(
            "Hermes Agent v0.22.3 (2026.9.24) · stable · local 5b147ab036d6\nInstall method: git",
        );
        assert_eq!(
            (i.version.as_str(), i.channel.as_str()),
            ("v0.22.3 (2026.9.24)", "stable")
        );
        i.parse_check("→ Update channel: stable\n→ Selected release available: stable v0.22.4 (4d04303818b6)\n");
        assert_eq!(i.available.as_deref(), Some("v0.22.4"));
        i.parse_check("→ Already up to date");
        assert_eq!(i.available, None);
        let mut s = Install::default();
        s.parse_version("Hermes Agent v0.21.5+8459.g2c542f7 (2026.9.24) · upstream 2c542f79");
        s.parse_check("Update available — run 'hermes update'\n→ Update channel: main\n⚕ Update available: 7587 commits behind origin/main.");
        assert_eq!(s.channel, "main");
        assert_eq!(
            s.available.as_deref(),
            Some("7587 commits behind origin/main")
        );
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
