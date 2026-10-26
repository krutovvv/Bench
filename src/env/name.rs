//! The project's name: a row of its block; its card renames the project or removes it. A
//! server's Host is named too (in this Mac's list of agents); this Mac's Host has no name to
//! change.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;
use serde_json::Value;

use super::section::{Card, Section, pad, project_block, remove, revert, row, save};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use crate::model::ProjectKey;

pub struct NameEnv {
    /// The project's name, as its card edits it.
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
    fn block(&self, general: bool) -> &'static str {
        project_block(general)
    }
    fn shown(&self, app: &BenchApp) -> bool {
        app.project_env.as_ref().is_some_and(|e| {
            let key = ProjectKey::parse(&e.id);
            !(key.agent.is_empty() && key.is_host())
        })
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let name = e.name.field.read(cx).value().to_string();
        vec![row(e, "name", String::new(), "Name".into(), name, false, true, cx)]
    }
    fn card(&self, app: &BenchApp, _name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let muted = cx.theme().muted_foreground;
        let host = ProjectKey::parse(&e.id).is_host();
        let mut actions = vec![save("name", e.name.dirty, cx)];
        actions.extend(revert("name", e.name.dirty, cx));
        // An agent's Host is not removed: a server's agent leaves Bench in the gear.
        if !host {
            actions.push(remove(e, "project", String::new(), "Remove the project", cx));
        }
        let note = if host {
            "The server's name in Bench; its agent is added and removed in the gear."
        } else {
            "Removing the project keeps its threads."
        };
        Some(Card::new(
            "Name",
            pad(v_flex()
                .child(Input::new(&e.name.field))
                .child(div().text_sm().text_color(muted).child(note))),
            actions,
        ))
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        if !e.name.dirty {
            let name = v["project"]["name"].as_str().unwrap_or_default().to_string();
            e.name.field.update(cx, |s, cx| s.set_value(name, window, cx));
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
