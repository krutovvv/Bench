//! Stored session history -> turns with folded steps.

use serde_json::Value;

use crate::model::{FileEdit, Step, Turn};

/// Model-facing "user" rows that are not the user's words.
const SYSTEM_PREFIXES: &[&str] = &[
    "[STILL IN PROGRESS",
    "[CONTEXT COMPACTION",
    "[CONTEXT SUMMARY]",
    "[PRIOR CONTEXT",
    "[Skills pruned during compression",
    "[ASYNC DELEGATION",
    "[IMPORTANT: Background",
    "[SYSTEM NOTICE",
    "[INTERNAL NOTIFICATION",
    "[USER INITIATED SKILLS RELOAD",
    "[System:",
];

pub fn is_system_row(text: &str) -> bool {
    let t = text.trim_start();
    SYSTEM_PREFIXES.iter().any(|p| t.starts_with(p))
}

pub fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

/// A short, human preview of a tool result or arguments.
pub fn preview(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            for key in [
                "summary",
                "title",
                "url",
                "query",
                "command",
                "path",
                "name",
                "description",
                "message",
                "output",
                "error",
            ] {
                if let Some(s) = map
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    return clip(s, 140);
                }
            }
            String::new()
        }
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(inner @ Value::Object(_)) => preview(&inner),
            _ => clip(
                s.lines().find(|l| !l.trim().is_empty()).unwrap_or_default(),
                140,
            ),
        },
        _ => String::new(),
    }
}

pub fn failed(v: &Value) -> bool {
    let obj = match v {
        Value::String(s) => serde_json::from_str::<Value>(s).unwrap_or(Value::Null),
        other => other.clone(),
    };
    obj.get("success") == Some(&Value::Bool(false))
        || obj.get("error").is_some_and(|e| !e.is_null() && e != "")
        || v.as_str()
            .is_some_and(|s| s.trim_start().starts_with("Error"))
}

/// Text the model wrote before a tool call was narration, not the answer.
pub fn demote_answer(turn: &mut Turn) {
    let text = std::mem::take(&mut turn.answer);
    if !text.trim().is_empty() {
        turn.steps.push(Step::Note(text));
    }
}

pub fn turns(messages: &[Value]) -> Vec<Turn> {
    let mut out: Vec<Turn> = Vec::new();
    for m in messages {
        // Newer gateways send the stored row (`content`); older ones a display projection (`text`).
        let mut text = text_of(&m["content"]);
        if text.is_empty() {
            text = m["text"].as_str().unwrap_or_default().to_string();
        }
        match m["role"].as_str() {
            Some("user") => {
                if is_system_row(&text) {
                    continue;
                }
                out.push(Turn {
                    user: text,
                    ..Default::default()
                });
            }
            Some("assistant") => {
                let Some(turn) = out.last_mut() else { continue };
                // A new assistant message means the previous text was not the last word.
                demote_answer(turn);
                let reasoning = m["reasoning"]
                    .as_str()
                    .or(m["reasoning_content"].as_str())
                    .unwrap_or_default();
                if !reasoning.trim().is_empty() {
                    turn.steps.push(Step::Thought(reasoning.trim().to_string()));
                }
                turn.answer = text;
            }
            Some("tool") => {
                let Some(turn) = out.last_mut() else { continue };
                demote_answer(turn);
                let name = m["tool_name"]
                    .as_str()
                    .or(m["name"].as_str())
                    .unwrap_or("tool")
                    .to_string();
                if let Some(edit) = file_edit(&name, &m["content"]) {
                    turn.files.push(edit);
                }
                turn.steps.push(Step::Tool {
                    detail: m["context"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| preview(&m["content"])),
                    failed: failed(&m["content"]),
                    call_id: m["tool_call_id"].as_str().unwrap_or_default().to_string(),
                    args: pretty(&m["args"]),
                    output: pretty(&m["content"]),
                    name,
                });
            }
            _ => {}
        }
    }
    out
}

