//! The project's soul (SYSTEM.md): what the project is and how to work in it. A row of the
//! project's block; its card is the text.

use gpui_kit::component::input::EditorState;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Card, Request, Section, code, project_block, revert, row, save};
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
    fn block(&self, general: bool) -> &'static str {
        project_block(general)
    }
    /// Its row says the soul's first line.
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let text = e.soul.text.read(cx).value().to_string();
        let first = text
            .lines()
            .map(|l| l.trim_start_matches('#').trim())
            .find(|l| !l.is_empty())
            .unwrap_or("not described yet")
            .to_string();
        let general = e.id == "general";
        vec![row(
            e,
            "soul",
            String::new(),
            "Soul".into(),
            first,
            false,
            general,
            cx,
        )]
    }
    fn card(&self, app: &BenchApp, _name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let mut actions = vec![save("soul", e.soul.dirty, cx)];
        actions.extend(revert("soul", e.soul.dirty, cx));
        Some(Card::new(
            "Soul: what the project is, how to work in it",
            code(&e.soul.text, cx),
            actions,
        ))
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
