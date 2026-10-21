//! Stored session history -> turns with folded steps.

use serde_json::Value;

use crate::model::{Step, Turn};

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
                turn.steps.push(Step::Tool {
                    name: m["tool_name"].as_str().or(m["name"].as_str()).unwrap_or("tool").to_string(),
                    detail: m["context"].as_str().map(str::to_string).unwrap_or_else(|| preview(&m["content"])),
                    failed: failed(&m["content"]),
                });
            }
            _ => {}
        }
    }
    out
}
