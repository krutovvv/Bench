//! The JSON-RPC session on its own thread. The UI sends `Cmd`s and receives `Evt`s.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::{Arc, mpsc};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::connection::Connection;
use crate::model::{Profile, SessionRow, Target, Usage};

pub enum Cmd {
    Sessions(String),
    Open {
        profile: String,
        session: String,
    },
    Create {
        profile: String,
        /// The working folder (a project, or a branch folder of one); None = the agent's default.
        cwd: Option<String>,
        /// What the thread is about ("tasks:today", "ssh:<host>"); "" = a plain chat.
        subject: String,
        /// The project it belongs to (its id); "" = none.
        project: String,
    },
    /// An insert put into the thread where it is now (agent/src/inserts.ts).
    Insert {
        session: String,
        id: String,
        key: String,
        state: Value,
    },
    /// An insert changed: {key?, state?, folded?, removed?}.
    PatchInsert {
        session: String,
        id: String,
        patch: Value,
    },
    /// A message sent while the agent works: it waits in the conversation's queue (Pi's inbox)
    /// until the agent answers.
    Queue {
        session: String,
        text: String,
        context: String,
    },
    /// A waiting message taken back, or sent now (into the running work).
    QueueWithdraw {
        session: String,
        id: String,
    },
    QueueSend {
        session: String,
        id: String,
    },
    /// The words of a message that still waits: it has not gone anywhere, so they may be fixed.
    QueueEdit {
        session: String,
        id: String,
        text: String,
    },
    Send {
        profile: String,
        session: String,
        text: String,
        effort: Option<String>,
        /// What the thread's live block shows now (terminal lines, the tool's input…), for the
        /// agent only: it is not part of the message the person sees.
        context: String,
    },
    Interrupt {
        profile: String,
        session: String,
    },
    /// How fast the model wrote in the answer that just ended, as Bench measured it from the
    /// stream (`crate::speed::Stat`). The agent keeps it with the thread.
    SaveSpeed {
        session: String,
        speed: Value,
    },
    /// `runtime` set when this client has the session open: it is closed first,
    /// since the gateway refuses to delete an active session.
    Delete {
        profile: String,
        session: String,
        runtime: Option<String>,
    },
    /// Switch the session's model (primary or one of the profile's fallbacks).
    /// Drop the last exchange and ask the same question again.
    Regenerate {
        profile: String,
        session: String,
        text: String,
    },
    /// Only the reasoning effort, no prompt (diagnostics).
    SetEffort {
        session: String,
        effort: String,
    },
    /// The profile's slash commands (and skills).
    Catalog {
        profile: String,
    },
    /// The session's background processes (polled while it is open).
    Processes {
        profile: String,
        session: String,
    },
    /// Stop one of them.
    KillProcess {
        profile: String,
        session: String,
        id: String,
    },
    /// Run a slash command in a session.
    Dispatch {
        profile: String,
        session: String,
        name: String,
        arg: String,
    },
    /// Answer a question (`answer` empty = declined).
    Clarify {
        profile: String,
        req: crate::model::ClarifyReq,
        answer: String,
    },
    /// Answer an approval request.
    Approve {
        profile: String,
        req: crate::model::ApprovalReq,
        choice: String,
    },
    Shutdown,
}