/// JSON (or a JSON string) as readable text, capped so a huge output cannot stall the UI.
pub fn pretty(v: &Value) -> String {
    let text = match v {
        Value::Null => String::new(),
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(inner @ (Value::Object(_) | Value::Array(_))) => {
                serde_json::to_string_pretty(&inner).unwrap_or_default()
            }
            _ => s.clone(),
        },
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

/// A file edit from a file tool's result: its path and, for `patch`, the unified diff.
pub fn file_edit(name: &str, result: &Value) -> Option<FileEdit> {
    if !matches!(
        name,
        "patch" | "write_file" | "edit_file" | "apply_patch" | "str_replace" | "create_file"
    ) {
        return None;
    }
    let r = match result {
        Value::String(s) => serde_json::from_str::<Value>(s).ok()?,
        other => other.clone(),
    };
    if r.get("error").is_some_and(|e| !e.is_null()) || r.get("success") == Some(&Value::Bool(false))
    {
        return None;
    }
    let path = r["resolved_path"]
        .as_str()
        .or_else(|| r["files_modified"].get(0).and_then(Value::as_str))
        .or(r["path"].as_str())?
        .to_string();
    let diff = r["diff"]
        .as_str()
        .filter(|d| !d.is_empty())
        .map(str::to_string);
    Some(FileEdit { path, diff })
}

// ---- sources & timing ------------------------------------------------------

/// Sites a tool call visited: URLs in its arguments, except file contents, patches and code
/// (an HTML file being written is full of schema URLs that were never opened).
pub fn tool_urls(name: &str, args: &Value, out: &mut Vec<String>) {
    let name = name.to_lowercase();
    if ["write", "patch", "edit", "create", "memory", "skill"]
        .iter()
        .any(|w| name.contains(w))
    {
        return;
    }
    let args = match args {
        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| args.clone()),
        other => other.clone(),
    };
    match &args {
        Value::Object(m) => {
            for (k, v) in m {
                if ![
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
                ]
                .contains(&k.as_str())
                {
                    urls_in(v, out);
                }
            }
        }
        other => urls_in(other, out),
    }
}

/// External http(s) URLs anywhere in a JSON value; local-network ones are skipped.
pub fn urls_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            for word in s.split(|c: char| c.is_whitespace() || "\"'<>`()[]".contains(c)) {
                if let Some(start) = word.find("http") {
                    let url = word[start..].trim_end_matches(['.', ',', ';', ':', '!', '?']);
                    if (url.starts_with("http://") || url.starts_with("https://"))
                        && !is_local(&host_of(url))
                        && !url.contains("w3.org/")
                        && !url.contains("${")
                        && !url.contains('{')
                        && !out.iter().any(|u| u == url)
                    {
                        out.push(url.to_string());
                    }
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

/// Per-turn extras from the `session.history` projection (tool args, timestamps).
pub struct Meta {
    /// tool_call_id -> arguments (the stored rows do not keep them; the projection does).
    pub args: std::collections::HashMap<String, String>,
    pub sources: Vec<String>,
    pub started: Option<f64>,
    pub finished: Option<f64>,
}

pub fn metas(messages: &[Value]) -> Vec<Meta> {
    let mut out: Vec<Meta> = vec![];
    for m in messages {
        let ts = m["timestamp"].as_f64();
        let text = m["text"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| text_of(&m["content"]));
        if m["role"] == "user" {
            if !is_system_row(&text) {
                out.push(Meta {
                    args: Default::default(),
                    sources: vec![],
                    started: ts,
                    finished: ts,
                });
            }
            continue;
        }
        let Some(meta) = out.last_mut() else { continue };
        if ts.is_some_and(|t| meta.finished.is_none_or(|f| t > f)) {
            meta.finished = ts;
        }
        if m["role"] == "tool" {
            if let Some(id) = m["tool_call_id"].as_str() {
                meta.args.insert(id.to_string(), pretty(&m["args"]));
            }
            let name = m["name"]
                .as_str()
                .or(m["tool_name"].as_str())
                .unwrap_or_default();
            tool_urls(name, &m["args"], &mut meta.sources);
        }
    }
    out
}

/// Pair metas with turns from the end (both skip the same system rows).
pub fn attach(turns: &mut [Turn], metas: Vec<Meta>) {
    for (turn, meta) in turns.iter_mut().rev().zip(metas.into_iter().rev()) {
        for step in turn.steps.iter_mut() {
            if let Step::Tool { call_id, args, .. } = step {
                if args.is_empty() {
                    if let Some(a) = meta.args.get(call_id.as_str()) {
                        *args = a.clone();
                    }
                }
            }
        }
        turn.sources = meta.sources;
        turn.started = meta.started;
        turn.finished = meta.finished;
    }
}
