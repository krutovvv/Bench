//! A project's environment as its thread's blocks (`env:<project id>`): its name, soul, Telegram
//! bot, how it runs, MCP servers, skills, prompts, schedule and accesses, edited in place (a card
//! under the block, Save once something changed) or through the agent in the conversation under
//! it. Each section is a file here:
//!
//! ```text
//! env/section.rs      the `Section` trait and the pieces rows and cards are made of
//! env/view.rs         the blocks and the open card, drawn through `SECTIONS`
//! env/<section>.rs    a section: its state (a part of `ProjectEnv`), rows, card, save, actions
//! env/project_form.rs the new project's page (the sidebar's "+")
//! ```
//!
//! A new section: a file like `prompts.rs`, its state in `ProjectEnv` (and `new_env`), a line in
//! `SECTIONS` where its rows go, and its part of the agent's `/api/projects/<id>/env`.

pub mod accesses;
pub mod lsp;
pub mod mcp;
pub mod name;
pub mod place;
pub mod project_form;
pub mod prompts;
pub mod run;
pub mod schedule;
pub mod section;
pub mod skills;
mod skills_tree;
mod skills_view;
pub mod soul;
pub mod telegram;
mod view;

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::Value;

use crate::app::BenchApp;
use crate::rest;
pub use section::Section;

pub struct ProjectEnv {
    pub id: String,
    /// Where it lives (`~/.bench/projects/<id>`).
    pub dir: String,
    pub name: name::NameEnv,
    pub place: place::PlaceEnv,
    pub soul: soul::SoulEnv,
    pub lsp: lsp::LspEnv,
    pub telegram: telegram::TelegramEnv,
    pub run: run::RunEnv,
    pub mcp: mcp::McpEnv,
    pub prompts: prompts::PromptsEnv,
    pub skills: skills::SkillsEnv,
    pub hosts: accesses::AccessesEnv,
    pub cron: schedule::ScheduleEnv,
    /// A new MCP server's or prompt's name (a new skill names itself in its front matter).
    pub new_name: Entity<InputState>,
    /// What is open under the block, as the agent's answer: (kind, name) — a `Section::key`
    /// ("new" for a new one).
    pub card: Option<(&'static str, String)>,
    /// Its blocks shut by a click on their head (by title).
    pub folded: std::collections::HashSet<&'static str>,
    /// A row's remove asked once ("mcp:<name>", "skill:<name>", "host:<id>", "project"): the
    /// second click does it.
    pub armed: Option<String>,
    pub error: Option<String>,
}

/// The sections, in the order their rows are drawn; neighbours with the same `block` share it.
pub static SECTIONS: &[&dyn Section] = &[
    &name::Name,
    &place::Place,
    &soul::Soul,
    &telegram::Telegram,
    &run::Run,
    &mcp::Mcp,
    &lsp::Lsp,
    &skills::Skills,
    &prompts::Prompts,
    &schedule::Schedule,
    &accesses::Machines,
    &accesses::Accesses,
];

/// The section whose key is `key`.
pub fn section(key: &str) -> Option<&'static dyn Section> {
    SECTIONS.iter().copied().find(|s| s.key() == key)
}

/// The agent a project's threads run with: its own (`project:<id>`), or this machine's for the
/// Host project. Its model, reasoning level and approvals live in that agent's folder, so a
/// project thinks and asks as it was set to, not as the machine was.
pub(crate) fn env_agent(id: &str) -> String {
    if id == "general" {
        "default".into()
    } else {
        format!("project:{id}")
    }
}

/// A path segment for the agent's REST routes.
pub(crate) fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

/// The code editor every text of the environment is edited in: highlighted (Markdown, JSON),
/// wrapped, line numbers, no room under the last line; its height follows the text.
pub(crate) fn editor(
    language: &'static str,
    window: &mut Window,
    cx: &mut Context<BenchApp>,
) -> Entity<EditorState> {
    let e = cx.new(|cx| {
        EditorState::new(window, cx)
            .line_number(true)
            .soft_wrap(true)
            .scroll_beyond_last_line(Some(0))
    });
    e.update(cx, |s, cx| s.set_highlighter(language, cx));
    e
}

/// A part typed in: `mark` sets its dirty flag, so its Save shows.
pub(crate) fn on_change<T: EventEmitter<InputEvent>>(
    field: &Entity<T>,
    cx: &mut Context<BenchApp>,
    mark: fn(&mut ProjectEnv),
) {
    cx.subscribe(field, move |this, _, ev: &InputEvent, cx| {
        if let (InputEvent::Change, Some(e)) = (ev, this.project_env.as_mut()) {
            mark(e);
            cx.notify();
        }
    })
    .detach();
}

/// Whether the project lives on a server (its agent runs there).
pub(crate) fn is_remote(_app: &BenchApp, id: &str) -> bool {
    !crate::model::ProjectKey::parse(id).agent.is_empty()
}

impl BenchApp {
    /// A project's pencil: its environment's thread (the latest, or a new one in the project).
    pub fn open_project_env(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.load_env(&id, window, cx);
        // "general": this Mac's own environment; its thread belongs to no project.
        let project = if id == "general" {
            String::new()
        } else {
            id.clone()
        };
        self.thread_project = project.clone();
        self.open_subject(format!("env:{id}"), cx);
        self.thread_project = project;
    }

