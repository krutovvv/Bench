//! The project's name, the first field of General, and the project's removal, its last. A
//! server's Host is named too (in this Mac's list of agents); this Mac's Host has no name to
//! change.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use serde_json::Value;

use super::section::{Section, Tab, form_actions, remove};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use crate::model::ProjectKey;
use bench_ui::settings_field;

pub struct NameEnv {
    /// The project's name, as its field edits it.
    pub field: Entity<InputState>,
    pub dirty: bool,
}

impl NameEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx));
        on_change(&field, cx, |e| e.name.dirty = true);
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
    fn tab(&self) -> Tab {
        Tab::General
    }
    fn block(&self, _general: bool) -> &'static str {
        "Project"
    }
    fn shown(&self, app: &BenchApp) -> bool {
        app.project_env.as_ref().is_some_and(|e| {
            let key = ProjectKey::parse(&e.id);
            !(key.agent.is_empty() && key.is_host())
        })
    }
    fn form(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<Vec<AnyElement>> {
        let e = app.project_env.as_ref()?;
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
        out.extend(form_actions("name", e.name.dirty, cx));
        Some(out)
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
    // Saving goes through `BenchApp::rename_project` (`env_save`): a project is renamed where it
    // lives, a server's Host in this Mac's list of agents.
    fn saved(&self, e: &mut ProjectEnv) {
        e.name.dirty = false;
    }
    fn revert(&self, app: &mut BenchApp, _: &mut Window, _: &mut Context<BenchApp>) -> bool {
        if let Some(e) = app.project_env.as_mut() {
            e.name.dirty = false;
        }
        false
    }
}

/// The last group of General: the project removed (asked once); an agent's Host is not removed
/// here (a server's agent leaves Bench in the gear).
pub struct Danger;

impl Section for Danger {
    fn key(&self) -> &'static str {
        "project"
    }
    fn tab(&self) -> Tab {
        Tab::General
    }
    fn block(&self, _general: bool) -> &'static str {
        "Remove"
    }
    fn shown(&self, app: &BenchApp) -> bool {
        app.project_env
            .as_ref()
            .is_some_and(|e| !ProjectKey::parse(&e.id).is_host())
    }
    fn form(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<Vec<AnyElement>> {
        let e = app.project_env.as_ref()?;
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
}
