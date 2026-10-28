//! Projects and where they live (`docs/remote-projects.md`): the sidebar's projects, put together
//! from three sources — this Mac's agent's projects, the servers' agents this Mac knows (each one's
//! Host project), and each server's own projects — and one answer to where any of them is
//! (`location`): which agent runs its threads, which machine its files and commands are on, which
//! folder. The rest of Bench asks this; nothing else works it out.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::BenchApp;
use crate::app::model::{Location, ProjectEntry, ProjectKey};
use crate::client::rest;

/// A server's agent this Mac knows: its ~/.ssh/config alias and its Host project's name.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerAgent {
    pub host: String,
    pub name: String,
}

/// A project as an agent lists it (`/api/projects`), under `agent`.
pub fn entry_of(agent: &str, p: &Value) -> Option<ProjectEntry> {
    let id = p["id"]
        .as_str()
        .filter(|id| !id.is_empty() && *id != ProjectKey::HOST)?;
    let text = |k: &str| p[k].as_str().filter(|v| !v.is_empty()).map(str::to_string);
    Some(ProjectEntry {
        id: ProjectKey::new(agent, id).to_string(),
        name: p["name"].as_str().unwrap_or_default().into(),
        agent: agent.to_string(),
        folder: text("folder"),
        machine: text("machine"),
        browser: p["browser"].as_bool().unwrap_or(false),
        telegram: p["telegram"]["token"].as_str().is_some_and(|t| !t.is_empty()),
    })
}

impl BenchApp {
    /// The sidebar's projects from their sources: this Mac's, then each server's Host followed by
    /// the server's own projects.
    pub fn compose_projects(&mut self) {
        let mut all = self.places.local.clone();
        for a in &self.places.server_agents {
            all.push(ProjectEntry {
                id: ProjectKey::host_of(&a.host).to_string(),
                name: a.name.clone(),
                agent: a.host.clone(),
                ..Default::default()
            });
            all.extend(
                self.places.server_projects
                    .get(&a.host)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        self.places.projects = all;
    }

    /// The servers' agents this Mac knows (this Mac's agent keeps the list); each is connected.
    pub fn load_server_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        self.background(
            window,
            cx,
            move || rest::get(&conn, "/api/agents"),
            |this, r, window, cx| {
                let Ok(v) = r else {
                    return;
                };
                this.places.server_agents = v["agents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| {
                        Some(ServerAgent {
                            host: a["host"].as_str()?.to_string(),
                            name: a["name"].as_str().unwrap_or_default().to_string(),
                        })
                    })
                    .collect();
                // An agent no longer listed is let go: its link, its threads, its projects.
                let known: Vec<String> =
                    this.places.server_agents.iter().map(|a| a.host.clone()).collect();
                this.agents.retain(|h, _| h.is_empty() || known.contains(h));
                this.places.server_projects.retain(|h, _| known.contains(h));
                this.threads.list
                    .retain(|s| s.host.is_empty() || known.contains(&s.host));
                this.compose_projects();
                this.connect_remotes(window, cx);
                cx.notify();
            },
        );
    }