    fn new_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> ProjectEnv {
        let new_name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        ProjectEnv {
            id: id.to_string(),
            dir: String::new(),
            name: name::NameEnv::new(window, cx),
            place: place::PlaceEnv::new(window, cx),
            soul: soul::SoulEnv::new(window, cx),
            lsp: lsp::LspEnv::new(window, cx),
            telegram: telegram::TelegramEnv::new(window, cx),
            run: Default::default(),
            mcp: mcp::McpEnv::new(window, cx),
            prompts: prompts::PromptsEnv::new(window, cx),
            skills: skills::SkillsEnv::new(window, cx),
            hosts: accesses::AccessesEnv::new(window, cx),
            cron: schedule::ScheduleEnv::new(window, cx),
            new_name,
            card: None,
            folded: Default::default(),
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
        // How it runs, read from the same agent its threads run with.
        let conn2 = conn.clone();
        let agent = env_agent(&eid);
        let asked2 = asked.clone();
        self.background(
            window,
            cx,
            move || rest::profile_defaults(&conn2, &agent),
            move |this, r, _window, cx| {
                let Ok((effort, _, _, approvals, _)) = r else {
                    return;
                };
                let Some(e) = this.project_env.as_mut().filter(|e| e.id == asked2) else {
                    return;
                };
                e.run.effort = effort.unwrap_or_default();
                e.run.approvals = approvals;
                cx.notify();
            },
        );
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| {
                // The name is the sidebar's: a server's Host is named in this Mac's list of agents,
                // not by the server (`places.rs`).
                let name = this.projects.iter().find(|p| p.id == asked).map(|p| p.name.clone());
                let Some(e) = this.project_env.as_mut().filter(|e| e.id == asked) else {
                    return;
                };
                match r {
                    Ok(mut v) => {
                        e.error = None;
                        if let Some(name) = name {
                            v["project"]["name"] = Value::String(name);
                        }
                        apply(e, &v, window, cx);
                    }
                    Err(err) => e.error = Some(format!("{err:#}")),
                }
                cx.notify();
            },
        );
    }

    /// Save what changed in a section's card (`what`: its key).
    pub fn env_save(&mut self, what: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(section) = section(what) else {
            return;
        };
        let Some(id) = self.project_env.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        // A name is the project's, wherever it lives (`places.rs` knows where).
        if what == "name" {
            if let Some(e) = self.project_env.as_mut() {
                let name = e.name.field.read(cx).value().trim().to_string();
                e.name.dirty = false;
                if !name.is_empty() {
                    self.rename_project(id, name, window, cx);
                }
            }
            return;
        }
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let Some((method, path, body)) = section.save(self, &id, &eid, window, cx) else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, method, &path, &profile, body),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut() {
                        // A new one is made: its form closes, it is in the list.
                        if e.card.as_ref().is_some_and(|(_, n)| n == "new") && what != "cron" {
                            e.card = None;
                            e.mcp.open = None;
                            e.skills.open = None;
                            e.prompts.open = None;
                        }
                        section.saved(e);
                    }
                    this.load_env(&id, window, cx);
                    section.reload(this, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// Revert: what is saved, back in the field.
    pub fn env_revert(&mut self, what: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(section) = section(what) else {
            return;
        };
        if self.project_env.is_none() || section.revert(self, window, cx) {
            return;
        }
        let Some(id) = self.project_env.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        self.load_env(&id, window, cx);
    }

    /// A row's Remove, twice: armed, then done. `what`: a section's key ("mcp", "skill",
    /// "prompt", "cron", "host"), or "project" (the project itself).
    pub fn env_remove(
        &mut self,
        what: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let Some(path) = section(what).and_then(|s| s.remove_path(&eid, &name)) else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    // What was removed had its card open.
                    if let Some(e) = this.project_env.as_mut() {
                        e.card = None;
                    }
                    this.load_env(&id, window, cx);
                    this.load_remote_hosts(window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A row's click: its card opens under the block (again: it closes); one card at a time.
    pub fn env_card(
        &mut self,
        kind: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let again = e
            .card
            .as_ref()
            .is_some_and(|(k, n)| *k == kind && *n == name);
        for s in SECTIONS {
            s.close(e);
        }
        e.armed = None;
        if again {
            e.card = None;
            cx.notify();
            return;
        }
        e.card = Some((kind, name.clone()));
        let sec = section(kind);
        // A new one: an empty form, its text pasted (or written), then Save.
        if name == "new" {
            if let Some((editor, template)) = sec.and_then(|s| s.new_one(e)) {
                editor.update(cx, |s, cx| s.set_value(template, window, cx));
                e.new_name.update(cx, |s, cx| s.set_value("", window, cx));
                e.new_name.update(cx, |s, cx| s.focus(window, cx));
                cx.notify();
                return;
            }
        }
        if let Some(s) = sec {
            s.open(self, name, window, cx);
        }
        cx.notify();
    }

    /// A folder or file shown in Finder (this Mac's projects only).
    pub fn reveal_in_finder(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
        {
            self.toast(NotificationType::Error, format!("{err:#}"), window, cx);
        }
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

/// The agent's answer into the blocks; fields being typed in keep their text.
pub fn apply(e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
    e.dir = v["dir"].as_str().unwrap_or_default().to_string();
    for s in SECTIONS {
        s.apply(e, v, window, cx);
    }
}
