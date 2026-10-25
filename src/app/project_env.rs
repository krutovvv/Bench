//! A project's environment as its thread's block (`env:<project id>`): its soul, MCP servers,
//! skills and accesses, edited in place (a Save appears once something changed) or through the
//! agent in the conversation under it.

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::HermesApp;
use crate::rest;

pub struct ProjectEnv {
    pub id: String,
    /// Where it lives (`~/.bench/projects/<id>`).
    pub dir: String,
    /// Its soul (SYSTEM.md): a row, opened to edit.
    pub soul: Entity<EditorState>,
    pub soul_open: bool,
    pub soul_dirty: bool,
    /// Its own Telegram bot: (@name, allowed user ids, has a token); a row, opened to edit.
    pub telegram: Option<(String, Vec<String>, bool)>,
    pub tg_open: bool,
    pub tg_token: Entity<InputState>,
    pub tg_users: Entity<InputState>,
    pub tg_dirty: bool,
    /// The project's name, editable at the block's top.
    pub name: Entity<InputState>,
    pub name_dirty: bool,
    /// MCP servers: (name, "http"/"stdio", enabled, connected, tools, error, its config as JSON).
    pub mcp: Vec<(String, String, bool, bool, u64, Option<String>, String)>,
    /// The server opened under its row, and its config being edited.
    pub mcp_open: Option<String>,
    pub mcp_json: Entity<EditorState>,
    pub mcp_dirty: bool,
    /// Its own prompts (@ in the composer), and the one opened under its row.
    pub prompts: Vec<String>,
    pub prompt_open: Option<String>,
    pub prompt_text: Entity<EditorState>,
    pub prompt_dirty: bool,
    /// Its own skills: (name, description).
    pub skills: Vec<(String, String)>,
    /// The skill opened under its row, and its SKILL.md being edited.
    pub skill_open: Option<String>,
    pub skill_text: Entity<EditorState>,
    pub skill_dirty: bool,
    /// Its accesses: (id, name, kind, address).
    pub hosts: Vec<(String, String, String, String)>,
    /// A row's remove asked once ("mcp:<name>", "skill:<name>", "host:<id>", "project"): the
    /// second click does it.
    pub armed: Option<String>,
    pub error: Option<String>,
}

/// A path segment for the agent's REST routes.
fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

impl HermesApp {
    /// A project's pencil: its environment's thread (the latest, or a new one in the project).
    pub fn open_project_env(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.load_env(&id, window, cx);
        // "general": this Mac's own environment; its thread belongs to no project.
        let project = if id == "general" { String::new() } else { id.clone() };
        self.thread_project = project.clone();
        self.open_subject(format!("env:{id}"), cx);
        self.thread_project = project;
    }

