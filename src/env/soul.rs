//! The project's soul (SYSTEM.md): what the project is and how to work in it. The first text of
//! Instructions (`ide.rs`).

use gpui_kit::component::input::EditorState;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Request, Section, Tab};
use super::{ProjectEnv, editor, on_change};
use crate::app::BenchApp;

pub struct SoulEnv {
    pub text: Entity<EditorState>,
    pub open: bool,
    pub dirty: bool,
}

impl SoulEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let text = editor("markdown", window, cx);
        on_change(&text, cx, |e| e.soul.dirty = true);
        Self {
            text,
            open: false,
            dirty: false,
        }
    }
}

pub struct Soul;

impl Section for Soul {
    fn key(&self) -> &'static str {
        "soul"
    }
    fn tab(&self) -> Tab {
        Tab::Instructions
    }
    fn block(&self, _general: bool) -> &'static str {
        "Instructions"
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        if !e.soul.dirty {
            let text = v["soul"].as_str().unwrap_or_default().to_string();
            e.soul
                .text
                .update(cx, |s, cx| s.set_value(text, window, cx));
        }
    }
    fn close(&self, e: &mut ProjectEnv) {
        e.soul.open = false;
    }
    fn open(&self, app: &mut BenchApp, _: String, _: &mut Window, _: &mut Context<BenchApp>) {
        if let Some(e) = app.project_env.as_mut() {
            e.soul.open = true;
        }
    }
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        Some((
            "PUT",
            format!("/api/projects/{eid}/soul"),
            json!({"text": e.soul.text.read(cx).value().to_string()}),
        ))
    }
    fn saved(&self, e: &mut ProjectEnv) {
        e.soul.dirty = false;
    }
    fn revert(&self, app: &mut BenchApp, _: &mut Window, _: &mut Context<BenchApp>) -> bool {
        if let Some(e) = app.project_env.as_mut() {
            e.soul.dirty = false;
        }
        false
    }
}
