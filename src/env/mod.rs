//! A project's environment: what its agent works with. Its settings (a document of the Explorer,
//! `page.rs`) holds the fields — name, where it lives, how it runs, its Telegram bot; the
//! Explorer (`src/explorer/`) holds what it contains — its soul, prompts and skills as files,
//! its MCP and language servers and scheduled jobs as parts opened in the middle. The agent
//! changes all of it too, from any thread, with its tools. Each section is a file here:
//!
//! ```text
//! env/section.rs      the `Section` trait, `Home`, `Item`, and the pieces sections are made of
//! env/page.rs         the Settings page: the `Home::Settings` sections' forms
//! env/instructions.rs the prompts and skills the environment lists (their texts are files)
//! env/<section>.rs    a section: its state (a part of `ProjectEnv`), form or items, card, save
//! env/project_form.rs the new project's page (the sidebar's "+")
//! ```
//!
//! A new section: a file like `telegram.rs`, its state in `ProjectEnv` (and `new_env`), a line in
//! `SECTIONS` (its `home`), and its part of the agent's `/api/projects/<id>/env`.

pub mod instructions;
pub mod lsp;
pub mod mcp;
pub mod name;
mod page;
pub mod place;
pub mod project_form;
pub mod run;
pub mod schedule;
pub mod section;
pub mod telegram;

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::Value;

use crate::app::BenchApp;
use crate::rest;
pub use section::{Home, Section};

pub struct ProjectEnv {
    pub id: String,
    /// Where it lives (`~/.bench/projects/<id>`).
    pub dir: String,
    pub name: name::NameEnv,
    pub place: place::PlaceEnv,
    pub lsp: lsp::LspEnv,
    pub telegram: telegram::TelegramEnv,
    pub run: run::RunEnv,
    pub mcp: mcp::McpEnv,
    pub instructions: instructions::InstructionsEnv,
    pub cron: schedule::ScheduleEnv,
    /// A new MCP server's name.
    pub new_name: Entity<InputState>,
    /// The part open in the middle (the Explorer's `DocKey::Part` of this project): (kind,
    /// name), the kind a `Section::key` (name "new" for a new one). One per project: its fields
    /// are its section's.
    pub card: Option<(&'static str, String)>,
    /// A remove asked once ("mcp:<name>", "project:"): the second click does it.
    pub armed: Option<String>,
    pub error: Option<String>,
}

/// The sections, in the order they are drawn where they live; neighbours with the same `block`
/// share its heading.
pub static SECTIONS: &[&dyn Section] = &[
    &name::Name,
    &place::Place,
    &run::Run,
    &telegram::Telegram,
    &name::Danger,
    &mcp::Mcp,
    &lsp::Lsp,
    &schedule::Schedule,
];

/// The section whose key is `key`.
pub fn section(key: &str) -> Option<&'static dyn Section> {
    SECTIONS.iter().copied().find(|s| s.key() == key)
}