pub enum Evt {
    Catalog(Vec<crate::model::SlashCommand>),
    /// (runtime session, its background processes)
    Processes(String, Vec<crate::model::BgProcess>),
    /// A slash command ran: either it printed something, or it wants a prompt sent.
    Dispatched {
        session: String,
        output: Option<String>,
        send: Option<String>,
    },
    Approval(crate::model::ApprovalReq),
    Clarify(crate::model::ClarifyReq),
    /// The pending question was answered, timed out or withdrawn.
    ClarifyDone(String),
    /// The session's plan, whole: (runtime session, steps).
    /// The agent's plan and the moment (unix seconds) it began; 0 = the agent did not say
    /// (an older agent), so Bench keeps the block where it already had it.
    Todos(String, Vec<crate::model::Todo>, f64),
    /// What the thread keeps of the speed of its last answer (its chart and its last speeds).
    Speed(String, crate::speed::Stat),
    /// The pending approval was answered or withdrawn.
    ApprovalDone(String),
    Connected(Arc<Connection>),
    Error(String),
    /// A session's reasoning effort as the gateway has it ("" = the provider's default).
    Effort(String, String),
    /// The fallback model answered the session's last call: (its model, why the main one failed);
    /// None once the main model answers.
    Fallback(String, Option<(String, String)>),
    Profiles(Vec<Profile>),
    Sessions(String, Vec<SessionRow>),
    /// `session` is the stored id (session list); `runtime` the id prompts and events use.
    History {
        session: String,
        runtime: String,
        open: protocol::ThreadOpen,
    },
    /// A change of an open thread: (runtime session, the change).
    Thread(String, protocol::ThreadEvent),
    Created {
        session: String,
        runtime: String,
    },
    /// A thread's inserts as the agent keeps them (the whole list, in order).
    Inserts(String, Vec<crate::model::Insert>),
    /// The agent refused an insert's change: the thread's list is read again.
    InsertFailed(String, String),
    /// A thread's history could not be read (session id, why): it stops "loading".
    ResumeFailed(String, String),
    /// The gateway says the session list changed (new session, generated title, …).
    SessionsChanged,
    Deleted,
    DeleteFailed(String),
    Started(String),
    Delta(String, String),
    Reasoning(String, String),
    /// A tool started: what the status says the agent is doing (name, what it acts on).
    ToolStart {
        session: String,
        name: String,
        detail: String,
    },
    ToolDone {
        session: String,
    },
    Usage(String, Usage),
    Complete {
        session: String,
        text: String,
    },
}

enum Pending {
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

/// Owns the gateway thread; dropping it shuts the session down.
pub struct Gateway {
    tx: mpsc::Sender<Cmd>,
}

impl Drop for Gateway {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
    }
}

impl Gateway {
    pub fn start(target: Target, evts: async_channel::Sender<Evt>) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(err) = run(&target, &rx, &evts) {
                let _ = evts.try_send(Evt::Error(format!("{err:#}")));
            }
        });
        Self { tx }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
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

fn run(
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

fn on_result(kind: Pending, r: &Value, emit: &impl Fn(Evt)) {
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

fn inserts(v: &Value) -> Vec<crate::model::Insert> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(crate::model::Insert::from_json)
                .collect()
        })
        .unwrap_or_default()
}

fn on_event(p: &Value, emit: &impl Fn(Evt)) {
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
        "approval.request" => emit(Evt::Approval(crate::model::ApprovalReq {
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
            emit(Evt::Clarify(crate::model::ClarifyReq {
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
            crate::speed::Stat::from_json(&payload["speed"]),
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
fn catalog(r: &Value) -> Vec<crate::model::SlashCommand> {
    let pair = |p: &Value, category: &str| {
        let name = p[0].as_str()?.trim().to_string();
        let name = if name.starts_with('/') {
            name
        } else {
            format!("/{name}")
        };
        Some(crate::model::SlashCommand {
            name,
            desc: p[1].as_str().unwrap_or_default().trim().to_string(),
            category: category.to_string(),
            key: String::new(),
        })
    };
    let mut out: Vec<crate::model::SlashCommand> = vec![];
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
fn log_error(method: &str, err: &str) {
    eprintln!("[gateway] {method}: {err}");
    let path = crate::connection::state_dir().join("gateway-errors.log");
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

fn todos(payload: &Value) -> Vec<crate::model::Todo> {
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
            crate::model::Todo {
                content: t["content"].as_str().unwrap_or_default().to_string(),
                status: t["status"].as_str().unwrap_or("pending").to_string(),
                depth,
            }
        })
        .collect()
}

/// `process.list`'s rows (the process id rides in `session_id`).
fn processes(r: &Value) -> Vec<crate::model::BgProcess> {
    r["processes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| crate::model::BgProcess {
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
