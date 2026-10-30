//! The agent's JSON-RPC session (crates/protocol), like Zed's `client::Client`: a typed request
//! is answered as a future of its own response type; what the agent tells unasked comes as
//! `Incoming` on one channel. One client per agent (`app/agents.rs`), its socket on its own
//! thread (`socket.rs`).

mod socket;

use std::future::Future;
use std::sync::{Arc, mpsc};

use anyhow::{Result, anyhow};
use protocol::Request;
use serde_json::Value;

use crate::app::model::Target;
use crate::client::connection::Connection;

/// What an agent's session brings without being asked.
pub enum Incoming {
    /// Connected (again): its REST connection.
    Connected(Arc<Connection>),
    /// It could not be reached, or the connection broke: why.
    Lost(String),
    Event(protocol::Event),
}

/// One call on its way: the method, its params, where its answer goes.
pub(crate) enum Outgoing {
    Call {
        method: &'static str,
        params: Value,
        reply: async_channel::Sender<Result<Value>>,
    },
    Shutdown,
}

/// The session with one agent; dropping it closes the socket.
pub struct Client {
    tx: mpsc::Sender<Outgoing>,
    /// What answers instead of an agent (`client/fake.rs`): no socket, no thread.
    fake: Option<Arc<dyn crate::client::fake::FakeAgent>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.tx.send(Outgoing::Shutdown);
    }
}

impl Client {
    pub fn start(target: Target, incoming: async_channel::Sender<Incoming>) -> Self {
        let (tx, rx) = mpsc::channel();
        if let Some(fake) = crate::client::fake::fake() {
            let conn = Connection::to_fake(fake.clone());
            let _ = incoming.try_send(Incoming::Connected(Arc::new(conn)));
            return Self { tx, fake: Some(fake) };
        }
        std::thread::spawn(move || {
            if let Err(err) = socket::run(&target, &rx, &incoming) {
                let _ = incoming.try_send(Incoming::Lost(format!("{err:#}")));
            }
        });
        Self { tx, fake: None }
    }

    /// Ask the agent; the future resolves with its answer, or the error it gave.
    pub fn request<R: Request>(&self, request: &R) -> impl Future<Output = Result<R::Response>> + use<R> {
        let sent = self.send(request);
        async move {
            let answer = sent?.recv().await.map_err(|_| anyhow!("the agent's connection closed"))??;
            Ok(serde_json::from_value(answer)?)
        }
    }

    /// The same, waited for on this thread (`bench --probe`, no UI).
    #[cfg(not(target_family = "wasm"))]
    pub fn request_blocking<R: Request>(&self, request: &R) -> Result<R::Response> {
        let answer = self
            .send(request)?
            .recv_blocking()
            .map_err(|_| anyhow!("the agent's connection closed"))??;
        Ok(serde_json::from_value(answer)?)
    }

    fn send<R: Request>(&self, request: &R) -> Result<async_channel::Receiver<Result<Value>>> {
        let (reply, answer) = async_channel::bounded(1);
        if let Some(fake) = &self.fake {
            let _ = reply.try_send(fake.rpc(R::METHOD, serde_json::to_value(request)?));
            return Ok(answer);
        }
        self.tx
            .send(Outgoing::Call {
                method: R::METHOD,
                params: serde_json::to_value(request)?,
                reply,
            })
            .map_err(|_| anyhow!("the agent's connection is closed"))?;
        Ok(answer)
    }
}
