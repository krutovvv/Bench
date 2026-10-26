//! Where tasks live: the agent's `bench.sqlite`, through its REST API (`/api/tasks`), so the
//! agent's `tasks` tool sees the same list. Calls run on a thread of their own.

use std::sync::Arc;
use std::sync::mpsc;

use anyhow::{Result, anyhow};
use chrono::NaiveDate;
use serde_json::{Value, json};

use crate::connection::Connection;
use crate::tasks::Task;

/// What the window asks of the store.
pub enum Cmd {
    /// All open tasks and those completed since that day.
    Load {
        done_since: NaiveDate,
    },
    /// Create (no id) or change a task; `token` comes back with a new task's id.
    Save {
        task: Task,
        token: u64,
    },
    Delete(String),
}

pub enum Evt {
    Loaded(Vec<Task>),
    Created { token: u64, id: String },
    Error(String),
}

pub struct TaskStore {
    tx: mpsc::Sender<Cmd>,
}

impl TaskStore {
    pub fn start(conn: Arc<Connection>, profile: String, evt: async_channel::Sender<Evt>) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::spawn(move || {
            for cmd in rx {
                let r = match cmd {
                    Cmd::Load { done_since } => {
                        load(&conn, &profile, done_since).map(Evt::Loaded).map(Some)
                    }
                    Cmd::Save { task, token } => {
                        save(&conn, &profile, &task).map(|id| Some(Evt::Created { token, id }))
                    }
                    Cmd::Delete(id) => crate::rest::send_json(
                        &conn,
                        "DELETE",
                        &format!("/api/tasks/{id}"),
                        &profile,
                        Value::Null,
                    )
                    .map(|_| None),
                };
                let e = match r {
                    Ok(Some(e)) => e,
                    Ok(None) => continue,
                    Err(e) => Evt::Error(format!("{e:#}")),
                };
                if evt.try_send(e).is_err() {
                    break;
                }
            }
        });
        Self { tx }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

fn day(v: &Value) -> Option<NaiveDate> {
    v.as_str()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

fn from_json(v: &Value) -> Task {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    Task {
        id: s("id"),
        title: s("title"),
        notes: s("notes"),
        tags: v["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        day: day(&v["day"]),
        evening: v["evening"].as_bool().unwrap_or(false),
        someday: v["someday"].as_bool().unwrap_or(false),
        important: v["important"].as_bool().unwrap_or(false),
        completed: day(&v["completed"]),
        spent: v["spent"].as_i64().unwrap_or(0),
        timer: v["timer"].as_i64(),
        created: v["created"].as_i64().unwrap_or(0),
        project: s("project"),
    }
}

fn to_json(t: &Task) -> Value {
    let d = |d: Option<NaiveDate>| d.map(|d| d.format("%Y-%m-%d").to_string());
    json!({
        "title": t.title, "notes": t.notes, "tags": t.tags, "day": d(t.day),
        "evening": t.evening, "someday": t.someday, "important": t.important,
        "completed": d(t.completed), "spent": t.spent, "timer": t.timer, "created": t.created,
        "project": t.project,
    })
}

fn load(conn: &Connection, profile: &str, done_since: NaiveDate) -> Result<Vec<Task>> {
    let v = crate::rest::get_raw(
        conn,
        profile,
        &format!("/api/tasks?done_since={}", done_since.format("%Y-%m-%d")),
    )?;
    let mut out: Vec<Task> = v["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(from_json)
        .collect();
    out.sort_by_key(|t| t.created);
    Ok(out)
}

fn save(conn: &Connection, profile: &str, t: &Task) -> Result<String> {
    let v = if t.id.is_empty() {
        crate::rest::send_json(conn, "POST", "/api/tasks", profile, to_json(t))?
    } else {
        crate::rest::send_json(
            conn,
            "PUT",
            &format!("/api/tasks/{}", t.id),
            profile,
            to_json(t),
        )?
    };
    v["task"]["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("the agent returned no task id"))
}
