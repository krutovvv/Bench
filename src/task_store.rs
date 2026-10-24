//! Where the Tasks mode keeps its tasks: the agent's `bench.sqlite`, through its REST API
//! (`/api/tasks`), so the agent's `tasks` tool sees the same list. The same commands and events
//! as the old Apple Reminders bridge (`crate::reminders::{Cmd, Evt}`).
//!
//! The first time, the tasks move from Reminders (macOS only): all of them, open and done, are
//! read, written to `reminders-backup.json` in Bench's state folder, imported (each carries its
//! reminder's id, so a repeat never doubles), checked to be all there, and only then removed
//! from Reminders. `reminders-moved` marks it done.

use std::sync::Arc;
use std::sync::mpsc;

use anyhow::{Result, anyhow};
use chrono::NaiveDate;
use serde_json::{Value, json};

use crate::connection::{Connection, state_dir};
use crate::reminders::{Cmd, Evt};
use crate::tasks::Task;

pub struct TaskStore {
    tx: mpsc::Sender<Cmd>,
}

impl TaskStore {
    pub fn start(conn: Arc<Connection>, profile: String, evt: async_channel::Sender<Evt>) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::spawn(move || {
            if let Err(e) = move_from_reminders(&conn, &profile) {
                let _ =
                    evt.try_send(Evt::Error(format!("Moving tasks from Reminders: {e:#}")));
            }
            for cmd in rx {
                let r = match cmd {
                    Cmd::Load { done_since } => load(&conn, &profile, done_since).map(Evt::Loaded).map(Some),
                    Cmd::Save { task, token } => save(&conn, &profile, &task)
                        .map(|id| Some(Evt::Created { token, id })),
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
            .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
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
        crate::rest::send_json(conn, "PUT", &format!("/api/tasks/{}", t.id), profile, to_json(t))?
    };
    v["task"]["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("the agent returned no task id"))
}

#[cfg(target_os = "macos")]
fn move_from_reminders(conn: &Connection, profile: &str) -> Result<()> {
    let marker = state_dir().join("reminders-moved");
    if marker.exists() {
        return Ok(());
    }
    let store = crate::reminders::open()?;
    let all = crate::reminders::load_all(&store)?;
    std::fs::create_dir_all(state_dir())?;
    let backup: Vec<Value> = all
        .iter()
        .map(|t| {
            let mut v = to_json(t);
            v["source_id"] = json!(format!("reminder:{}", t.id));
            v
        })
        .collect();
    std::fs::write(
        state_dir().join("reminders-backup.json"),
        serde_json::to_string_pretty(&backup)?,
    )?;
    crate::rest::send_json(
        conn,
        "POST",
        "/api/tasks/import",
        profile,
        json!({ "tasks": backup }),
    )?;
    // Every reminder must be in the agent's store before any is removed.
    let have: std::collections::HashSet<String> = crate::rest::get_raw(conn, profile, "/api/tasks?done_since=1970-01-01")?["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["source_id"].as_str().map(str::to_string))
        .collect();
    let missing = all
        .iter()
        .filter(|t| !have.contains(&format!("reminder:{}", t.id)))
        .count();
    if missing > 0 {
        return Err(anyhow!("{missing} tasks did not arrive; Reminders left as they are"));
    }
    let mut removed = 0;
    for t in &all {
        if crate::reminders::remove(&store, &t.id).is_ok() {
            removed += 1;
        }
    }
    std::fs::write(&marker, format!("{} moved, {removed} removed from Reminders\n", all.len()))?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn move_from_reminders(_: &Connection, _: &str) -> Result<()> {
    Ok(())
}
