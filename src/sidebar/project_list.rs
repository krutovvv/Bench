//! Projects: the sidebar's only unit. A project is a row in the agent's store (`/api/projects`);
//! a folder on disk (a codebase) is optional — without one its threads work in a folder Bench
//! keeps for it. A thread belongs to one project; its tools (a server, a host, the browser,
//! a utility) are attached inside the thread with `/` (`inserts/slash.rs`).

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rest;

impl BenchApp {
    pub fn load_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        self.background(
            window,
            cx,
            move || rest::get(&conn, "/api/projects"),
            |this, r, _window, cx| {
                if let Ok(v) = r {
                    this.places.local = v["projects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|p| crate::app::places::entry_of("", p))
                        .collect();
                    this.places.host_browser = v["host_browser"].as_bool().unwrap_or(false);
                    this.places.note_host_bot("", &v);
                    this.compose_projects();
                }
                cx.notify();
            },
        );
    }
    /// A project's "+": a new thread in it (it works in the project's folder).
    pub fn new_thread_in(&mut self, project: String, cx: &mut Context<Self>) {
        self.new_project.form = None;
        self.new_thread(cx);
        self.new_project.folder = None;
        self.active_thread
            .update(cx, |v, _| v.thread.project = project);
        // The thread exists at once ("Untitled" in the project until its first message).
        let at = self.thread_ref(cx);
        self.create_thread(&at, cx);
        cx.notify();
    }

    /// The project goes from the list; its threads stay, without a project.
    pub fn delete_project(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        // The first time asks (its head shows Remove · Cancel): its environment goes with it.
        let key = format!("p:{id}");
        if !self.armed.confirm(key) {
            cx.notify();
            return;
        }
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
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, serde_json::Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    this.places.local.retain(|p| p.id != id);
                    for list in this.places.server_projects.values_mut() {
                        list.retain(|p| p.id != id);
                    }
                    this.compose_projects();
                    for s in this.threads.list.iter_mut().filter(|s| s.project == id) {
                        s.project.clear();
                    }
                    cx.notify();
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }
}
