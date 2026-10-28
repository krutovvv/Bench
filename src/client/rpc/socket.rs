//! The socket itself, on its own thread: each call sent as a JSON-RPC frame, each answer handed
//! back to its caller by id, each event read as a `protocol::Event`.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::{Arc, mpsc};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use tungstenite::Message;

use super::{Incoming, Outgoing};
use crate::app::model::Target;
use crate::client::connection::Connection;

pub(super) fn run(
    target: &Target,
    calls: &mpsc::Receiver<Outgoing>,
    incoming: &async_channel::Sender<Incoming>,
) -> Result<()> {
    let conn = Arc::new(Connection::open(target)?);
    let mut ws = conn.websocket()?;
    let _ = incoming.try_send(Incoming::Connected(conn.clone()));
    let mut next = 0u64;
    let mut waiting: HashMap<u64, (&'static str, async_channel::Sender<Result<Value>>)> =
        HashMap::new();
    let result = loop {
        while let Ok(out) = calls.try_recv() {
            match out {
                Outgoing::Shutdown => return Ok(()),
                Outgoing::Call {
                    method,
                    params,
                    reply,
                } => {
                    next += 1;
                    let frame =
                        json!({"jsonrpc": "2.0", "id": next, "method": method, "params": params});
                    if let Err(e) = ws.send(Message::text(frame.to_string())) {
                        let _ = reply.try_send(Err(anyhow!("{e}")));
                        continue;
                    }
                    waiting.insert(next, (method, reply));
                }
            }
        }
        let frame = match ws.read() {
            Ok(Message::Text(t)) => t,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                continue;
            }
            Err(e) => break Err(anyhow!("connection lost: {e}")),
        };
        let Ok(msg) = serde_json::from_str::<Value>(&frame) else {
            continue;
        };
        if let Some(id) = msg["id"].as_u64() {
            let Some((method, reply)) = waiting.remove(&id) else {
                continue;
            };
            let answer = match msg.get("error") {
                Some(err) => {
                    let text = err["message"].as_str().unwrap_or("error").to_string();
                    log_error(method, &text);
                    Err(anyhow!(text))
                }
                None => Ok(msg["result"].clone()),
            };
            let _ = reply.try_send(answer);
        } else if msg["method"] == "event" {
            match serde_json::from_value(msg["params"].clone()) {
                Ok(event) => {
                    let _ = incoming.try_send(Incoming::Event(event));
                }
                Err(e) => log_error("event", &format!("{e}: {}", msg["params"])),
            }
        }
    };
    // Whoever still waits hears why there will be no answer.
    for (_, (_, reply)) in waiting {
        let _ = reply.try_send(Err(anyhow!("the agent's connection closed")));
    }
    result
}

/// The agent's errors go to `agent-errors.log` too (a Finder-launched app has no visible stderr).
fn log_error(method: &str, err: &str) {
    eprintln!("[agent] {method}: {err}");
    let path = crate::client::connection::state_dir().join("agent-errors.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write as _;
        let _ = writeln!(f, "{} {method}: {err}", crate::app::now_secs() as u64);
    }
}