    /// A server's own projects (its agent lists them), after its Host in the sidebar.
    pub fn load_server_projects(
        &mut self,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conn) = self.conn_of(&host) else {
            return;
        };
        self.background(
            window,
            cx,
            move || rest::get(&conn, "/api/projects"),
            move |this, r, _window, cx| {
                let Ok(v) = r else {
                    return;
                };
                let list = v["projects"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| entry_of(&host, p))
                    .collect();
                this.places.server_projects.insert(host.clone(), list);
                this.compose_projects();
                // Its threads were placed before its projects were known: again.
                this.list_threads(&host, cx);
                cx.notify();
            },
        );
    }

    /// A project just made on a server: in the sidebar at once.
    pub fn add_server_project(&mut self, entry: ProjectEntry) {
        self.places.server_projects
            .entry(entry.agent.clone())
            .or_default()
            .push(entry);
        self.compose_projects();
    }

    /// The server has an agent this Mac knows: projects can be made there.
    pub fn has_agent_on(&self, host: &str) -> bool {
        self.places.server_agents.iter().any(|a| a.host == host)
    }

    /// Where a project lives. A key not (yet) listed is placed by itself: its agent.
    pub fn location(&self, project: &str) -> Location {
        match self.places.projects.iter().find(|p| p.id == project) {
            Some(p) => p.location(),
            None => Location {
                agent: ProjectKey::parse(project).agent,
                ..Default::default()
            },
        }
    }

    /// A project's name as the sidebar shows it (this Mac's Host when it is none of them).
    pub fn project_name(&self, id: &str) -> String {
        self.places.projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| crate::app::LOCAL_HOST.into())
    }

    /// The open thread, as what the app does for it is told which thread it is (`ThreadRef`).
    /// Never while its view draws (`App::thread`).
    pub fn thread_ref(&self, cx: &App) -> crate::thread::ThreadRef {
        self.thread(cx).thread_ref()
    }

    /// Where a thread (or the new one) lives: its agent is the one that keeps it.
    pub fn thread_location(&self, t: &crate::thread::ThreadRef) -> Location {
        let mut at = self.location(&t.project);
        if t.id.is_some() {
            at.agent = t.host.clone();
        }
        at
    }

    /// The agent keeping a thread: "" = this Mac.
    pub fn thread_host(&self, t: &crate::thread::ThreadRef) -> String {
        self.thread_location(t).agent
    }

    /// The agent keeping a thread the sidebar lists ("" = this Mac's).
    pub fn host_of_thread(&self, id: &protocol::ThreadId) -> String {
        self.threads.list
            .iter()
            .find(|s| &s.id == id)
            .map(|s| s.host.clone())
            .unwrap_or_default()
    }

    /// The connection a project's environment is read through, and its id on that agent.
    pub fn env_target(
        &self,
        project: &str,
    ) -> Option<(
        std::sync::Arc<crate::client::connection::Connection>,
        String,
    )> {
        let key = ProjectKey::parse(project);
        self.conn_of(&key.agent).map(|c| (c, key.id))
    }

    /// A project's new name, where it lives: an agent's Host is named in the list of agents (a
    /// server's) — this Mac's Host has no name to change; any other on its agent.
    pub fn rename_project(
        &mut self,
        project: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = ProjectKey::parse(&project);
        let (conn, method, path) = match (key.agent.is_empty(), key.is_host()) {
            (true, true) => return,
            (false, true) => (
                self.local_conn(),
                "PUT",
                format!("/api/agents/{}", key.agent),
            ),
            _ => (
                self.conn_of(&key.agent),
                "PUT",
                format!("/api/projects/{}", key.id),
            ),
        };
        let Some(conn) = conn else {
            return;
        };
        for a in self
            .places.server_agents
            .iter_mut()
            .filter(|a| key.is_host() && a.host == key.agent)
        {
            a.name = name.clone();
        }
        for p in self
            .places.local
            .iter_mut()
            .chain(self.places.server_projects.values_mut().flatten())
        {
            if p.id == project {
                p.name = name.clone();
            }
        }
        self.compose_projects();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, method, &path, json!({"name": name})),
            |this, r, window, cx| {
                if let Err(e) = r {
                    this.toast(NotificationType::Error, format!("{e:#}"), window, cx);
                    this.load_projects(window, cx);
                }
            },
        );
        cx.notify();
    }
}

/// Where the sidebar's projects come from: this Mac's agent's projects, the servers' agents this
/// Mac knows and each one's own projects as it last listed them, composed into `projects`.
#[derive(Default)]
pub struct Places {
    /// The person's projects as the sidebar shows them (`compose_projects`).
    pub projects: Vec<crate::app::model::ProjectEntry>,
    pub local: Vec<crate::app::model::ProjectEntry>,
    pub server_agents: Vec<ServerAgent>,
    pub server_projects: std::collections::HashMap<String, Vec<crate::app::model::ProjectEntry>>,
    /// This Mac's agent (its Host) has the watchable server browser.
    pub host_browser: bool,
}
