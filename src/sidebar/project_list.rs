//! Projects: the sidebar's only unit. A project is a row in the agent's store (`/api/projects`);
//! a folder on disk (a codebase) is optional — without one its threads work in a folder Bench
//! keeps for it. A thread belongs to one project; its tools (a server, a host, the browser,
//! tasks, a utility) are attached inside the thread with `/` (`subjects.rs`).

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rest;

impl BenchApp {
    pub fn load_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, "/api/projects"),
            |this, r, _window, cx| {
                if let Ok(v) = r {
                    this.local_projects = v["projects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|p| crate::app::places::entry_of("", p))
                        .collect();
                    this.host_browser = v["host_browser"].as_bool().unwrap_or(false);
                    this.compose_projects();
                }
                cx.notify();
            },
        );
    }
    /// A project's "+": a new thread in it (it works in the project's folder).
    pub fn new_thread_in(&mut self, project: String, cx: &mut Context<Self>) {
        self.project_form = None;
        self.new_session(cx);
        self.new_project = None;
        self.new_subject = String::new();
        self.thread_project = project;
        // The thread exists at once ("Untitled" in the project until its first message).
        self.create_thread_now();
        cx.notify();
    }

    /// The project goes from the list; its threads stay, without a project.
    pub fn delete_project(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        // The first time asks (its head shows Remove · Cancel): its environment goes with it.
        let key = format!("p:{id}");
        if self.confirm_delete.as_deref() != Some(key.as_str()) {
            self.confirm_delete = Some(key);
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        // It goes from the agent that keeps it. An agent's Host is not removed (a server's agent
        // leaves Bench in the gear).
        let key = crate::app::model::ProjectKey::parse(&id);
        if key.is_host() {
            return;
        }
        let Some(conn) = self.conn_of(&key.agent) else {
            return;
        };
        let path = format!("/api/projects/{}", key.id);
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, serde_json::Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    this.local_projects.retain(|p| p.id != id);
                    for list in this.server_projects.values_mut() {
                        list.retain(|p| p.id != id);
                    }
                    this.compose_projects();
                    for s in this.sessions.iter_mut().filter(|s| s.project == id) {
                        s.project.clear();
                    }
                    cx.notify();
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }
}
