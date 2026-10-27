//! The project's name, the first field of General, and the project's removal, its last. A
//! server's Host is named too (in this Mac's list of agents); this Mac's Host has no name to
//! change.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use serde_json::Value;

use super::section::{SettingsForm, Request, Section, form_actions, remove};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use crate::app::model::ProjectKey;
use bench_ui::settings_field;

pub struct NameEnv {
    /// The project's name, as its field edits it.
    pub field: Entity<InputState>,
    pub dirty: bool,
}

impl NameEnv {
    pub(super) fn new(id: &str, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx));
        on_change(&field, id, cx, |e| e.name.dirty = true);
        Self {
            field,
            dirty: false,
        }
    }
}

pub struct Name;

impl Section for Name {
    fn key(&self) -> &'static str {
        "name"
    }

    fn shown(&self, _app: &BenchApp, e: &ProjectEnv) -> bool {
        let key = ProjectKey::parse(&e.id);
        !(key.agent.is_empty() && key.is_host())
    }

    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        if !e.name.dirty {
            let name = v["project"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            e.name
                .field
                .update(cx, |s, cx| s.set_value(name, window, cx));
        }
    }

    /// A name is the project's, wherever it lives: `BenchApp::rename_project` renames it there
    /// (a server's Host in this Mac's list of agents); nothing is sent from here.
    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        _eid: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.envs.get_mut(id)?;
        let name = e.name.field.read(cx).value().trim().to_string();
        e.name.dirty = false;
        if !name.is_empty() {
            app.rename_project(id.to_string(), name, window, cx);
        }
        None
    }

    fn saved(&self, e: &mut ProjectEnv) {
        e.name.dirty = false;
    }

    fn revert(
        &self,
        app: &mut BenchApp,
        id: &str,
        _: &mut Window,
        _: &mut Context<BenchApp>,
    ) -> bool {
        if let Some(e) = app.envs.get_mut(id) {
            e.name.dirty = false;
        }
        false
    }
}

impl SettingsForm for Name {
    fn block(&self, _general: bool) -> &'static str {
        "Project"
    }

    fn form(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        let host = ProjectKey::parse(&e.id).is_host();
        let hint = if host {
            "The server's name in Bench"
        } else {
            "As the sidebar shows it"
        };
        let mut out = vec![
            settings_field(
                "Name",
                hint,
                Input::new(&e.name.field).into_any_element(),
                cx,
            )
            .into_any_element(),
        ];
        out.extend(form_actions(&e.id, "name", e.name.dirty, cx));
        Some(out)
    }
}

/// The last group of General: the project removed (asked once); an agent's Host is not removed
/// here (a server's agent leaves Bench in the gear).
pub struct Danger;

impl Section for Danger {
    fn key(&self) -> &'static str {
        "project"
    }

    fn shown(&self, _app: &BenchApp, e: &ProjectEnv) -> bool {
        !ProjectKey::parse(&e.id).is_host()
    }
}

impl SettingsForm for Danger {
    fn block(&self, _general: bool) -> &'static str {
        "Remove"
    }

    fn form(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        Some(vec![
            settings_field(
                "Remove the project",
                "Its threads are kept, outside projects",
                div()
                    .flex()
                    .justify_end()
                    .child(remove(
                        e,
                        "project",
                        String::new(),
                        "Remove the project",
                        cx,
                    ))
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        ])
    }

    /// The project goes (asked already, in its form): its documents close, a new thread starts.
    fn remove_here(
        &self,
        app: &mut BenchApp,
        id: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> bool {
        app.envs.remove(id);
        // Its documents go with it.
        let gone: Vec<_> = app
            .explorer
            .tabs
            .iter()
            .filter(|t| t.project() == id)
            .cloned()
            .collect();
        for key in gone {
            app.explorer.docs.remove(&key);
            app.close_doc(key, cx);
        }
        app.confirm_delete = Some(format!("p:{id}"));
        app.delete_project(id.to_string(), window, cx);
        app.center = crate::workspace::Center::Chat;
        app.new_session(cx);
        app.thread_project = String::new();
        true
    }
}
