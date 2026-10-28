//! The agents Bench works with (`docs/remote-projects.md`): this Mac's (key "") and the servers'
//! this Mac knows (key: the ~/.ssh/config alias; their list is this Mac's agent's, `places.rs`),
//! each installed and started over SSH and reached through a tunnel (`connection.rs`). One link
//! per agent, all alike: its typed session (`client::rpc::Client`), its REST connection once up.
//! A request goes to the agent it concerns; what an agent says unasked comes to `on_agent`.
//! Where a project or thread lives is `places.rs`'s to say.

use std::sync::Arc;

use anyhow::Result;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use protocol::Request;

use crate::app::BenchApp;
use crate::app::model::{ProjectKey, Target, ThreadRow};
use crate::client::connection::Connection;
use crate::client::rpc::{Client, Incoming};
use crate::thread::ThreadRef;

/// One agent: its session, its REST connection once up, why it is not up.
pub struct AgentLink {
    pub client: Client,
    pub conn: Option<Arc<Connection>>,
    pub error: Option<String>,
    /// The sources it runs, as its status says (None until asked).
    pub build: Option<String>,
}

impl BenchApp {
    /// Connect to the agent under `host` ("" = this Mac's), once: its session starts, and what it
    /// says comes to `on_agent`.
    pub fn connect_agent(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.agents.contains_key(&host) || self.demo {
            return;
        }
        let target = if host.is_empty() {
            Target::Local
        } else {
            Target::Ssh(host.clone())
        };
        let (tx, rx) = async_channel::unbounded::<Incoming>();
        self.agents.insert(
            host.clone(),
            AgentLink {
                client: Client::start(target, tx),
                conn: None,
                error: None,
                build: None,
            },
        );
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(incoming) = rx.recv().await {
                if this
                    .update_in(cx, |this, window, cx| {
                        this.on_agent(&host, incoming, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// Connect to the servers' agents this Mac knows (once each).
    pub fn connect_remotes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hosts: Vec<String> = self.server_agents.iter().map(|a| a.host.clone()).collect();
        for host in hosts {
            self.connect_agent(host, window, cx);
        }
    }

    /// What an agent said unasked: it came up, it went away, or an event.
    fn on_agent(&mut self, host: &str, incoming: Incoming, window: &mut Window, cx: &mut Context<Self>) {
        match incoming {
            Incoming::Connected(conn) => {
                if let Some(a) = self.agents.get_mut(host) {
                    a.conn = Some(conn);
                    a.error = None;
                }
                self.agent_up(host.to_string(), window, cx);
            }
            Incoming::Lost(why) => {
                if let Some(a) = self.agents.get_mut(host) {
                    a.conn = None;
                    a.error = Some(why.clone());
                }
                // This Mac's agent is the window's own: losing it is said out loud.
                if host.is_empty() {
                    self.toast(NotificationType::Error, why, window, cx);
                }
            }
            Incoming::Event(event) => self.on_agent_event(host, event, window, cx),
        }
        cx.notify();
    }

    /// The agent kept under `host` ("" = this Mac's): its REST connection, once up.
    pub fn conn_of(&self, host: &str) -> Option<Arc<Connection>> {
        self.agents.get(host).and_then(|a| a.conn.clone())
    }

    /// This Mac's agent.
    pub fn local_conn(&self) -> Option<Arc<Connection>> {
        self.conn_of("")
    }

    /// The agent keeping a thread (or the new one).
    pub fn thread_conn(&self, t: &ThreadRef) -> Option<Arc<Connection>> {
        self.conn_of(&self.thread_host(t))
    }

    /// Ask the agent under `host`; `done` gets its answer on the UI thread. Not connected: an
    /// error, as any other.
    pub fn request<R: Request>(
        &self,
        host: &str,
        request: R,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<R::Response>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let answer = self
            .agents
            .get(host)
            .map(|a| a.client.request(&request));
        let window = self.window;
        cx.spawn(async move |this, cx| {
            let result = match answer {
                Some(answer) => answer.await,
                None => Err(anyhow::anyhow!("the agent is not connected")),
            };
            let _ = window.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    done(this, result, window, cx);
                    cx.notify();
                })
            });
        })
        .detach();
    }

    /// Tell the agent under `host` something whose answer is only "done": a failure is a toast.
    pub fn send<R: Request>(&self, host: &str, request: R, cx: &mut Context<Self>) {
        let what = R::METHOD;
        self.request(host, request, cx, move |this, r, window, cx| {
            if let Err(e) = r {
                this.toast(NotificationType::Error, format!("{what}: {e:#}"), window, cx);
            }
        });
    }