    fn new_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> ProjectEnv {
        // One editor for all three: the code editor, highlighted (Markdown, JSON), wrapped, no
        // line numbers, no room under the last line; its height follows the text.
        let editor = |language: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            let e = cx.new(|cx| EditorState::new(window, cx).line_number(true).folding(false).soft_wrap(true).scroll_beyond_last_line(Some(0)));
            e.update(cx, |s, cx| s.set_highlighter(language, cx));
            e
        };
        let soul = editor("markdown", window, cx);
        let skill_text = editor("markdown", window, cx);
        let mcp_json = editor("json", window, cx);
        let prompt_text = editor("markdown", window, cx);
        let name = cx.new(|cx| InputState::new(window, cx));
        let tg_token = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Bot token from @BotFather (kept as is when empty)"));
        let tg_users = cx.new(|cx| InputState::new(window, cx).placeholder("Telegram user ids who may write to it, comma-separated"));
        // A part typed in: its Save shows.
        fn dirty(this: &mut HermesApp, which: &str, ev: &InputEvent, cx: &mut Context<HermesApp>) {
            if let (InputEvent::Change, Some(e)) = (ev, this.project_env.as_mut()) {
                match which {
                    "soul" => e.soul_dirty = true,
                    "skill" => e.skill_dirty = true,
                    "telegram" => e.tg_dirty = true,
                    "prompt" => e.prompt_dirty = true,
                    "name" => e.name_dirty = true,
                    _ => e.mcp_dirty = true,
                }
                cx.notify();
            }
        }
        cx.subscribe(&soul, |this, _, ev, cx| dirty(this, "soul", ev, cx)).detach();
        cx.subscribe(&skill_text, |this, _, ev, cx| dirty(this, "skill", ev, cx)).detach();
        cx.subscribe(&mcp_json, |this, _, ev, cx| dirty(this, "mcp", ev, cx)).detach();
        cx.subscribe(&prompt_text, |this, _, ev, cx| dirty(this, "prompt", ev, cx)).detach();
        cx.subscribe(&name, |this, _, ev, cx| dirty(this, "name", ev, cx)).detach();
        cx.subscribe(&tg_token, |this, _, ev, cx| dirty(this, "telegram", ev, cx)).detach();
        cx.subscribe(&tg_users, |this, _, ev, cx| dirty(this, "telegram", ev, cx)).detach();
        ProjectEnv {
            id: id.to_string(),
            dir: String::new(),
            soul,
            soul_open: false,
            soul_dirty: false,
            telegram: None,
            tg_open: false,
            tg_token,
            tg_users,
            tg_dirty: false,
            name,
            name_dirty: false,
            mcp: vec![],
            mcp_open: None,
            mcp_json,
            mcp_dirty: false,
            prompts: vec![],
            prompt_open: None,
            prompt_text,
            prompt_dirty: false,
            skills: vec![],
            skill_open: None,
            skill_text,
            skill_dirty: false,
            hosts: vec![],
            armed: None,
            error: None,
        }
    }

    /// Read the environment for the block (again after the agent or a Save changed it); what is
    /// being typed is kept.
    pub fn load_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.project_env.as_ref().is_none_or(|e| e.id != id) {
            let env = self.new_env(id, window, cx);
            self.project_env = Some(env);
        }
        if self.demo {
            crate::demo::env(self, window, cx);
            return;
        }
        let Some((conn, eid)) = self.env_target(id) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/projects/{eid}/env");
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
                        apply(e, &v, window, cx);
                    }
                    Err(err) => e.error = Some(format!("{err:#}")),
                }
                cx.notify();
            },
        );
    }

    /// Save what changed in the block: "soul", "mcp" (mcp.json), "skill" (the open one).
    pub fn env_save(&mut self, what: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_ref() else {
            return;
        };
        let id = e.id.clone();
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let (method, path, body) = match what {
            "soul" => ("PUT", format!("/api/projects/{eid}/soul"), json!({"text": e.soul.read(cx).value().to_string()})),
            "mcp" => {
                let Some(name) = e.mcp_open.clone() else {
                    return;
                };
                ("PUT", format!("/api/projects/{eid}/mcp/{}/spec", enc(&name)), json!({"text": e.mcp_json.read(cx).value().to_string()}))
            }
            "prompt" => {
                let Some(name) = e.prompt_open.clone() else {
                    return;
                };
                ("PUT", format!("/api/projects/{eid}/prompts/{}", enc(&name)), json!({"text": e.prompt_text.read(cx).value().to_string()}))
            }
            // The name is the project's own row, on this Mac.
            "name" => ("PUT", format!("/api/projects/{id}"), json!({"name": e.name.read(cx).value().trim()})),
            "telegram" => {
                let token = e.tg_token.read(cx).value().trim().to_string();
                let users: Vec<String> = e.tg_users.read(cx).value().split(',').map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect();
                let mut body = json!({"allowed_users": users});
                if !token.is_empty() {
                    body["token"] = json!(token);
                }
                ("PUT", format!("/api/projects/{eid}/telegram"), body)
            }
            _ => {
                let Some(name) = e.skill_open.clone() else {
                    return;
                };
                ("PUT", format!("/api/projects/{eid}/skills/{}", enc(&name)), json!({"text": e.skill_text.read(cx).value().to_string()}))
            }
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, method, &path, &profile, body),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut() {
                        match what {
                            "soul" => e.soul_dirty = false,
                            "mcp" => e.mcp_dirty = false,
                            "telegram" => {
                                e.tg_dirty = false;
                                e.tg_token.update(cx, |s, cx| s.set_value("", window, cx));
                            }
                            "prompt" => e.prompt_dirty = false,
                            "name" => e.name_dirty = false,
                            _ => e.skill_dirty = false,
                        }
                    }
                    this.load_env(&id, window, cx);
                    if what == "name" {
                        this.load_projects(window, cx);
                    }
                    if what == "prompt" {
                        this.load_prompts(window, cx);
                    }
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// Revert: what is saved, back in the field.
    pub fn env_revert(&mut self, what: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        match what {
            "soul" => e.soul_dirty = false,
            "mcp" => e.mcp_dirty = false,
            "telegram" => {
                e.tg_dirty = false;
                e.tg_token.update(cx, |s, cx| s.set_value("", window, cx));
            }
            "prompt" => {
                e.prompt_dirty = false;
                if let Some(name) = e.prompt_open.take() {
                    return self.env_open("prompt", name, window, cx);
                }
            }
            "name" => e.name_dirty = false,
            _ => {
                e.skill_dirty = false;
                if let Some(name) = e.skill_open.take() {
                    return self.env_open_skill(name, window, cx);
                }
            }
        }
        let id = e.id.clone();
        self.load_env(&id, window, cx);
    }

    /// A row's Remove, twice: armed, then done. `what`: "mcp", "skill", "host" (out of the
    /// project), "project".
    pub fn env_remove(&mut self, what: &'static str, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let key = format!("{what}:{name}");
        if e.armed.as_deref() != Some(&key) {
            e.armed = Some(key);
            cx.notify();
            return;
        }
        e.armed = None;
        let id = e.id.clone();
        if what == "project" {
            self.project_env = None;
            self.delete_project(id, window, cx);
            self.new_session(cx);
            self.thread_project = String::new();
            return;
        }
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let path = match what {
            "mcp" => format!("/api/projects/{eid}/mcp/{}", enc(&name)),
            "skill" => format!("/api/projects/{eid}/skills/{}", enc(&name)),
            "prompt" => format!("/api/projects/{eid}/prompts/{}", enc(&name)),
            _ => format!("/api/projects/{eid}/hosts/{}", enc(&name)),
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    this.load_env(&id, window, cx);
                    this.load_remote_hosts(window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// An MCP server's or a prompt's row: its text opens under it to edit (again: closes).
    pub fn env_open(&mut self, what: &'static str, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let (open, editor, dirty) = if what == "mcp" { (&mut e.mcp_open, &e.mcp_json, &mut e.mcp_dirty) } else { (&mut e.prompt_open, &e.prompt_text, &mut e.prompt_dirty) };
        if open.as_deref() == Some(&name) && !*dirty {
            *open = None;
            cx.notify();
            return;
        }
        *open = Some(name.clone());
        *dirty = false;
        let editor = editor.clone();
        if what == "mcp" {
            let spec = e.mcp.iter().find(|m| m.0 == name).map(|m| m.6.clone()).unwrap_or_default();
            editor.update(cx, |s, cx| s.set_value(spec, window, cx));
            cx.notify();
            return;
        }
        let pid = e.id.clone();
        let Some((conn, eid)) = self.env_target(&pid) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/projects/{eid}/prompts");
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |_, r, window, cx| {
                if let Ok(v) = r {
                    let text = v["prompts"].as_array().into_iter().flatten().find(|p| p["name"].as_str() == Some(&name)).and_then(|p| p["text"].as_str()).unwrap_or_default().to_string();
                    editor.update(cx, |s, cx| s.set_value(text, window, cx));
                }
                cx.notify();
            },
        );
    }

    /// A skill's row: its SKILL.md opens under it to read and edit (again: closes).
    pub fn env_open_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if e.skill_open.as_deref() == Some(&name) && !e.skill_dirty {
            e.skill_open = None;
            cx.notify();
            return;
        }
        e.skill_open = Some(name.clone());
        e.skill_dirty = false;
        if self.demo {
            let text = format!("---\nname: {name}\ndescription: A made-up skill of the demo.\n---\n\nWhat to do, step by step.");
            e.skill_text.update(cx, |s, cx| s.set_value(text, window, cx));
            cx.notify();
            return;
        }
        let pid = e.id.clone();
        let Some((conn, eid)) = self.env_target(&pid) else {
            return;
        };
        let path = format!("/api/projects/{eid}/skills/{}", enc(&name));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    if let Some(e) = this.project_env.as_mut().filter(|e| e.skill_open.as_deref() == Some(&name)) {
                        let text = v["text"].as_str().unwrap_or_default().to_string();
                        e.skill_text.update(cx, |s, cx| s.set_value(text, window, cx));
                        e.skill_dirty = false;
                    }
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A section's "+ Add": the start of the request in the composer, the caret after it and the
    /// thread scrolled down to it, for the person to finish and the agent to do.
    pub fn ask_in_composer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let end = gpui_kit::component::input::Position::new(0, text.chars().count() as u32);
        self.composer.update(cx, |s, cx| {
            s.set_value(text.to_string(), window, cx);
            s.set_cursor_position(end, window, cx);
        });
        self.jump_to_latest(cx);
    }
}

/// The agent's answer into the block; fields being typed in keep their text.
pub fn apply(e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
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
                m["spec"].as_str().unwrap_or("{}").to_string(),
            )
        })
        .collect();
    e.skills = v["skills"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| (k["name"].as_str().unwrap_or_default().to_string(), k["description"].as_str().unwrap_or_default().to_string()))
        .collect();
    e.hosts = v["hosts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|h| {
            let s = |k: &str| h[k].as_str().unwrap_or_default().to_string();
            let at = if s("user").is_empty() { s("host") } else { format!("{}@{}", s("user"), s("host")) };
            (s("id"), s("name"), s("kind"), at)
        })
        .collect();
    e.telegram = v["telegram"].as_object().map(|t| {
        (
            t.get("username").and_then(Value::as_str).unwrap_or_default().to_string(),
            t.get("allowed_users").and_then(Value::as_array).map(|a| a.iter().filter_map(|u| u.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            t.get("has_token").and_then(Value::as_bool).unwrap_or(false),
        )
    });
    if !e.tg_dirty {
        let users = e.telegram.as_ref().map(|t| t.1.join(", ")).unwrap_or_default();
        e.tg_users.update(cx, |s, cx| s.set_value(users, window, cx));
    }
    if !e.soul_dirty {
        let text = v["soul"].as_str().unwrap_or_default().to_string();
        e.soul.update(cx, |s, cx| s.set_value(text, window, cx));
    }
    e.prompts = v["prompts"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_string)).collect();
    if !e.name_dirty {
        let name = v["project"]["name"].as_str().unwrap_or_default().to_string();
        e.name.update(cx, |s, cx| s.set_value(name, window, cx));
    }
}
