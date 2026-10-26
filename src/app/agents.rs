//! The agents Bench works with (`docs/remote-projects.md`): this Mac's (key "") and one on each
//! server a project lives on (key: its ~/.ssh/config alias), installed and started over SSH and
//! reached through a tunnel (`connection.rs`). One list: a thread's commands, its files, its
//! media and attachments go to the agent that keeps it, whichever that is.

use std::sync::Arc;

use gpui_kit::*;

use super::BenchApp;
use crate::connection::Connection;
use crate::gateway::{Cmd, Evt, Gateway};
use crate::model::Target;

/// One agent: its command channel, its REST connection once up, why it is not up.
pub struct AgentLink {
    pub gateway: Gateway,
    pub conn: Option<Arc<Connection>>,
    pub error: Option<String>,
}

impl AgentLink {
    pub fn new(gateway: Gateway) -> Self {
        Self {
            gateway,
            conn: None,
            error: None,
        }
    }
}

impl BenchApp {
    /// The agent kept under `host` ("" = this Mac's): its REST connection, once up.
    pub fn conn_of(&self, host: &str) -> Option<Arc<Connection>> {
        self.agents.get(host).and_then(|a| a.conn.clone())
    }

    /// This Mac's agent.
    pub fn local_conn(&self) -> Option<Arc<Connection>> {
        self.conn_of("")
    }

    /// The agent keeping the open thread (or the new one), and where its files are.
    pub fn thread_conn(&self) -> Option<Arc<Connection>> {
        self.conn_of(&self.thread_host())
    }

    /// Where the agent under `host` runs, as media and attachments reach its files.
    pub fn target_of(&self, host: &str) -> Target {
        if host.is_empty() {
            self.target.clone()
        } else {
            Target::Ssh(host.to_string())
        }
    }

    pub fn thread_target(&self) -> Target {
        self.target_of(&self.thread_host())
    }

    /// Connect to the agents of the remote projects (once each).
    pub fn connect_remotes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hosts: Vec<String> = self
            .projects
            .iter()
            .filter_map(|p| p.host.clone())
            .collect();
        for host in hosts {
            if self.agents.contains_key(&host) || self.demo {
                continue;
            }
            let (tx, rx) = async_channel::unbounded::<Evt>();
            let gateway = Gateway::start(Target::Ssh(host.clone()), tx);
            self.agents.insert(host.clone(), AgentLink::new(gateway));
            cx.spawn_in(window, async move |this, cx| {
                while let Ok(evt) = rx.recv().await {
                    if this
                        .update_in(cx, |this, window, cx| {
                            this.on_remote_event(&host, evt, window, cx)
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
    }

    /// An event from a server's agent: its connection and its threads are its own; what a
    /// thread streams goes where the local one's would.
    fn on_remote_event(
        &mut self,
        host: &str,
        evt: Evt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match evt {
            Evt::Connected(conn) => {
                if let Some(r) = self.agents.get_mut(host) {
                    r.conn = Some(conn);
                    r.error = None;
                    r.gateway.send(Cmd::Sessions("default".into()));
                }
                cx.notify();
            }
            Evt::Error(e) => {
                if let Some(r) = self.agents.get_mut(host) {
                    r.error = Some(e);
                }
                cx.notify();
            }
            // The server's agent has no profiles to pick: its threads are the project's.
            Evt::Profiles(_) | Evt::Catalog(_) => {}
            Evt::Sessions(_, rows) => {
                let project = self
                    .projects
                    .iter()
                    .find(|p| p.host.as_deref() == Some(host))
                    .map(|p| p.id.clone())
                    .unwrap_or_default();
                self.sessions.retain(|s| s.host != host);
                self.sessions
                    .extend(
                        rows.into_iter()
                            .filter(|s| s.source != "subagent")
                            .map(|mut s| {
                                s.host = host.to_string();
                                s.project = project.clone();
                                s
                            }),
                    );
                cx.notify();
            }
            Evt::SessionsChanged | Evt::Deleted => {
                if let Some(r) = self.agents.get(host) {
                    r.gateway.send(Cmd::Sessions("default".into()));
                }
            }
            other => self.on_event(other, window, cx),
        }
    }

    /// The server whose agent keeps the open thread (or will keep the new one): "" = this Mac.
    /// Where the open thread's shell runs: its project's server, or this Mac (`term::LOCAL`).
    pub fn thread_machine(&self) -> String {
        let host = self.thread_host();
        if host.is_empty() {
            crate::inserts::terminal::session::LOCAL.to_string()
        } else {
            host
        }
    }

    pub fn thread_host(&self) -> String {
        match &self.session {
            Some(id) => self.host_of_session(id),
            None => self
                .projects
                .iter()
                .find(|p| p.id == self.thread_project)
                .and_then(|p| p.host.clone())
                .unwrap_or_default(),
        }
    }

    pub fn host_of_session(&self, id: &str) -> String {
        self.sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.host.clone())
            .unwrap_or_default()
    }

    /// Send a command to the agent it concerns: the thread's own (a server's, or this Mac's).
    pub fn route(&self, cmd: Cmd) {
        let host = match &cmd {
            Cmd::Sessions(_) | Cmd::Catalog { .. } => String::new(),
            Cmd::Delete { session, .. } => self.host_of_session(session),
            _ => self.thread_host(),
        };
        if let Some(a) = self.agents.get(&host) {
            a.gateway.send(cmd);
        }
    }

    /// A new thread's creation, for the agent that will keep it: a remote project's thread is
    /// its server agent's own (its general environment, no project there).
    pub fn create_cmd(&self, profile: String, cwd: Option<String>) -> Cmd {
        let remote = !self.thread_host().is_empty();
        Cmd::Create {
            profile: if remote { "default".into() } else { profile },
            cwd,
            subject: self.new_subject.clone(),
            project: if remote {
                String::new()
            } else {
                self.thread_project.clone()
            },
        }
    }

    /// The connection a project's environment is read through, and its id there: a remote
    /// project is its server agent's general environment.
    pub fn env_target(&self, project: &str) -> Option<(Arc<Connection>, String)> {
        match self
            .projects
            .iter()
            .find(|p| p.id == project)
            .and_then(|p| p.host.clone())
        {
            Some(host) => self.conn_of(&host).map(|c| (c, "general".to_string())),
            None => self.local_conn().map(|c| (c, project.to_string())),
        }
    }
}
