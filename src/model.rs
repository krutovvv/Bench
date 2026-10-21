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
            Target::Local => "Этот Mac".into(),
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
    },
    /// Interim prose the model wrote between tool calls.
    Note(String),
    /// Model reasoning.
    Thought(String),
}

/// One exchange: the user's words, the steps the agent took, the answer.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    pub user: String,
    pub steps: Vec<Step>,
    pub answer: String,
    pub expanded: bool,
}

impl Turn {
    pub fn tool_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { .. }))
            .count()
    }
}

/// Context/usage readout for the status bar.
#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub model: String,
    pub context_used: u64,
    pub context_max: u64,
    pub context_percent: f64,
    pub total_tokens: u64,
    pub compressions: u64,
    pub cache_hit_pct: Option<u64>,
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
            context_percent: v["context_percent"].as_f64().unwrap_or(0.0),
            total_tokens: n("total"),
            compressions: n("compressions"),
            cache_hit_pct: v["cache_hit_pct"].as_u64(),
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
