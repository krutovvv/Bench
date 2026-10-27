//! What the agent sends back — answers to calls and events — read into `Evt`s.

use super::Evt;
use super::session::Pending;
use crate::app::model::{Profile, SessionRow, Usage};
use serde_json::Value;

pub(super) fn on_result(kind: Pending, r: &Value, emit: &impl Fn(Evt)) {
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    match kind {
        Pending::Profiles => emit(Evt::Profiles(
            list(&r["profiles"])
                .iter()
                .map(|p| {
                    let name = p["name"].as_str().unwrap_or_default().to_string();
                    let label = p["display_name"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(&name)
                        .to_string();
                    let model = match &p["model"] {
                        Value::String(s) => s.clone(),
                        m => m["default"]
                            .as_str()
                            .or(m["name"].as_str())
                            .unwrap_or_default()
                            .to_string(),
                    };
                    Profile { name, label, model }
                })
                .collect(),
        )),
        Pending::Sessions(p) => emit(Evt::Sessions(
            p,
            list(&r["sessions"])
                .iter()
                .map(|s| SessionRow {
                    id: s["id"].as_str().unwrap_or_default().into(),
                    title: s["title"]
                        .as_str()
                        .filter(|t| !t.trim().is_empty())
                        .unwrap_or("Untitled")
                        .into(),
                    messages: s["message_count"].as_u64().unwrap_or(0),
                    running: s["running"].as_bool().unwrap_or(false),
                    source: s["source"].as_str().unwrap_or_default().into(),
                    subject: s["subject"].as_str().unwrap_or_default().into(),
                    project: s["project"].as_str().unwrap_or_default().into(),
                    host: String::new(),
                })
                .collect(),
        )),
        Pending::Resume(..) => {}
        Pending::Thread {
            session,
            runtime,
            replay,
        } => {
            let open = serde_json::from_value(r.clone()).unwrap_or_default();
            emit(Evt::History {
                session,
                runtime,
                open,
            });
            for frame in &replay {
                on_event(frame, emit);
            }
        }
        Pending::Create => {
            let runtime = r["session_id"].as_str().unwrap_or_default().to_string();
            let session = r["stored_session_id"]
                .as_str()
                .unwrap_or(&runtime)
                .to_string();
            emit(Evt::Created { session, runtime })
        }
        Pending::Inserts(session) => emit(Evt::Inserts(session, inserts(&r["inserts"]))),
        Pending::Delete => emit(Evt::Deleted),
        Pending::Catalog => emit(Evt::Catalog(catalog(r))),
        Pending::Processes(session) => emit(Evt::Processes(session, processes(r))),
        Pending::Dispatch(session) => {
            let s = |k: &str| {
                r[k].as_str()
                    .map(str::to_string)
                    .filter(|v| !v.trim().is_empty())
            };
            match r["type"].as_str().unwrap_or_default() {
                "send" | "skill" | "prompt" => emit(Evt::Dispatched {
                    session,
                    output: None,
                    send: s("message"),
                }),
                _ => emit(Evt::Dispatched {
                    session,
                    output: s("output").or_else(|| s("message")).or_else(|| s("text")),
                    send: None,
                }),
            }
        }
        Pending::Ignore => {}
    }
}

fn inserts(v: &Value) -> Vec<crate::app::model::Insert> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(crate::app::model::Insert::from_json)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn on_event(p: &Value, emit: &impl Fn(Evt)) {
    let sid = p["session_id"].as_str().unwrap_or_default().to_string();
    let payload = &p["payload"];
    let text = || payload["text"].as_str().unwrap_or_default().to_string();
    match p["type"].as_str().unwrap_or_default() {
        "message.start" => emit(Evt::Started(sid)),
        // The session's current settings: on resume, after a change, at turn boundaries.
        "session.info" => {
            if let Some(effort) = payload["reasoning_effort"].as_str() {
                emit(Evt::Effort(sid.clone(), effort.to_string()));
            }
            if let Some(f) = payload.get("fallback") {
                let note = f["model"].as_str().map(|m| {
                    (
                        m.to_string(),
                        f["reason"].as_str().unwrap_or_default().to_string(),
                    )
                });
                emit(Evt::Fallback(sid, note));
            }
        }
        "message.delta" => emit(Evt::Delta(sid, text())),
        "reasoning.delta" => emit(Evt::Reasoning(sid, text())),
        "tool.start" => emit(Evt::ToolStart {
            name: payload["name"].as_str().unwrap_or("tool").to_string(),
            detail: payload["context"].as_str().unwrap_or_default().to_string(),
            session: sid,
        }),
        "tool.complete" => {
            if payload["todos"].is_array() {
                emit(Evt::Todos(sid.clone(), todos(payload), todo_at(payload)));
            }
            emit(Evt::ToolDone { session: sid })
        }
        kind if kind.starts_with("thread.") => {
            if let Some(e) = protocol::ThreadEvent::parse(kind, payload) {
                emit(Evt::Thread(sid, e))
            }
        }
        "approval.request" => emit(Evt::Approval(crate::app::model::ApprovalReq {
            prompt_id: payload["prompt_id"]
                .as_str()
                .or(payload["request_id"].as_str())
                .unwrap_or_default()
                .to_string(),
            command: payload["command"].as_str().unwrap_or_default().to_string(),
            description: payload["description"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            choices: payload["choices"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_else(|| {
                    let mut c = vec!["once".to_string()];
                    if payload["allow_session"].as_bool() != Some(false) {
                        c.push("session".into());
                    }
                    if payload["allow_permanent"].as_bool() != Some(false) {
                        c.push("always".into());
                    }
                    c.push("deny".into());
                    c
                }),
            session: sid,
        })),
        "approval.settled" | "approval.cancelled" | "approval.received" => {
            emit(Evt::ApprovalDone(sid))
        }
        "clarify.request" => {
            let strings = |v: &Value| -> Vec<String> {
                v.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect()
            };
            emit(Evt::Clarify(crate::app::model::ClarifyReq {
                prompt_id: payload["prompt_id"]
                    .as_str()
                    .or(payload["request_id"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                generation: payload["execution_generation"].as_i64(),
                question: payload["question"].as_str().unwrap_or_default().to_string(),
                choices: strings(&payload["choices"]),
                multi_select: payload["multi_select"].as_bool().unwrap_or(false),
                session: sid,
            }))
        }
        "clarify.settled" | "clarify.cancelled" => emit(Evt::ClarifyDone(sid)),
        "todo.updated" => emit(Evt::Todos(sid, todos(payload), todo_at(payload))),
        "sessions.changed" => emit(Evt::SessionsChanged),
        "session.inserts" => emit(Evt::Inserts(sid, inserts(&payload["inserts"]))),
        "session.usage" => emit(Evt::Usage(sid, Usage::from_json(&payload["usage"]))),
        "session.speed" => emit(Evt::Speed(
            sid,
            crate::statusbar::speed::Stat::from_json(&payload["speed"]),
        )),
        "message.complete" => {
            if payload["usage"].is_object() {
                emit(Evt::Usage(sid.clone(), Usage::from_json(&payload["usage"])));
            }
            emit(Evt::Complete {
                session: sid,
                text: text(),
            })
        }
        _ => {}
    }
}

/// `commands.catalog` → one list: each category's commands, then skills (pairs not in a category).
fn catalog(r: &Value) -> Vec<crate::app::model::SlashCommand> {
    let pair = |p: &Value, category: &str| {
        let name = p[0].as_str()?.trim().to_string();
        let name = if name.starts_with('/') {
            name
        } else {
            format!("/{name}")
        };
        Some(crate::app::model::SlashCommand {
            name,
            desc: p[1].as_str().unwrap_or_default().trim().to_string(),
            category: category.to_string(),
            key: String::new(),
        })
    };
    let mut out: Vec<crate::app::model::SlashCommand> = vec![];
    for c in r["categories"].as_array().into_iter().flatten() {
        let cat = c["name"].as_str().unwrap_or("Commands");
        out.extend(
            c["pairs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| pair(p, cat)),
        );
    }
    for p in r["pairs"].as_array().into_iter().flatten() {
        if let Some(cmd) = pair(p, "Skills") {
            if !out.iter().any(|c| c.name == cmd.name) {
                out.push(cmd);
            }
        }
    }
    out
}

/// RPC errors go to `gateway-errors.log` too (a Finder-launched app has no visible stderr).
pub(super) fn log_error(method: &str, err: &str) {
    eprintln!("[gateway] {method}: {err}");
    let path = crate::client::connection::state_dir().join("gateway-errors.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write as _;
        let _ = writeln!(f, "{} {method}: {err}", crate::app::now_secs() as u64);
    }
}

/// The plan steps of a `todo.updated` / todo `tool.complete` payload, sub-steps indented.
/// When this plan began, as the agent dated it.
fn todo_at(payload: &Value) -> f64 {
    payload["at"].as_f64().unwrap_or(0.)
}

fn todos(payload: &Value) -> Vec<crate::app::model::Todo> {
    let items = payload["todos"].as_array().cloned().unwrap_or_default();
    let parent_of = |id: &str| {
        items
            .iter()
            .find(|t| t["id"].as_str() == Some(id))
            .and_then(|t| t["parent"].as_str().map(str::to_string))
    };
    items
        .iter()
        .map(|t| {
            let mut depth = 0;
            let mut up = t["parent"].as_str().map(str::to_string);
            while let Some(p) = up.filter(|_| depth < 4) {
                depth += 1;
                up = parent_of(&p);
            }
            crate::app::model::Todo {
                content: t["content"].as_str().unwrap_or_default().to_string(),
                status: t["status"].as_str().unwrap_or("pending").to_string(),
                depth,
            }
        })
        .collect()
}

/// `process.list`'s rows (the process id rides in `session_id`).
fn processes(r: &Value) -> Vec<crate::app::model::BgProcess> {
    r["processes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| crate::app::model::BgProcess {
            id: p["session_id"].as_str().unwrap_or_default().to_string(),
            command: p["command"].as_str().unwrap_or_default().to_string(),
            running: p["status"].as_str() != Some("exited"),
            exit_code: p["exit_code"].as_i64(),
            started: p["started_epoch"].as_f64(),
            exited: p["exited_at"].as_f64(),
            reason: p["completion_reason"].as_str().map(str::to_string),
            tail: p["output_tail"]
                .as_str()
                .or(p["output_preview"].as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .filter(|p| !p.id.is_empty())
        .collect()
}
