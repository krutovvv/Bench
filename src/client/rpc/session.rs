//! The JSON-RPC session itself, on its own thread: each `Cmd` sent as a call over the WebSocket,
//! what comes back handed to `events`.

use super::events::{log_error, on_event, on_result};
use super::{Cmd, Evt};
use crate::app::model::Target;
use crate::client::connection::Connection;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::{Arc, mpsc};
use tungstenite::{Message, WebSocket};

pub(super) enum Pending {
    Profiles,
    Sessions(String),
    /// (stored session id, profile)
    Resume(String, String),
    /// Resume done; `thread.open` brings the thread itself.
    Thread {
        session: String,
        runtime: String,
        /// What was open when the chat was reopened (questions, approvals, the plan), as
        /// event frames to replay once the UI has the session.
        replay: Vec<Value>,
    },
    Create,
    /// An insert's answer: the session's new list.
    Inserts(String),
    Delete,
    Catalog,
    /// `process.list` for this runtime session.
    Processes(String),
    Dispatch(String),
    Ignore,
}

struct Rpc {
    ws: WebSocket<TcpStream>,
    next_id: u64,
    pending: HashMap<u64, Pending>,
    /// Method of each pending call, for error reports.
    methods: HashMap<u64, String>,
    /// runtime session id -> its execution generation: the turn Stop and answers name.
    fences: HashMap<String, i64>,
}

