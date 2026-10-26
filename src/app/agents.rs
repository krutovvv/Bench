//! The agents Bench works with (`docs/remote-projects.md`): this Mac's (key "") and one on each
//! server a project lives on (key: its ~/.ssh/config alias), installed and started over SSH and
//! reached through a tunnel (`connection.rs`). One list: a thread's commands, its files, its
//! media and attachments go to the agent that keeps it, whichever that is.

use std::sync::Arc;

use gpui_kit::*;

use super::BenchApp;
use crate::connection::Connection;
use crate::gateway::{Cmd, Evt, Gateway};
use crate::model::{ProjectEntry, Target};

/// One agent: its command channel, its REST connection once up, why it is not up.
pub struct AgentLink {
    pub gateway: Gateway,
    pub conn: Option<Arc<Connection>>,
    pub error: Option<String>,
    /// The sources it runs, as its status says (None until asked, or an agent too old to say).
    pub build: Option<String>,
}

impl AgentLink {
    pub fn new(gateway: Gateway) -> Self {
        Self {
            gateway,
            conn: None,
            error: None,
            build: None,
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

    /// The agents in the order the gear lists them: this Mac's, then the servers' by name.
    pub fn agent_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self.agents.keys().cloned().collect();
        hosts.sort_by(|a, b| (!a.is_empty(), a).cmp(&(!b.is_empty(), b)));
        hosts
    }

    /// An agent as people call it.
    pub fn agent_name(host: &str) -> String {
        if host.is_empty() {
            "This Mac".into()
        } else {
            host.to_string()
        }
    }

    /// An agent came up: what waited for it is asked now — the gear showing it, a server's
    /// own projects.
    pub fn agent_up(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel == super::Panel::Providers && self.gear_agent == host && !self.pv.loaded {
            self.load_providers(window, cx);
        }
        if !host.is_empty() {
            self.load_server_projects(host.clone(), window, cx);
        }
        self.ask_build(host, cx);
    }

    /// A server agent's own projects (its Host is the row on this Mac that names the server):
    /// in the sidebar right after its Host, with ids `<host>:<id>`.
    pub fn load_server_projects(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.conn_of(&host) else {
            return;
        };
        self.background(
            window,
            cx,
            move || crate::rest::get_json(&conn, "/api/projects"),
            move |this, r, _window, cx| {
                let Ok(v) = r else {
                    return;
                };
                let list: Vec<ProjectEntry> = v["projects"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| {
                        let rid = p["id"].as_str().filter(|id| !id.is_empty() && *id != "general")?;
                        Some(ProjectEntry {
                            id: format!("{host}:{rid}"),
                            name: p["name"].as_str().unwrap_or_default().into(),
                            folder: p["folder"].as_str().filter(|f| !f.is_empty()).map(str::to_string),
                            host: Some(host.clone()),
                            remote_id: Some(rid.to_string()),
                            machine: None,
                            browser: p["browser"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect();
                this.server_projects.insert(host.clone(), list.clone());
                this.place_server_projects(&host, list);
                // Its threads were placed before its projects were known: again.
                if let Some(a) = this.agents.get(&host) {
                    a.gateway.send(Cmd::Sessions("default".into()));
                }
                cx.notify();
            },
        );
    }

    /// A server's own projects into the list, after its Host row (replacing what was there).
    pub fn place_server_projects(&mut self, host: &str, list: Vec<ProjectEntry>) {
        self.projects
            .retain(|p| !(p.host.as_deref() == Some(host) && p.remote_id.is_some()));
        let at = self
            .projects
            .iter()
            .position(|p| p.host.as_deref() == Some(host))
            .map_or(self.projects.len(), |i| i + 1);
        self.projects.splice(at..at, list);
    }

    /// The server has an agent Bench knows (a Host row names it): new projects go there.
    pub fn has_agent_on(&self, host: &str) -> bool {
        self.projects
            .iter()
            .any(|p| p.host.as_deref() == Some(host) && p.remote_id.is_none())
    }

    /// Where a project is: its agent's host ("" = this Mac) and its id there ("general" for a
    /// server's Host).
    pub fn project_place(&self, project: &str) -> (String, String) {
        match self.projects.iter().find(|p| p.id == project) {
            Some(p) => match &p.host {
                Some(h) => (h.clone(), p.remote_id.clone().unwrap_or_else(|| "general".into())),
                None => (String::new(), p.id.clone()),
            },
            None => (String::new(), project.to_string()),
        }
    }

    /// Ask a newly connected agent which sources it runs (for the gear).
    pub fn ask_build(&mut self, host: String, cx: &mut Context<Self>) {
        let Some(conn) = self.conn_of(&host) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let build = cx
                .background_spawn(async move {
                    crate::rest::get_json(&conn, "/api/status")
                        .ok()
                        .and_then(|v| v["build"].as_str().map(str::to_string))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(a) = this.agents.get_mut(&host) {
                    a.build = build;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Run `job` off the UI thread with the agent under `host`, then `done` on the UI thread.
    pub(crate) fn rest_on<T: Send + 'static>(
        &mut self,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
        job: impl FnOnce(&Connection, &str) -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let Some(conn) = self.conn_of(&host) else {
            return;
        };
        let profile = self.current_profile();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { job(&conn, &profile) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                done(this, result, window, cx);
                cx.notify();
            });
        })
        .detach();
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
                self.agent_up(host.to_string(), window, cx);
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
                // A thread of the server's own project is that project's; the rest are its Host's.
                let home = self
                    .projects
                    .iter()
                    .find(|p| p.host.as_deref() == Some(host) && p.remote_id.is_none())
                    .map(|p| p.id.clone())
                    .unwrap_or_default();
                let known: Vec<String> = self
                    .projects
                    .iter()
                    .filter(|p| p.host.as_deref() == Some(host))
                    .map(|p| p.id.clone())
                    .collect();
                self.sessions.retain(|s| s.host != host);
                self.sessions
                    .extend(
                        rows.into_iter()
                            .filter(|s| s.source != "subagent")
                            .map(|mut s| {
                                s.host = host.to_string();
                                let own = format!("{host}:{}", s.project);
                                s.project = if !s.project.is_empty() && known.contains(&own) {
                                    own
                                } else {
                                    home.clone()
                                };
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
        if !host.is_empty() {
            return host;
        }
        // This Mac's agent working on a server: the project's machine.
        self.projects
            .iter()
            .find(|p| p.id == self.thread_project)
            .and_then(|p| p.machine.clone())
            .unwrap_or_else(|| crate::inserts::terminal::session::LOCAL.to_string())
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

    /// A new thread's creation, for the agent that will keep it, in the project as that agent
    /// knows it: a server's Host is its own environment (no project there).
    pub fn create_cmd(&self, profile: String, cwd: Option<String>) -> Cmd {
        let (host, id) = self.project_place(&self.thread_project);
        let remote = !host.is_empty();
        Cmd::Create {
            profile: if remote { "default".into() } else { profile },
            cwd,
            subject: self.new_subject.clone(),
            project: match (remote, id.as_str()) {
                (true, "general") => String::new(),
                (true, _) => id,
                (false, _) => self.thread_project.clone(),
            },
        }
    }

    /// The connection a project's environment is read through, and its id there.
    pub fn env_target(&self, project: &str) -> Option<(Arc<Connection>, String)> {
        let (host, id) = self.project_place(project);
        self.conn_of(&host).map(|c| (c, id))
    }
}
