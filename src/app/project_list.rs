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
            |this, r, _, cx| {
                if let Ok(v) = r {
                    this.projects = v["projects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|p| ProjectEntry {
                            id: p["id"].as_str().unwrap_or_default().into(),
                            name: p["name"].as_str().unwrap_or_default().into(),
                            folder: p["folder"].as_str().filter(|f| !f.is_empty()).map(str::to_string),
                        })
                        .collect();
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
        self.project_form = Some(ProjectForm { name, folder: None });
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
        let body = json!({"name": name, "folder": form.folder});
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/projects", &profile, body),
            |this, r, window, cx| match r {
                Ok(v) => {
                    this.project_form = None;
                    this.load_projects(window, cx);
                    if let Some(id) = v["project"]["id"].as_str() {
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

/// A project's environment as its thread's block shows it (`env:<project id>`).
pub struct ProjectEnv {
    pub id: String,
    /// Where it lives (`~/.bench/projects/<id>`).
    pub dir: String,
    /// Its soul (SYSTEM.md), editable in the block.
    pub soul: Entity<gpui_kit::component::input::EditorState>,
    pub dirty: bool,
    /// MCP servers: (name, "http"/"stdio", enabled, tools, error).
    pub mcp: Vec<(String, String, bool, u64, Option<String>)>,
    /// Its own skills: (name, description).
    pub skills: Vec<(String, String)>,
    pub error: Option<String>,
}

impl HermesApp {
    /// A project's pencil: its environment's thread (the latest, or a new one in the project).
    pub fn open_project_env(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.load_env(&id, window, cx);
        self.thread_project = id.clone();
        self.open_subject(format!("env:{id}"), cx);
        self.thread_project = id;
    }

    /// Read the environment for the block (again after the agent changed it).
    pub fn load_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.conn.clone() else {
            return;
        };
        if self.project_env.as_ref().is_none_or(|e| e.id != id) {
            let soul = cx.new(|cx| {
                gpui_kit::component::input::EditorState::new(window, cx)
                    .scroll_beyond_last_line(Some(1))
                    .soft_wrap(true)
            });
            soul.update(cx, |s, cx| s.set_highlighter("markdown", cx));
            let sub = cx.subscribe(&soul, |this, _, ev: &gpui_kit::component::input::InputEvent, cx| {
                if let gpui_kit::component::input::InputEvent::Change = ev {
                    if let Some(e) = this.project_env.as_mut() {
                        e.dirty = true;
                    }
                    cx.notify();
                }
            });
            sub.detach();
            self.project_env = Some(ProjectEnv { id: id.to_string(), dir: String::new(), soul, dirty: false, mcp: vec![], skills: vec![], error: None });
        }
        let profile = self.current_profile();
        let path = format!("/api/projects/{id}/env");
        let asked = id.to_string();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| {
                let Some(e) = this.project_env.as_mut().filter(|e| e.id == asked) else {
                    return;
                };
                match r {
                    Ok(v) => {
                        e.error = None;
                        e.dir = v["dir"].as_str().unwrap_or_default().to_string();
                        e.mcp = v["mcp"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|m| {
                                (
                                    m["name"].as_str().unwrap_or_default().to_string(),
                                    m["kind"].as_str().unwrap_or_default().to_string(),
                                    m["enabled"].as_bool().unwrap_or(true),
                                    m["tools"].as_u64().unwrap_or(0),
                                    m["error"].as_str().map(str::to_string),
                                )
                            })
                            .collect();
                        e.skills = v["skills"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|k| (k["name"].as_str().unwrap_or_default().to_string(), k["description"].as_str().unwrap_or_default().to_string()))
                            .collect();
                        // The soul as saved, unless the person is typing in it.
                        if !e.dirty {
                            let text = v["soul"].as_str().unwrap_or_default().to_string();
                            e.soul.update(cx, |s, cx| s.set_value(text, window, cx));
                            e.dirty = false;
                        }
                    }
                    Err(err) => e.error = Some(format!("{err:#}")),
                }
                cx.notify();
            },
        );
    }

    /// The block's Save: the soul as typed.
    pub fn save_soul(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(e)) = (self.conn.clone(), self.project_env.as_ref()) else {
            return;
        };
        let text = e.soul.read(cx).value().to_string();
        let (id, path) = (e.id.clone(), format!("/api/projects/{}/soul", e.id));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "PUT", &path, &profile, json!({"text": text})),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut().filter(|e| e.id == id) {
                        e.dirty = false;
                    }
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}
