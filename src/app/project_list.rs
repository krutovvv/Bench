//! Projects: the sidebar's only unit. A project is a row in the agent's store (`/api/projects`);
//! a folder on disk (a codebase) is optional — without one its threads work in a folder Bench
//! keeps for it. A thread belongs to one project; its tools (a server, a host, the browser,
//! tasks, a utility) are attached inside the thread with `/` (`subjects.rs`).

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::json;

use super::HermesApp;
use crate::model::ProjectEntry;
use crate::rest;

/// The new project's form (the sidebar's "+").
pub struct ProjectForm {
    pub name: Entity<InputState>,
    /// A codebase on disk, if it has one.
    pub folder: Option<String>,
    /// A server whose agent the project is (an ~/.ssh/config host); None = this Mac.
    pub host: Option<String>,
}

impl HermesApp {
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
                            folder: p["folder"].as_str().filter(|f| !f.is_empty()).map(str::to_string),
                            host: p["host"].as_str().filter(|h| !h.is_empty()).map(str::to_string),
                        })
                        .collect();
                    // The remote projects' agents, connected beside this Mac's.
                    this.connect_remotes(window, cx);
                }
                cx.notify();
            },
        );
    }

    /// The sidebar's "+": the new project's form takes the chat area.
    pub fn new_project_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = super::Panel::Chat;
        self.ssh.form = None;
        self.files_remote.form = false;
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Project name"));
        name.update(cx, |s, cx| s.focus(window, cx));
        self.project_form = Some(ProjectForm { name, folder: None, host: None });
        cx.notify();
    }

    /// The form's "Choose…": a folder on disk the project works in.
    pub fn pick_project_codebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("The project's folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let folder = path.to_string_lossy().to_string();
                if let Some(f) = this.project_form.as_mut() {
                    // An empty name takes the folder's.
                    if f.name.read(cx).value().trim().is_empty() {
                        let base = folder.rsplit('/').next().unwrap_or(&folder).to_string();
                        f.name.update(cx, |s, cx| s.set_value(base, window, cx));
                    }
                    f.folder = Some(folder);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Create the form's project and start its first thread.
    pub fn save_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(form)) = (self.conn.clone(), self.project_form.as_ref()) else {
            return;
        };
        let name = form.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.toast(NotificationType::Error, "The project needs a name".to_string(), window, cx);
            return;
        }
        let body = json!({"name": name, "folder": form.folder, "host": form.host});
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/projects", &profile, body),
            |this, r, window, cx| match r {
                Ok(v) => {
                    this.project_form = None;
                    this.load_projects(window, cx);
                    // A local project starts with its first thread; a remote one with its
                    // server agent's threads (they show once it is connected).
                    if let (Some(id), None) = (v["project"]["id"].as_str(), v["project"]["host"].as_str()) {
                        this.new_thread_in(id.to_string(), cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
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