/// The agent a project's threads run with: its own (`project:<id>`), or this machine's for the
/// Host project. Its model, reasoning level and approvals live in that agent's folder, so a
/// project thinks and asks as it was set to, not as the machine was.
pub(crate) fn env_agent(id: &str) -> String {
    if id == crate::model::ProjectKey::HOST {
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
    // A fold opened or closed changes how tall it is: the view around it is drawn again.
    cx.observe(&e, |_, _, cx| cx.notify()).detach();
    e
}

/// A field of the project `id`'s environment typed in: `mark` sets its dirty flag, so its Save
/// shows.
pub(crate) fn on_change<T: EventEmitter<InputEvent>>(
    field: &Entity<T>,
    id: &str,
    cx: &mut Context<BenchApp>,
    mark: fn(&mut ProjectEnv),
) {
    let id = id.to_string();
    cx.subscribe(field, move |this, _, ev: &InputEvent, cx| {
        if let (InputEvent::Change, Some(e)) = (ev, this.envs.get_mut(&id)) {
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
    /// A project's settings: the Explorer on it, its settings open as a document.
    pub fn open_project_settings(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_explorer(id.clone(), window, cx);
        self.open_doc(
            crate::explorer::DocKey::Settings { project: id },
            window,
            cx,
        );
    }

    /// A part of the project `project`'s environment opened in the middle (`DocKey::Part`).
    pub fn open_part(
        &mut self,
        project: String,
        kind: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = crate::explorer::DocKey::Part {
            project,
            kind,
            name,
        };
        self.open_doc(key, window, cx);
    }

    fn new_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> ProjectEnv {
        let new_name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        ProjectEnv {
            id: id.to_string(),
            dir: String::new(),
            name: name::NameEnv::new(id, window, cx),
            place: place::PlaceEnv::new(id, window, cx),
            lsp: lsp::LspEnv::new(id, window, cx),
            telegram: telegram::TelegramEnv::new(id, window, cx),
            run: Default::default(),
            mcp: mcp::McpEnv::new(id, window, cx),
            instructions: Default::default(),
            cron: schedule::ScheduleEnv::new(id, window, cx),
            new_name,
            card: None,
            armed: None,
            error: None,
        }
    }

    /// Read the project `id`'s environment (again after the agent or a Save changed it) into its
    /// entry of `envs`, made the first time; what is being typed is kept, other projects' entries
    /// are not touched.
    pub fn load_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.envs.contains_key(id) {
            let env = self.new_env(id, window, cx);
            self.envs.insert(id.to_string(), env);
        }
        if self.demo {
            crate::demo::env(self, id, window, cx);
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
                let Some(e) = this.envs.get_mut(&asked2) else {
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
                let name = this
                    .projects
                    .iter()
                    .find(|p| p.id == asked)
                    .map(|p| p.name.clone());
                let Some(e) = this.envs.get_mut(&asked) else {
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

    /// Save what changed in a section of the project `id` (`what`: the section's key).
    pub fn env_save(
        &mut self,
        id: &str,
        what: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = section(what) else {
            return;
        };
        if !self.envs.contains_key(id) {
            return;
        }
        // The project as its agent knows it (`env_target`'s); a section that saves elsewhere
        // (the name) needs no link to the agent.
        let eid = crate::model::ProjectKey::parse(id).id;
        let Some((method, path, body)) = section.save(self, id, &eid, window, cx) else {
            return;
        };
        let Some((conn, _)) = self.env_target(id) else {
            return;
        };
        let profile = self.current_profile();
        let id = id.to_string();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, method, &path, &profile, body),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    // A new one is made: its form's tab gives way to it when it has a name of its
                    // own (an MCP server is named in its form; a job keeps its form).
                    let mut made = None;
                    if let Some(e) = this.envs.get_mut(&id) {
                        let new = e
                            .card
                            .as_ref()
                            .is_some_and(|(k, n)| *k == what && n == "new");
                        if let Some(name) = new.then(|| section.made(e, cx)).flatten() {
                            section.close(e);
                            e.card = None;
                            made = Some(name);
                        }
                        section.saved(e);
                    }
                    if let Some(name) = made {
                        let new = crate::explorer::DocKey::Part {
                            project: id.clone(),
                            kind: what,
                            name: "new".into(),
                        };
                        this.close_doc(new, cx);
                        if !name.is_empty() {
                            this.open_part(id.clone(), what, name, window, cx);
                        }
                    }
                    this.load_env(&id, window, cx);
                    section.reload(this, &id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// Revert in the project `id`: what is saved, back in the field.
    pub fn env_revert(
        &mut self,
        id: &str,
        what: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = section(what) else {
            return;
        };
        if !self.envs.contains_key(id) || section.revert(self, id, window, cx) {
            return;
        }
        self.load_env(id, window, cx);
    }

    /// A Remove in the project `id`, twice: armed, then done. `what`: a section's key ("mcp",
    /// "cron", "project": the project itself, which its section removes here).
    pub fn env_remove(
        &mut self,
        id: &str,
        what: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.envs.get_mut(id) else {
            return;
        };
        let key = format!("{what}:{name}");
        if e.armed.as_deref() != Some(&key) {
            e.armed = Some(key);
            cx.notify();
            return;
        }
        e.armed = None;
        let Some(section) = section(what) else {
            return;
        };
        if section.remove_here(self, id, window, cx) {
            return;
        }
        let Some((conn, eid)) = self.env_target(id) else {
            return;
        };
        let Some(path) = section.remove_path(&eid, &name) else {
            return;
        };
        let profile = self.current_profile();
        let id = id.to_string();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    // What was removed was open: its tab closes.
                    let gone = crate::explorer::DocKey::Part {
                        project: id.clone(),
                        kind: what,
                        name,
                    };
                    this.close_doc(gone, cx);
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A part of the project `id` made its open one: what had another open shuts, its section
    /// fills its fields ("new": an empty form, its text to paste or write, then Save). The caller
    /// (`open_doc`) has made sure no unsaved draft is lost.
    pub fn env_card(
        &mut self,
        id: &str,
        kind: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.envs.get_mut(id) else {
            return;
        };
        if e.card
            .as_ref()
            .is_some_and(|(k, n)| *k == kind && *n == name)
        {
            return;
        }
        for s in SECTIONS {
            s.close(e);
        }
        e.armed = None;
        e.card = Some((kind, name.clone()));
        let sec = section(kind);
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
            s.open(self, id, name, window, cx);
        }
        cx.notify();
    }

    /// The open part of the project `id`, when its draft is not saved: its section.
    pub fn env_dirty_card(&self, id: &str) -> Option<(&'static dyn Section, String)> {
        let e = self.envs.get(id)?;
        let (kind, name) = e.card.clone()?;
        let s = section(kind).filter(|s| s.dirty(e))?;
        Some((s, name))
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
}

/// The agent's answer into the sections; fields being typed in keep their text.
pub fn apply(e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
    e.dir = v["dir"].as_str().unwrap_or_default().to_string();
    e.instructions.apply(v);
    for s in SECTIONS {
        s.apply(e, v, window, cx);
    }
}
