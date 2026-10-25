//! Remote projects: a project can be the agent on a server (an ~/.ssh/config host), as Hermes'
//! gateways were. Bench keeps a connection to each such agent beside the local one (installed
//! and started over SSH, reached through a tunnel: `connection.rs`); the server's threads join
//! the sidebar under their project, and a thread's commands go to the agent that keeps it.

use std::sync::Arc;

use gpui_kit::*;

use super::BenchApp;
use crate::connection::Connection;
use crate::gateway::{Cmd, Evt, Gateway};
use crate::model::Target;

/// The agent on a server a project lives on.
pub struct RemoteAgent {
    pub gateway: Gateway,
    pub conn: Option<Arc<Connection>>,
    pub error: Option<String>,
}

impl BenchApp {
    /// Connect to the agents of the remote projects (once each).
    pub fn connect_remotes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hosts: Vec<String> = self.projects.iter().filter_map(|p| p.host.clone()).collect();
        for host in hosts {
            if self.remotes.contains_key(&host) || self.demo {
                continue;
            }
            let (tx, rx) = async_channel::unbounded::<Evt>();
            let gateway = Gateway::start(Target::Ssh(host.clone()), tx);
            self.remotes.insert(host.clone(), RemoteAgent { gateway, conn: None, error: None });
            cx.spawn_in(window, async move |this, cx| {
                while let Ok(evt) = rx.recv().await {
                    if this.update_in(cx, |this, window, cx| this.on_remote_event(&host, evt, window, cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
    }

    /// An event from a server's agent: its connection and its threads are its own; what a
    /// thread streams goes where the local one's would.
    fn on_remote_event(&mut self, host: &str, evt: Evt, window: &mut Window, cx: &mut Context<Self>) {
        match evt {
            Evt::Connected(conn) => {
                if let Some(r) = self.remotes.get_mut(host) {
                    r.conn = Some(conn);
                    r.error = None;
                    r.gateway.send(Cmd::Sessions("default".into()));
                }
                cx.notify();
            }
            Evt::Error(e) => {
                if let Some(r) = self.remotes.get_mut(host) {
                    r.error = Some(e);
                }
                cx.notify();
            }
            // The server's agent has no profiles to pick: its threads are the project's.
            Evt::Profiles(_) | Evt::Catalog(_) => {}
            Evt::Sessions(_, rows) => {
                let project = self.projects.iter().find(|p| p.host.as_deref() == Some(host)).map(|p| p.id.clone()).unwrap_or_default();
                self.sessions.retain(|s| s.host != host);
                self.sessions.extend(rows.into_iter().filter(|s| s.source != "subagent").map(|mut s| {
                    s.host = host.to_string();
                    s.project = project.clone();
                    s
                }));
                cx.notify();
            }
            Evt::SessionsChanged | Evt::Deleted => {
                if let Some(r) = self.remotes.get(host) {
                    r.gateway.send(Cmd::Sessions("default".into()));
                }
            }
            other => self.on_event(other, window, cx),
        }
    }

    /// The server whose agent keeps the open thread (or will keep the new one): "" = this Mac.
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
        self.sessions.iter().find(|s| s.id == id).map(|s| s.host.clone()).unwrap_or_default()
    }

    /// Send a command to the agent it concerns: the thread's own (a server's, or this Mac's).
    pub fn route(&self, cmd: Cmd) {
        let host = match &cmd {
            Cmd::Sessions(_) | Cmd::Catalog { .. } => String::new(),
            Cmd::Delete { session, .. } => self.host_of_session(session),
            _ => self.thread_host(),
        };
        if host.is_empty() {
            if let Some(g) = &self.gateway {
                g.send(cmd);
            }
        } else if let Some(r) = self.remotes.get(&host) {
            r.gateway.send(cmd);
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
            project: if remote { String::new() } else { self.thread_project.clone() },
        }
    }

    /// The connection a project's environment is read through, and its id there: a remote
    /// project is its server agent's general environment.
    pub fn env_target(&self, project: &str) -> Option<(Arc<Connection>, String)> {
        match self.projects.iter().find(|p| p.id == project).and_then(|p| p.host.clone()) {
            Some(host) => self.remotes.get(&host)?.conn.clone().map(|c| (c, "general".to_string())),
            None => self.conn.clone().map(|c| (c, project.to_string())),
        }
    }
}
