//! Projects: the sidebar's only unit. A project is a row in the agent's store (`/api/projects`);
//! a folder on disk (a codebase) is optional — without one its threads work in a folder Bench
//! keeps for it. A thread belongs to one project; its tools (a server, a host, the browser,
//! tasks, a utility) are attached inside the thread with `/` (`subjects.rs`).

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::BenchApp;
use crate::model::ProjectEntry;
use crate::rest;

impl BenchApp {
    pub fn load_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, "/api/projects"),
            |this, r, window, cx| {
                if let Ok(v) = r {
                    this.projects = v["projects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|p| ProjectEntry {
                            id: p["id"].as_str().unwrap_or_default().into(),
                            name: p["name"].as_str().unwrap_or_default().into(),
                            folder: p["folder"]
                                .as_str()
                                .filter(|f| !f.is_empty())
                                .map(str::to_string),
                            host: p["host"]
                                .as_str()
                                .filter(|h| !h.is_empty())
                                .map(str::to_string),
                            browser: p["browser"].as_bool().unwrap_or(false),
                        })
                        .collect();
                    this.host_browser = v["host_browser"].as_bool().unwrap_or(false);
                    // The remote projects' agents, connected beside this Mac's.
                    this.connect_remotes(window, cx);
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
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/projects/{id}");
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, serde_json::Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    this.projects.retain(|p| p.id != id);
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