    /// A thread's request, to the agent keeping it.
    pub fn send_at<R: Request>(&self, at: &ThreadRef, request: R, cx: &mut Context<Self>) {
        self.send(&self.thread_host(at), request, cx);
    }

    /// The agent under `host` lists its threads: the sidebar's rows of that agent are these.
    pub fn list_threads(&self, host: &str, cx: &mut Context<Self>) {
        let host = host.to_string();
        self.request(&host.clone(), protocol::ListThreads {}, cx, move |this, r, window, cx| {
            let Ok(list) = r else {
                return;
            };
            let rows = list
                .into_iter()
                .map(|t| {
                    let project = this.place_project(&host, t.project.as_deref());
                    ThreadRow::new(&host, project, t)
                })
                .collect();
            this.threads_listed(&host, rows, window, cx);
        });
    }

    /// The project a thread of the agent `host` sits in, as Bench keys projects: this Mac's own as
    /// they are ("" for its Host); a server's under the server, its Host when the agent names no
    /// project Bench knows.
    fn place_project(&self, host: &str, project: Option<&str>) -> String {
        if host.is_empty() {
            return project.unwrap_or_default().to_string();
        }
        let own = project.map(|p| ProjectKey::new(host, p).to_string());
        match own {
            Some(key) if self.projects.iter().any(|p| p.id == key) => key,
            _ => ProjectKey::host_of(host).to_string(),
        }
    }

    /// Where the agent under `host` runs, as media and attachments reach its files.
    pub fn target_of(&self, host: &str) -> Target {
        if host.is_empty() {
            Target::Local
        } else {
            Target::Ssh(host.to_string())
        }
    }

    /// The machine a thread's files are on, as media are fetched from it.
    pub fn thread_target(&self, t: &ThreadRef) -> Target {
        self.target_of(&self.thread_location(t).machine_here())
    }

    /// Where a thread's shell runs: its project's machine, or this Mac (`term::LOCAL`).
    pub fn thread_machine(&self, t: &ThreadRef) -> String {
        let here = self.thread_location(t).machine_here();
        if here.is_empty() {
            crate::inserts::terminal::session::LOCAL.to_string()
        } else {
            here
        }
    }

    /// The machine a thread's files are on when its agent works on another one: its agent is
    /// asked for them with this machine (`/api/fs?machine=`).
    pub fn files_machine(&self, t: &ThreadRef) -> Option<String> {
        self.thread_location(t).files_machine()
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

    /// An agent came up: what waited for it is asked now — its threads, the gear showing it, the
    /// servers this Mac knows and the tasks (this Mac's), a server's own projects.
    pub fn agent_up(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.center == super::Center::Providers && self.gear_agent == host && !self.pv.loaded {
            self.load_providers(window, cx);
        }
        if host.is_empty() {
            self.load_server_agents(window, cx);
            self.load_presets(window, cx);
            self.load_projects(window, cx);
            self.load_agent_settings(window, cx);
            // The folder a thread works in when none is chosen: a thread elsewhere shows its own.
            if self.default_cwd.is_none() {
                self.rest(window, cx, crate::client::rest::default_cwd, |this, r, _, _| {
                    this.default_cwd = r.ok();
                });
            }
            self.load_commands(cx);
            // Tasks live with the agent: another machine, its own tasks.
            self.restart_tasks(cx);
        } else {
            self.load_server_projects(host.clone(), window, cx);
        }
        self.list_threads(&host, cx);
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
                    crate::client::rest::get_json(&conn, "/api/status")
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

    /// Run `job` off the UI thread with this Mac's agent and its general settings ("default"),
    /// then `done` on the UI thread.
    pub(crate) fn rest<T: Send + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        job: impl FnOnce(&Connection, &str) -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        self.rest_on(String::new(), window, cx, job, done);
    }

    /// Run `job` off the UI thread with the agent under `host` and its general settings, then
    /// `done` on the UI thread.
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
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { job(&conn, crate::app::model::GENERAL_AGENT) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                done(this, result, window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Restart the agent under `host` from the sources this Bench carries (the gear's Update, for
    /// one that runs an older build): its link is dropped, and reconnecting installs the new
    /// sources (`launch::ensure_remote`). Its runs are durable: one it was in the middle of
    /// resumes.
    pub fn update_agent(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        if host.is_empty() {
            self.restart_agent(window, cx);
            return;
        }
        self.toast(
            NotificationType::Info,
            format!("Updating the agent on {host}…"),
            window,
            cx,
        );
        let target = Target::Ssh(host.clone());
        cx.spawn_in(window, async move |this, cx| {
            let _ = cx
                .background_spawn(async move { crate::client::connection::restart_agent(&target, true) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.agents.remove(&host);
                this.connect_agent(host, window, cx);
                cx.notify();
            });
        })
        .detach();
    }
}
