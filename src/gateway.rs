//! The JSON-RPC session on its own thread. The UI sends `Cmd`s and receives `Evt`s.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::{Arc, mpsc};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::connection::Connection;
use crate::history;
use crate::model::{Profile, SessionRow, Target, Turn, Usage};

pub enum Cmd {
    Sessions(String),
    Open {
        profile: String,
        session: String,
    },
    Create {
        profile: String,
        /// The working folder (a project, or a branch folder of one); None = Hermes' default.
        cwd: Option<String>,
    },
    Send {
        profile: String,
        session: String,
        text: String,
        effort: Option<String>,
    },
    Interrupt {
        profile: String,
        session: String,
    },
    /// `runtime` set when this client has the session open: it is closed first,
    /// since the gateway refuses to delete an active session.
    Delete {
        profile: String,
        session: String,
        runtime: Option<String>,
    },
    /// Switch the session's model (primary or one of the profile's fallbacks).
    SetModel {
        profile: String,
        session: String,
        model: String,
        provider: String,
    },
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
    /// Run a slash command in a session.
    Dispatch {
        profile: String,
        session: String,
        name: String,
        arg: String,
    },
    /// `approvals.mode` for the profile: off / smart / manual.
    SetApproval {
        profile: String,
        mode: String,
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
    /// A slash command ran: either it printed something, or it wants a prompt sent.
    Dispatched {
        session: String,
        output: Option<String>,
        send: Option<String>,
    },
    Approval(crate::model::ApprovalReq),
    /// The background self-improvement review of the last turn: what it saved or patched.
    Review(String, String),
    /// The pending approval was answered or withdrawn.
    ApprovalDone(String),
    Subagent {
        session: String,
        id: String,
        /// "start" | "tool" | "text" | "complete"
        kind: &'static str,
        text: String,
        status: String,
    },
    Connected(Arc<Connection>),
    Error(String),
    Profiles(Vec<Profile>),
    Sessions(String, Vec<SessionRow>),
    /// `session` is the stored id (session list); `runtime` the id prompts and events use.
    History {
        session: String,
        runtime: String,
        turns: Vec<Turn>,
        running: bool,
    },
    Created {
        session: String,
        runtime: String,
    },
    /// The gateway says the session list changed (new session, generated title, …).
    SessionsChanged,
    Deleted,
    DeleteFailed(String),
    Started(String),
    Delta(String, String),
    Reasoning(String, String),
    ToolStart {
        session: String,
        name: String,
        detail: String,
        urls: Vec<String>,
        call_id: String,
        args: String,
    },
    ToolDone {
        session: String,
        failed: bool,
        call_id: String,
        output: String,
        edit: Option<crate::model::FileEdit>,
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
    /// Resume done; `session.history` fills in sources and timing before the UI gets it.
    Meta {
        session: String,
        runtime: String,
        turns: Vec<Turn>,
        running: bool,
    },
    Create,
    Delete,
    Catalog,
    Dispatch(String),
    /// Regenerate, step 1 done: the transcript (for the last user row id) has arrived.
    Regen {
        profile: String,
        text: String,
    },
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
                let _ = evts.send_blocking(Evt::Error(format!("{err:#}")));
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
    canonical: bool,
    /// runtime session id -> (revision, execution generation), for fenced mutations.
    fences: HashMap<String, (i64, i64)>,
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

    /// A fenced session mutation (canonical protocol): model, reasoning, …
    fn mutate(&mut self, session: &str, operation: &str, payload: Value) -> Result<()> {
        let (revision, generation) = self.fences.get(session).copied().unwrap_or((0, 0));
        let params = json!({
            "session_id": session,
            "request_id": uuid::Uuid::new_v4().to_string(),
            "expected_revision": revision,
            "expected_generation": generation,
            "operation": operation,
            "payload": payload,
        });
        self.call("session.mutate", params, Pending::Ignore)
    }

    /// Remember a session's revision/generation from a snapshot-shaped value.
    fn note_fence(&mut self, session: &str, v: &Value) {
        let rev = v["revision"].as_i64();
        let generation = v["execution_generation"].as_i64();
        if rev.is_some() || generation.is_some() {
            let entry = self.fences.entry(session.to_string()).or_insert((0, 0));
            if let Some(r) = rev {
                entry.0 = r;
            }
            if let Some(g) = generation {
                entry.1 = g;
            }
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
            Cmd::Create { profile, cwd } => {
                let mut params = json!({"profile": profile});
                if let Some(cwd) = cwd {
                    params["cwd"] = json!(cwd);
                }
                self.call("session.create", params, Pending::Create)?
            }
            Cmd::SetModel {
                profile,
                session,
                model,
                provider,
            } => {
                if self.canonical {
                    self.mutate(
                        &session,
                        "model",
                        json!({"model": model, "provider": provider}),
                    )?
                } else {
                    let command = if provider.is_empty() {
                        format!("/model {model}")
                    } else {
                        format!("/model {model} --provider {provider}")
                    };
                    self.call(
                        "slash.exec",
                        json!({"session_id": session, "profile": profile, "command": command}),
                        Pending::Ignore,
                    )?
                }
            }
            Cmd::SetEffort { session, effort } => {
                self.mutate(&session, "reasoning", json!({"effort": effort}))?
            }
            Cmd::Catalog { profile } => self.call(
                "commands.catalog",
                json!({"profile": profile}),
                Pending::Catalog,
            )?,
            Cmd::Dispatch {
                profile,
                session,
                name,
                arg,
            } => {
                let params = if self.canonical {
                    json!({"session_id": session, "profile": profile, "name": name, "arg": arg})
                } else {
                    let command = if arg.is_empty() {
                        name
                    } else {
                        format!("{name} {arg}")
                    };
                    json!({"session_id": session, "profile": profile, "command": command})
                };
                let method = if self.canonical {
                    "command.dispatch"
                } else {
                    "slash.exec"
                };
                self.call(method, params, Pending::Dispatch(session))?
            }
            Cmd::SetApproval { profile, mode } => self.call(
                "config.set",
                json!({"profile": profile, "key": "approvals.mode", "value": mode}),
                Pending::Ignore,
            )?,
            Cmd::Approve {
                profile,
                req,
                choice,
            } => {
                let params = if self.canonical {
                    let generation = req
                        .generation
                        .or_else(|| self.fences.get(&req.session).map(|f| f.1))
                        .unwrap_or(0);
                    json!({"session_id": req.session, "execution_generation": generation,
                           "prompt_id": req.prompt_id, "choice": choice})
                } else {
                    json!({"session_id": req.session, "request_id": req.prompt_id,
                           "choice": choice, "profile": profile})
                };
                self.call("approval.respond", params, Pending::Ignore)?
            }
            Cmd::Send {
                profile,
                session,
                text,
                effort,
            } => {
                if let Some(value) = effort {
                    if self.canonical {
                        self.mutate(&session, "reasoning", json!({"effort": value}))?;
                    } else {
                        let params = json!({"key": "reasoning", "value": value, "session_id": session, "profile": profile});
                        self.call("config.set", params, Pending::Ignore)?;
                    }
                }
                let mut params = json!({"session_id": session, "text": text, "profile": profile});
                if self.canonical {
                    params["submission_id"] = json!(uuid::Uuid::new_v4().to_string());
                }
                self.call("prompt.submit", params, Pending::Ignore)?
            }
            // Stop names the turn it stops: the gateway refuses any other generation.
            Cmd::Interrupt { profile, session } => {
                let generation = self.fences.get(&session).map_or(0, |f| f.1);
                self.call(
                    "session.interrupt",
                    json!({"session_id": session, "profile": profile,
                           "execution_generation": generation}),
                    Pending::Ignore,
                )?
            }
            // The owner protocol has no undo: re-read the transcript for the last user row, cut
            // there (a fenced `rewind`), then submit the same text again.
            Cmd::Regenerate {
                profile,
                session,
                text,
            } if self.canonical => self.call(
                "session.resume",
                json!({"session_id": session, "profile": profile}),
                Pending::Regen { profile, text },
            )?,
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
                let mut params = json!({"session_id": session, "text": text, "profile": profile});
                if self.canonical {
                    params["submission_id"] = json!(uuid::Uuid::new_v4().to_string());
                }
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
        let _ = evts.send_blocking(e);
    };
    let conn = Arc::new(Connection::open(target)?);
    let ws = conn.websocket()?;
    let mut rpc = Rpc {
        ws,
        next_id: 0,
        pending: HashMap::new(),
        methods: HashMap::new(),
        canonical: conn.canonical(),
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
                    // Older gateways may lack the projection: show the turns without extras.
                    Pending::Meta {
                        session,
                        runtime,
                        turns,
                        running,
                    } => emit(Evt::History {
                        session,
                        runtime,
                        turns,
                        running,
                    }),
                    // A close of a session the gateway already dropped is not worth a toast.
                    // Fire-and-forget calls on a session the gateway no longer (or not yet) knows:
                    // logged above, not worth a toast.
                    Pending::Ignore if text.contains("not found") || text.contains("not_found") => {
                    }
                    _ => emit(Evt::Error(text)),
                }
                continue;
            }
            if let Pending::Regen { profile, text } = kind {
                let r = &msg["result"];
                let Some(sid) = r["session_id"].as_str().map(str::to_string) else {
                    continue;
                };
                rpc.note_fence(&sid, r);
                let row = r["messages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .rev()
                    .find(|m| m["role"] == "user" && m["row_id"].is_i64())
                    .and_then(|m| m["row_id"].as_i64());
                if let Some(row) = row {
                    rpc.mutate(&sid, "rewind", json!({"target_message_id": row}))?;
                }
                let params = json!({"session_id": sid, "text": text, "profile": profile,
                                    "submission_id": uuid::Uuid::new_v4().to_string()});
                rpc.call("prompt.submit", params, Pending::Ignore)?;
                continue;
            }
            // Resume -> fetch the history projection for sources and timing.
            if let Pending::Resume(session, profile) = kind {
                let r = &msg["result"];
                let runtime = r["session_id"].as_str().unwrap_or(&session).to_string();
                rpc.note_fence(&runtime, r);
                let turns =
                    history::turns(r["messages"].as_array().map(Vec::as_slice).unwrap_or(&[]));
                let running = r["running"].as_bool().unwrap_or(false);
                let params = json!({"session_id": runtime, "profile": profile});
                rpc.call(
                    "session.history",
                    params,
                    Pending::Meta {
                        session,
                        runtime,
                        turns,
                        running,
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
                rpc.fences.entry(sid.to_string()).or_insert((0, 0)).1 = g;
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
                    model: s["model"].as_str().unwrap_or_default().into(),
                    running: s["running"].as_bool().unwrap_or(false),
                    source: s["source"].as_str().unwrap_or_default().into(),
                })
                .collect(),
        )),
        Pending::Resume(..) => {}
        Pending::Meta {
            session,
            runtime,
            mut turns,
            running,
        } => {
            history::attach(&mut turns, history::metas(&list(&r["messages"])));
            emit(Evt::History {
                session,
                runtime,
                turns,
                running,
            })
        }
        Pending::Create => {
            let runtime = r["session_id"].as_str().unwrap_or_default().to_string();
            let session = r["stored_session_id"]
                .as_str()
                .unwrap_or(&runtime)
                .to_string();
            emit(Evt::Created { session, runtime })
        }
        Pending::Delete => emit(Evt::Deleted),
        Pending::Catalog => emit(Evt::Catalog(catalog(r))),
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
        Pending::Ignore | Pending::Regen { .. } => {}
    }
}

fn on_event(p: &Value, emit: &impl Fn(Evt)) {
    let sid = p["session_id"].as_str().unwrap_or_default().to_string();
    let payload = &p["payload"];
    let text = || payload["text"].as_str().unwrap_or_default().to_string();
    match p["type"].as_str().unwrap_or_default() {
        "message.start" => emit(Evt::Started(sid)),
        "message.delta" => emit(Evt::Delta(sid, text())),
        "reasoning.delta" => emit(Evt::Reasoning(sid, text())),
        "tool.start" => emit(Evt::ToolStart {
            call_id: payload["tool_id"].as_str().unwrap_or_default().to_string(),
            args: history::pretty(&payload["args"]),
            name: payload["name"].as_str().unwrap_or("tool").to_string(),
            detail: payload["context"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| history::preview(&payload["args"])),
            urls: {
                let mut urls = vec![];
                history::tool_urls(
                    payload["name"].as_str().unwrap_or_default(),
                    &payload["args"],
                    &mut urls,
                );
                urls
            },
            session: sid,
        }),
        "tool.complete" => emit(Evt::ToolDone {
            failed: history::failed(&payload["result"]),
            call_id: payload["tool_id"].as_str().unwrap_or_default().to_string(),
            output: history::pretty(&payload["result"]),
            edit: history::file_edit(
                payload["name"].as_str().unwrap_or_default(),
                &payload["result"],
            ),
            session: sid,
        }),
        "approval.request" => emit(Evt::Approval(crate::model::ApprovalReq {
            prompt_id: payload["prompt_id"]
                .as_str()
                .or(payload["request_id"].as_str())
                .unwrap_or_default()
                .to_string(),
            generation: payload["execution_generation"].as_i64(),
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
        kind @ ("subagent.start" | "subagent.tool" | "subagent.text" | "subagent.complete") => {
            let s = |k: &str| payload[k].as_str().unwrap_or_default().to_string();
            let id = payload["subagent_id"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| format!("sa:{}:{}", payload["task_index"], s("goal")));
            let (kind, text) = match kind {
                "subagent.start" => ("start", {
                    let g = s("goal");
                    if g.is_empty() { s("preview") } else { g }
                }),
                "subagent.tool" => ("tool", {
                    let name = s("tool_name");
                    let preview = s("tool_preview");
                    let preview = if preview.is_empty() {
                        s("text")
                    } else {
                        preview
                    };
                    crate::app::tool_label(&name, &preview)
                }),
                "subagent.text" => ("text", s("text")),
                _ => ("complete", {
                    let summary = s("summary");
                    if summary.is_empty() {
                        s("text")
                    } else {
                        summary
                    }
                }),
            };
            emit(Evt::Subagent {
                session: sid,
                id,
                kind,
                text,
                status: s("status"),
            })
        }
        "review.summary" => {
            let t = text();
            if !t.trim().is_empty() {
                emit(Evt::Review(sid, t.trim().to_string()))
            }
        }
        "sessions.changed" => emit(Evt::SessionsChanged),
        "session.usage" => emit(Evt::Usage(sid, Usage::from_json(&payload["usage"]))),
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
