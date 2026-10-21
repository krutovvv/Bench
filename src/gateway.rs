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
    /// Drop the last exchange and ask the same question again.
    Regenerate {
        profile: String,
        session: String,
        text: String,
    },
    Shutdown,
}

pub enum Evt {
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
    },
    ToolDone {
        session: String,
        failed: bool,
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
    canonical: bool,
}

impl Rpc {
    fn call(&mut self, method: &str, params: Value, kind: Pending) -> Result<()> {
        self.next_id += 1;
        self.pending.insert(self.next_id, kind);
        let frame =
            json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params});
        self.ws.send(Message::text(frame.to_string()))?;
        Ok(())
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
            Cmd::Create { profile } => self.call(
                "session.create",
                json!({"profile": profile}),
                Pending::Create,
            )?,
            Cmd::Send {
                profile,
                session,
                text,
                effort,
            } => {
                if let Some(value) = effort {
                    let params = json!({"key": "reasoning", "value": value, "session_id": session, "profile": profile});
                    self.call("config.set", params, Pending::Ignore)?;
                }
                let mut params = json!({"session_id": session, "text": text, "profile": profile});
                if self.canonical {
                    params["submission_id"] = json!(uuid::Uuid::new_v4().to_string());
                }
                self.call("prompt.submit", params, Pending::Ignore)?
            }
            Cmd::Interrupt { profile, session } => self.call(
                "session.interrupt",
                json!({"session_id": session, "profile": profile}),
                Pending::Ignore,
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
        canonical: conn.canonical(),
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
            Err(e) => return Err(anyhow!("соединение потеряно: {e}")),
        };
        let Ok(msg) = serde_json::from_str::<Value>(&frame) else {
            continue;
        };

        if let Some(id) = msg["id"].as_u64() {
            let Some(kind) = rpc.pending.remove(&id) else {
                continue;
            };
            if let Some(err) = msg.get("error") {
                let text = err["message"].as_str().unwrap_or("error").to_string();
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
                    Pending::Ignore if text.contains("not found") => {}
                    _ => emit(Evt::Error(text)),
                }
                continue;
            }
            // Resume -> fetch the history projection for sources and timing.
            if let Pending::Resume(session, profile) = kind {
                let r = &msg["result"];
                let runtime = r["session_id"].as_str().unwrap_or(&session).to_string();
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
            on_result(kind, &msg["result"], &emit);
            continue;
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
                        .unwrap_or("Без названия")
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
        Pending::Ignore => {}
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
            session: sid,
        }),
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
