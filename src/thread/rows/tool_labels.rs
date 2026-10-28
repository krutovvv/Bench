//! What a tool call says in one line, as pure functions of its name and arguments: its label
//! ("bash: ls", "reminders · list items"), the word for the run's status, the pages it touched,
//! its arguments as readable text. Tested here; drawn by `work.rs`.

use bench_ui::clip;
use serde_json::Value;

/// JSON as readable text, capped so a huge value cannot stall the UI.
pub fn pretty(v: &Value) -> String {
    let text = match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    if text.chars().count() > 12_000 {
        format!(
            "{}\n… (truncated)",
            text.chars().take(12_000).collect::<String>()
        )
    } else {
        text
    }
}

/// The external pages a tool call touched: http(s) URLs in its arguments. File tools and code
/// carry URLs that are not pages visited, so they are skipped.
pub fn tool_urls(name: &str, args: &Value, out: &mut Vec<String>) {
    let name = name.to_lowercase();
    if ["write", "patch", "edit", "create", "memory", "skill"]
        .iter()
        .any(|w| name.contains(w))
    {
        return;
    }
    const CODE: &[&str] = &[
        "content",
        "code",
        "text",
        "patch",
        "new_string",
        "old_string",
        "script",
        "html",
        "body",
        "expression",
        "function",
        "js",
        "javascript",
        "source",
    ];
    match args {
        Value::Object(m) => m
            .iter()
            .filter(|(k, _)| !CODE.contains(&k.as_str()))
            .for_each(|(_, v)| urls_in(v, out)),
        other => urls_in(other, out),
    }
}

fn urls_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            for word in s.split(|c: char| c.is_whitespace() || "\"'<>`()[]".contains(c)) {
                let Some(start) = word.find("http") else {
                    continue;
                };
                let url = word[start..].trim_end_matches(['.', ',', ';', ':', '!', '?']);
                if (url.starts_with("http://") || url.starts_with("https://"))
                    && !is_local(&host_of(url))
                    && !url.contains("w3.org/")
                    && !url.contains('{')
                    && !out.iter().any(|u| u == url)
                {
                    out.push(url.to_string());
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| urls_in(x, out)),
        Value::Object(m) => m.values().for_each(|x| urls_in(x, out)),
        _ => {}
    }
}

pub fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    host.trim_start_matches("www.").to_lowercase()
}

fn is_local(host: &str) -> bool {
    host.is_empty()
        || host == "localhost"
        || host.ends_with(".local")
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("0.0.0.0")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (16..=31).contains(&n)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn pages_a_tool_touched() {
        let mut out = vec![];
        super::tool_urls(
            "web_fetch",
            &json!({"url": "https://www.example.com/a", "code": "https://x.dev"}),
            &mut out,
        );
        super::tool_urls(
            "bash",
            &json!({"command": "curl http://localhost:3000 https://docs.rs/gpui."}),
            &mut out,
        );
        super::tool_urls("edit", &json!({"path": "https://nope.io"}), &mut out);
        assert_eq!(out, ["https://www.example.com/a", "https://docs.rs/gpui"]);
        assert_eq!(
            super::host_of("https://www.example.com:8080/a"),
            "example.com"
        );
    }
}

///  ("session search"; an MCP tool
/// `mcp__<server>__<tool>` as "<server> · <tool>"), then what it worked on.
pub fn tool_label(name: &str, detail: &str) -> String {
    let name = match name.strip_prefix("mcp__").and_then(|n| n.split_once("__")) {
        Some((server, tool)) => format!("{server} · {}", tool.replace('_', " ")),
        None => name.replace('_', " "),
    };
    // One line: a command with a heredoc, a patch, a quoted page — the running line is one row
    // tall whatever the tool was handed, or the thread jumps in height as the work changes.
    let detail = clip_line(detail);
    if detail.is_empty() {
        name
    } else {
        format!("{name}: {detail}")
    }
}

/// The one word for what a kind of tool is doing, for the live line: a person reads words
/// faster than command lines, and the opened step list still holds the exact command.
pub(crate) fn activity_word(name: &str, preview: &str) -> String {
    // A file's own name, not its path: the line is about the thing, not where it lives.
    let target = || {
        preview
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim())
            .map(|l| {
                std::path::Path::new(l)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| clip(l, 40))
            })
            .unwrap_or_default()
    };
    let with = |verb: &str| {
        let t = target();
        if t.is_empty() {
            format!("{verb}…")
        } else {
            format!("{verb} {t}…")
        }
    };
    if name.starts_with("mcp__browser") {
        return "browsing…".to_string();
    }
    match name {
        "bash" | "ssh_run" => "running a command…".to_string(),
        "read" => with("reading"),
        "edit" | "write" | "search_replace" => with("editing"),
        "grep" | "glob" | "ls" | "session_search" | "tool_search" => "searching…".to_string(),
        "web_fetch" | "web_search" => "checking the web…".to_string(),
        "delegate" => "delegating…".to_string(),
        "todo" => "planning…".to_string(),
        "memory" => "taking notes…".to_string(),
        "cron" => "the schedule…".to_string(),
        "clarify" => "asking you…".to_string(),
        "skill_view" | "skill_manage" => "a skill…".to_string(),
        _ if name.starts_with("mcp__") => {
            let tool = name.rsplit("__").next().unwrap_or(name).replace('_', " ");
            format!("using {tool}…")
        }
        _ => format!("using {}…", name.replace('_', " ")),
    }
}

/// One line of at most 160 characters.
fn clip_line(s: &str) -> String {
    let line = s
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .trim();
    if line.chars().count() > 160 {
        format!("{}…", line.chars().take(159).collect::<String>())
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tool_label_tests {
    #[test]
    fn labels() {
        assert_eq!(super::tool_label("bash", "ls"), "bash: ls");
        assert_eq!(super::tool_label("session_search", ""), "session search");
        assert_eq!(
            super::tool_label("bash", "python3 - <<'PY'\nimport re\np='src/app.rs'"),
            "bash: python3 - <<'PY'",
            "a multiline command stays one line"
        );
        assert_eq!(super::tool_label("bash", "\n\n   \n"), "bash");
        assert_eq!(
            super::tool_label("mcp__reminders__list_items", "Today"),
            "reminders · list items: Today"
        );
    }
}
