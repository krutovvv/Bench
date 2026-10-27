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
use crate::client::rpc::Cmd;

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
    })
}

impl BenchApp {
    /// The sidebar's projects from their sources: this Mac's, then each server's Host followed by
    /// the server's own projects.
    pub fn compose_projects(&mut self) {
        let mut all = self.local_projects.clone();
        for a in &self.server_agents {
            all.push(ProjectEntry {
                id: ProjectKey::host_of(&a.host).to_string(),
                name: a.name.clone(),
                agent: a.host.clone(),
                ..Default::default()
            });
            all.extend(
                self.server_projects
                    .get(&a.host)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        self.projects = all;
    }

    /// The servers' agents this Mac knows (this Mac's agent keeps the list); each is connected.
    pub fn load_server_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        self.background(
            window,
            cx,
            move || rest::get_json(&conn, "/api/agents"),
            |this, r, window, cx| {
                let Ok(v) = r else {
                    return;
                };
                this.server_agents = v["agents"]
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
                    this.server_agents.iter().map(|a| a.host.clone()).collect();
                this.agents.retain(|h, _| h.is_empty() || known.contains(h));
                this.server_projects.retain(|h, _| known.contains(h));
                this.sessions
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
            move || rest::get_json(&conn, "/api/projects"),
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
                this.server_projects.insert(host.clone(), list);
                this.compose_projects();
                // Its threads were placed before its projects were known: again.
                if let Some(a) = this.agents.get(&host) {
                    a.gateway.send(Cmd::Sessions("default".into()));
                }
                cx.notify();
            },
        );
    }

    /// A project just made on a server: in the sidebar at once.
    pub fn add_server_project(&mut self, entry: ProjectEntry) {
        self.server_projects
            .entry(entry.agent.clone())
            .or_default()
            .push(entry);
        self.compose_projects();
    }

    /// The server has an agent this Mac knows: projects can be made there.
    pub fn has_agent_on(&self, host: &str) -> bool {
        self.server_agents.iter().any(|a| a.host == host)
    }

    /// Where a project lives. A key not (yet) listed is placed by itself: its agent.
    pub fn location(&self, project: &str) -> Location {
        match self.projects.iter().find(|p| p.id == project) {
            Some(p) => p.location(),
            None => Location {
                agent: ProjectKey::parse(project).agent,
                ..Default::default()
            },
        }
    }

    /// A project's name as the sidebar shows it (this Mac's Host when it is none of them).
    pub fn project_name(&self, id: &str) -> String {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| crate::app::LOCAL_HOST.into())
    }

    /// The open thread's project as the Explorer and the environment name it: its own, or the
    /// Host of this Mac when it has none.
    pub fn thread_project_or_host(&self) -> String {
        if self.active_thread.thread_project.is_empty() {
            ProjectKey::HOST.to_string()
        } else {
            self.active_thread.thread_project.clone()
        }
    }

    /// Where the open thread (or the new one) lives: its agent is the one that keeps it.
    pub fn thread_location(&self) -> Location {
        let mut at = self.location(&self.active_thread.thread_project);
        if let Some(id) = &self.active_thread.session {
            at.agent = self.host_of_session(id);
        }
        at
    }

    /// The agent keeping the open thread: "" = this Mac.
    pub fn thread_host(&self) -> String {
        self.thread_location().agent
    }

    pub fn host_of_session(&self, id: &str) -> String {
        self.sessions
            .iter()
            .find(|s| s.id == id)
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

    /// A new thread's creation, for the agent that will keep it, in the project as that agent
    /// knows it (an agent's Host is its own environment: no project there).
    pub fn create_cmd(&self, profile: String, cwd: Option<String>) -> Cmd {
        let key = ProjectKey::parse(&self.active_thread.thread_project);
        let remote = !key.agent.is_empty();
        Cmd::Create {
            profile: if remote { "default".into() } else { profile },
            cwd,
            subject: self.new_subject.clone(),
            project: if key.is_host() || key.id.is_empty() {
                String::new()
            } else {
                key.id
            },
        }
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
            .server_agents
            .iter_mut()
            .filter(|a| key.is_host() && a.host == key.agent)
        {
            a.name = name.clone();
        }
        for p in self
            .local_projects
            .iter_mut()
            .chain(self.server_projects.values_mut().flatten())
        {
            if p.id == project {
                p.name = name.clone();
            }
        }
        self.compose_projects();
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, method, &path, &profile, json!({"name": name})),
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