impl Rpc {
    fn call(&mut self, method: &str, params: Value, kind: Pending) -> Result<()> {
        self.next_id += 1;
        self.pending.insert(self.next_id, kind);
        self.methods.insert(self.next_id, method.to_string());
        let frame =
            json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params});
        self.ws.send(Message::text(frame.to_string()))?;
        Ok(())
    }

    /// Remember a session's execution generation from a snapshot-shaped value.
    fn note_fence(&mut self, session: &str, v: &Value) {
        if let Some(g) = v["execution_generation"].as_i64() {
            self.fences.insert(session.to_string(), g);
        }
    }

    /// Returns false on shutdown.
    fn handle(&mut self, cmd: Cmd) -> Result<bool> {
        match cmd {
            Cmd::Shutdown => return Ok(false),
            Cmd::Sessions(p) => self.call(
                "session.list",
                json!({"limit": 80, "profile": p}),
                Pending::Sessions(p),
            )?,
            Cmd::Open { profile, session } => self.call(
                "session.resume",
                json!({"session_id": session, "profile": profile}),
                Pending::Resume(session, profile),
            )?,
            Cmd::Delete {
                profile,
                session,
                runtime,
            } => {
                if let Some(runtime) = runtime {
                    self.call(
                        "session.close",
                        json!({"session_id": runtime}),
                        Pending::Ignore,
                    )?;
                }
                self.call(
                    "session.delete",
                    json!({"session_id": session, "profile": profile}),
                    Pending::Delete,
                )?
            }
            Cmd::Create {
                profile,
                cwd,
                subject,
                project,
            } => {
                let mut params = json!({"profile": profile});
                if !project.is_empty() {
                    params["project"] = json!(project);
                }
                if let Some(cwd) = cwd {
                    params["cwd"] = json!(cwd);
                }
                if !subject.is_empty() {
                    params["subject"] = json!(subject);
                }
                self.call("session.create", params, Pending::Create)?
            }
            Cmd::Queue {
                session,
                text,
                context,
            } => {
                let mut params = json!({"session_id": session, "text": text, "mode": "queue"});
                if !context.is_empty() {
                    params["context"] = json!(context);
                }
                self.call("prompt.submit", params, Pending::Ignore)?
            }
            Cmd::QueueWithdraw { session, id } => self.call(
                "session.queue.withdraw",
                json!({"session_id": session, "id": id}),
                Pending::Ignore,
            )?,
            Cmd::QueueSend { session, id } => self.call(
                "session.queue.send",
                json!({"session_id": session, "id": id}),
                Pending::Ignore,
            )?,
            Cmd::QueueEdit { session, id, text } => self.call(
                "session.queue.edit",
                json!({"session_id": session, "id": id, "text": text}),
                Pending::Ignore,
            )?,
            Cmd::Insert {
                session,
                id,
                key,
                state,
            } => self.call(
                "session.insert",
                json!({"session_id": session, "id": id, "key": key, "state": state}),
                Pending::Inserts(session.clone()),
            )?,
            Cmd::PatchInsert { session, id, patch } => self.call(
                "session.insert.patch",
                json!({"session_id": session, "id": id, "patch": patch}),
                Pending::Inserts(session.clone()),
            )?,
            Cmd::SetEffort { session, effort } => self.call(
                "config.set",
                json!({"key": "reasoning", "value": effort, "session_id": session}),
                Pending::Ignore,
            )?,
            Cmd::Rename { session, title } => self.call(
                "session.title",
                json!({"session_id": session, "title": title}),
                Pending::Ignore,
            )?,
            Cmd::Catalog { profile } => self.call(
                "commands.catalog",
                json!({"profile": profile}),
                Pending::Catalog,
            )?,
            Cmd::Processes { profile, session } => self.call(
                "process.list",
                json!({"session_id": session, "profile": profile}),
                Pending::Processes(session),
            )?,
            Cmd::KillProcess {
                profile,
                session,
                id,
            } => self.call(
                "process.kill",
                json!({"session_id": session, "process_id": id, "profile": profile}),
                Pending::Ignore,
            )?,
            Cmd::Dispatch {
                profile,
                session,
                name,
                arg,
            } => {
                let command = if arg.is_empty() {
                    name
                } else {
                    format!("{name} {arg}")
                };
                let params = json!({"session_id": session, "profile": profile, "command": command});
                self.call("slash.exec", params, Pending::Dispatch(session))?
            }
            Cmd::Clarify {
                profile,
                req,
                answer,
            } => {
                let generation = req
                    .generation
                    .or_else(|| self.fences.get(&req.session).copied())
                    .unwrap_or(0);
                let params = json!({"session_id": req.session, "execution_generation": generation,
                                    "prompt_id": req.prompt_id, "request_id": req.prompt_id,
                                    "answer": answer, "profile": profile});
                self.call("clarify.respond", params, Pending::Ignore)?
            }
            Cmd::Approve {
                profile,
                req,
                choice,
            } => {
                let params = json!({"session_id": req.session, "request_id": req.prompt_id,
                                    "choice": choice, "profile": profile});
                self.call("approval.respond", params, Pending::Ignore)?
            }
            Cmd::Send {
                profile,
                session,
                text,
                effort,
                context,
            } => {
                if let Some(value) = effort {
                    let params = json!({"key": "reasoning", "value": value, "session_id": session, "profile": profile});
                    self.call("config.set", params, Pending::Ignore)?;
                }
                let mut params = json!({"session_id": session, "text": text, "profile": profile});
                if !context.is_empty() {
                    params["context"] = json!(context);
                }
                self.call("prompt.submit", params, Pending::Ignore)?
            }
            // Stop names the turn it stops: the gateway refuses any other generation.
            Cmd::SaveSpeed { session, speed } => self.call(
                "session.speed",
                json!({"session_id": session, "speed": speed}),
                Pending::Ignore,
            )?,
            Cmd::Interrupt { profile, session } => {
                let generation = self.fences.get(&session).copied().unwrap_or(0);
                self.call(
                    "session.interrupt",
                    json!({"session_id": session, "profile": profile,
                           "execution_generation": generation}),
                    Pending::Ignore,
                )?
            }
            Cmd::Regenerate {
                profile,
                session,
                text,
            } => {
                self.call(
                    "session.undo",
                    json!({"session_id": session, "profile": profile, "intent": "retry"}),
                    Pending::Ignore,
                )?;
                let params = json!({"session_id": session, "text": text, "profile": profile});
                self.call("prompt.submit", params, Pending::Ignore)?
            }
        }
        Ok(true)
    }
}

