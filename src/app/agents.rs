//! The agents Bench works with (`docs/remote-projects.md`): this Mac's (key "") and the servers'
//! this Mac knows (key: the ~/.ssh/config alias; their list is this Mac's agent's, `places.rs`),
//! each installed and started over SSH and reached through a tunnel (`connection.rs`). One list of
//! links: a command goes to the agent it concerns, a REST call through that agent's connection.
//! Where a project or thread lives is `places.rs`'s to say.

use std::sync::Arc;

use gpui_kit::*;

use super::BenchApp;
use crate::connection::Connection;
use crate::gateway::{Cmd, Evt, Gateway};
use crate::model::{ProjectKey, Target};

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

    /// The agent keeping the open thread (or the new one).
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

    /// The machine the open thread's files are on, as media are fetched from it.
    pub fn thread_target(&self) -> Target {
        self.target_of(&self.thread_location().machine_here())
    }

    /// Where the open thread's shell runs: its project's machine, or this Mac (`term::LOCAL`).
    pub fn thread_machine(&self) -> String {
        let here = self.thread_location().machine_here();
        if here.is_empty() {
            crate::inserts::terminal::session::LOCAL.to_string()
        } else {
            here
        }
    }

    /// The machine the open thread's files are on when its agent works on another one: its agent
    /// is asked for them with this machine (`/api/fs?machine=`).
    pub fn files_machine(&self) -> Option<String> {
        self.thread_location().files_machine()
    }

    /// The agents in the order the gear lists them: this Mac's, then the servers' as listed.
    pub fn agent_hosts(&self) -> Vec<String> {
        std::iter::once(String::new())
            .chain(self.server_agents.iter().map(|a| a.host.clone()))
            .collect()
    }

    /// An agent as people call it: "This Mac", or its server's alias.
    pub fn agent_name(host: &str) -> String {
        if host.is_empty() {
            "This Mac".into()
        } else {
            host.to_string()
        }
    }

    /// An agent came up: what waited for it is asked now — the gear showing it, the servers this
    /// Mac knows (this Mac's), a server's own projects.
    pub fn agent_up(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel == super::Panel::Providers && self.gear_agent == host && !self.pv.loaded {
            self.load_providers(window, cx);
        }
        if host.is_empty() {
            self.load_server_agents(window, cx);
            self.load_presets(window, cx);
        } else {
            self.load_server_projects(host.clone(), window, cx);
        }
        self.ask_build(host, cx);
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

    /// Restart the agent under `host` from the sources this Bench carries (the gear's Update, for
    /// one that runs an older build). A server's is stopped and its link dropped: reconnecting
    /// installs the new sources there (`pi_agent::ensure_remote`). Its runs are durable: one
    /// it was in the middle of resumes.
    pub fn update_agent(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        if host.is_empty() {
            self.restart_gateway(window, cx);
            return;
        }
        self.toast(
            gpui_kit::component::notification::NotificationType::Info,
            format!("Updating the agent on {host}…"),
            window,
            cx,
        );
        let target = Target::Ssh(host.clone());
        cx.spawn_in(window, async move |this, cx| {
            let _ = cx
                .background_spawn(async move { crate::connection::restart_gateway(&target, true) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.agents.remove(&host);
                this.connect_remotes(window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Connect to the servers' agents this Mac knows (once each).
    pub fn connect_remotes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hosts: Vec<String> = self.server_agents.iter().map(|a| a.host.clone()).collect();
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
            // The server's agent has no profiles to pick: its threads are its projects'.
            Evt::Profiles(_) | Evt::Catalog(_) => {}
            Evt::Sessions(_, rows) => {
                // A thread of one of the server's projects is that project's; the rest its Host's.
                let home = ProjectKey::host_of(host).to_string();
                let known: Vec<String> = self
                    .projects
                    .iter()
                    .filter(|p| p.agent == host)
                    .map(|p| p.id.clone())
                    .collect();
                self.sessions.retain(|s| s.host != host);
                self.sessions
                    .extend(
                        rows.into_iter()
                            .filter(|s| s.source != "subagent")
                            .map(|mut s| {
                                s.host = host.to_string();
                                let own = ProjectKey::new(host, &s.project).to_string();
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

    /// Send a command to the agent it concerns: the thread's own (a server's, or this Mac's).
    pub fn route(&self, cmd: Cmd) {
        let host = match &cmd {
            Cmd::Sessions(_) | Cmd::Catalog { .. } => String::new(),
            Cmd::Delete { session, .. } | Cmd::Rename { session, .. } => {
                self.host_of_session(session)
            }
            _ => self.thread_host(),
        };
        if let Some(a) = self.agents.get(&host) {
            a.gateway.send(cmd);
        }
    }
}
