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
    pub soul: Entity<gpui_kit::component::input::TextareaState>,
    pub dirty: bool,
    /// The soul shows as text; Edit opens it in the editor.
    pub editing: bool,
    /// A row's remove asked once ("mcp:<name>", "skill:<name>"): the second click removes.
    pub armed: Option<String>,
    /// The soul shown whole (else its first lines).
    pub soul_all: bool,
    /// The skill opened to read: (name, its SKILL.md).
    pub skill_open: Option<(String, String)>,
    /// MCP servers: (name, "http"/"stdio", enabled, connected, tools, error).
    pub mcp: Vec<(String, String, bool, bool, u64, Option<String>)>,
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
        if self.project_env.as_ref().is_none_or(|e| e.id != id) {
            // Prose, like the composer: the thread's font, wrapped, no line numbers.
            let soul = cx.new(|cx| gpui_kit::component::input::TextareaState::new(window, cx).submit_on_enter(false));
            let sub = cx.subscribe(&soul, |this, _, ev: &gpui_kit::component::input::InputEvent, cx| {
                if let gpui_kit::component::input::InputEvent::Change = ev {
                    if let Some(e) = this.project_env.as_mut() {
                        e.dirty = true;
                    }
                    cx.notify();
                }
            });
            sub.detach();
            self.project_env = Some(ProjectEnv { id: id.to_string(), dir: String::new(), soul, dirty: false, editing: false, armed: None, soul_all: false, skill_open: None, mcp: vec![], skills: vec![], error: None });
        }
        // The demo's environment is made up (src/demo.rs).
        if self.demo {
            crate::demo::env(self, window, cx);
            return;
        }
        let Some(conn) = self.conn.clone() else {
            return;
        };
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
                                    m["connected"].as_bool().unwrap_or(false),
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
                        e.editing = false;
                    }
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// Edit / Cancel on the soul: Cancel puts back what is saved.
    pub fn edit_soul(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        e.editing = on;
        if on {
            e.soul.update(cx, |s, cx| s.focus(window, cx));
        } else {
            e.dirty = false;
            let id = e.id.clone();
            self.load_env(&id, window, cx);
        }
        cx.notify();
    }

    /// A server of the project on or off, or (twice: armed, then done) removed.
    pub fn env_mcp(&mut self, name: String, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(e)) = (self.conn.clone(), self.project_env.as_mut()) else {
            return;
        };
        if action == "remove" && e.armed.as_deref() != Some(&format!("mcp:{name}")) {
            e.armed = Some(format!("mcp:{name}"));
            cx.notify();
            return;
        }
        e.armed = None;
        let id = e.id.clone();
        let enabled = e.mcp.iter().find(|m| m.0 == name).is_some_and(|m| m.2);
        let path = format!("/api/projects/{id}/mcp/{}", enc(&name));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || match action {
                "remove" => rest::send_json(&conn, "DELETE", &path, &profile, serde_json::Value::Null),
                _ => rest::send_json(&conn, "PUT", &path, &profile, json!({"enabled": !enabled})),
            },
            move |this, r, window, cx| match r {
                Ok(_) => this.load_env(&id, window, cx),
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A skill of the project removed (twice: armed, then done).
    pub fn env_remove_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(e)) = (self.conn.clone(), self.project_env.as_mut()) else {
            return;
        };
        if e.armed.as_deref() != Some(&format!("skill:{name}")) {
            e.armed = Some(format!("skill:{name}"));
            cx.notify();
            return;
        }
        e.armed = None;
        let id = e.id.clone();
        let path = format!("/api/projects/{id}/skills/{}", enc(&name));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, serde_json::Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => this.load_env(&id, window, cx),
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A section's "+": the start of the request in the composer, the caret after it and the
    /// thread scrolled down to it, for the person to finish and the agent to do.
    pub fn ask_in_composer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let end = gpui_kit::component::input::Position::new(0, text.chars().count() as u32);
        self.composer.update(cx, |s, cx| {
            s.set_value(text.to_string(), window, cx);
            s.set_cursor_position(end, window, cx);
        });
        self.jump_to_latest(cx);
    }

    /// A skill's row: its text opens under it (again: closes).
    pub fn env_read_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if e.skill_open.as_ref().is_some_and(|(n, _)| *n == name) {
            e.skill_open = None;
            cx.notify();
            return;
        }
        if self.demo {
            e.skill_open = Some((name.clone(), format!("---\nname: {name}\n---\n\n# {name}\n\nA made-up skill of the demo.")));
            cx.notify();
            return;
        }
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let path = format!("/api/projects/{}/skills/{}", e.id, enc(&name));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.skill_open = Some((name.clone(), v["text"].as_str().unwrap_or_default().to_string()));
                    }
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}

/// A path segment for the agent's REST routes.
fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