pub(super) fn run(
    target: &Target,
    cmds: &mpsc::Receiver<Cmd>,
    evts: &async_channel::Sender<Evt>,
) -> Result<()> {
    let emit = |e: Evt| {
        let _ = evts.try_send(e);
    };
    let conn = Arc::new(Connection::open(target)?);
    let ws = conn.websocket()?;
    let mut rpc = Rpc {
        ws,
        next_id: 0,
        pending: HashMap::new(),
        methods: HashMap::new(),
        fences: HashMap::new(),
    };
    emit(Evt::Connected(conn.clone()));
    rpc.call("profiles.list", json!({}), Pending::Profiles)?;

    loop {
        while let Ok(cmd) = cmds.try_recv() {
            if !rpc.handle(cmd)? {
                return Ok(());
            }
        }
        let frame = match rpc.ws.read() {
            Ok(Message::Text(t)) => t,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                continue;
            }
            Err(e) => return Err(anyhow!("connection lost: {e}")),
        };
        let Ok(msg) = serde_json::from_str::<Value>(&frame) else {
            continue;
        };

        if let Some(id) = msg["id"].as_u64() {
            let method = rpc.methods.remove(&id).unwrap_or_default();
            let Some(kind) = rpc.pending.remove(&id) else {
                continue;
            };
            if let Some(err) = msg.get("error") {
                let text = err["message"].as_str().unwrap_or("error").to_string();
                log_error(&method, &err.to_string());
                match kind {
                    Pending::Delete => emit(Evt::DeleteFailed(text)),
                    Pending::Inserts(session) => emit(Evt::InsertFailed(session, text)),
                    Pending::Create => emit(Evt::CreateFailed(text)),
                    Pending::Resume(session, _) => emit(Evt::ResumeFailed(session, text)),
                    // A poll every few seconds: logged above, never a toast.
                    Pending::Processes(_) => {}
                    // The thread itself could not be read: said where it would show.
                    Pending::Thread { session, .. } => emit(Evt::ResumeFailed(session, text)),
                    // A close of a session the gateway already dropped is not worth a toast.
                    // Fire-and-forget calls on a session the gateway no longer (or not yet) knows:
                    // logged above, not worth a toast.
                    Pending::Ignore if text.contains("not found") || text.contains("not_found") => {
                    }
                    _ => emit(Evt::Error(text)),
                }
                continue;
            }
            // Resume -> fetch the history projection for sources and timing.
            if let Pending::Resume(session, profile) = kind {
                let r = &msg["result"];
                let runtime = r["session_id"].as_str().unwrap_or(&session).to_string();
                rpc.note_fence(&runtime, r);

                let mut replay: Vec<Value> = r["prompts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| {
                        let kind = if p["kind"] == "clarify" {
                            "clarify.request"
                        } else {
                            "approval.request"
                        };
                        json!({"type": kind, "session_id": runtime, "payload": p})
                    })
                    .collect();
                // The session's own settings (its reasoning effort), as the event that updates them.
                if r["info"].is_object() {
                    replay.push(json!({"type": "session.info", "session_id": runtime,
                                       "payload": r["info"]}));
                }
                // The thread's numbers from its last turn: the bar shows them at once, not
                // after the next answer.
                if r["usage"].is_object() {
                    replay.push(json!({"type": "session.usage", "session_id": runtime,
                                       "payload": {"usage": r["usage"]}}));
                }
                // How fast the model wrote in this thread's last answer: its chart comes back
                // with the thread, not with the app.
                if r["speed"].is_object() {
                    replay.push(json!({"type": "session.speed", "session_id": runtime,
                                       "payload": {"speed": r["speed"]}}));
                }
                if r["inserts"].is_array() {
                    replay.push(json!({"type": "session.inserts", "session_id": runtime,
                                       "payload": {"inserts": r["inserts"]}}));
                }
                if r["todo_state"]["todos"].is_array() {
                    replay.push(json!({"type": "todo.updated", "session_id": runtime,
                                       "payload": r["todo_state"]}));
                }
                let params = json!({"session_id": runtime, "profile": profile});
                rpc.call(
                    "thread.open",
                    params,
                    Pending::Thread {
                        session,
                        runtime,
                        replay,
                    },
                )?;
                continue;
            }
            if matches!(kind, Pending::Create) {
                if let Some(sid) = msg["result"]["session_id"].as_str() {
                    rpc.note_fence(sid, &msg["result"]);
                }
            }
            on_result(kind, &msg["result"], &emit);
            continue;
        }
        // session.info carries the authority's current revision/generation; every event frame
        // is stamped with the generation of the turn it belongs to.
        if let Some(sid) = msg["params"]["session_id"].as_str() {
            if let Some(g) = msg["params"]["execution_generation"].as_i64() {
                rpc.fences.insert(sid.to_string(), g);
            }
            if msg["params"]["type"] == "session.info" {
                let payload = msg["params"]["payload"].clone();
                rpc.note_fence(sid, &payload);
            }
        }
        on_event(&msg["params"], &emit);
    }
}
