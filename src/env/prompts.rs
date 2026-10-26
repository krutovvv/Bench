//! The project's own prompts (@ in the composer): a row each; a card is the prompt's text.

use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{
    Card, Request, Section, add, code, empty, new_card, remove, revert, row, save,
};
use super::{ProjectEnv, editor, enc, on_change};
use crate::app::BenchApp;
use crate::rest;

pub struct PromptsEnv {
    pub names: Vec<String>,
    /// The one opened under its row, and its text.
    pub open: Option<String>,
    pub text: Entity<EditorState>,
    pub dirty: bool,
}

impl PromptsEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let text = editor("markdown", window, cx);
        on_change(&text, cx, |e| e.prompts.dirty = true);
        Self {
            names: vec![],
            open: None,
            text,
            dirty: false,
        }
    }
}

pub struct Prompts;

impl Section for Prompts {
    fn key(&self) -> &'static str {
        "prompt"
    }
    fn block(&self, _general: bool) -> &'static str {
        "Prompts"
    }
    fn head(&self, _app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        Some(add("env-prompt-add", "prompt", "", cx).into_any_element())
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let mut rows = vec![];
        if e.prompts.names.is_empty() {
            rows.push(empty(
                "None of its own: @ in the composer offers the general ones.",
                cx,
            ));
        }
        for (k, name) in e.prompts.names.iter().cloned().enumerate() {
            rows.push(row(
                e,
                "prompt",
                name.clone(),
                name,
                String::new(),
                false,
                k == 0,
                cx,
            ));
        }
        rows
    }
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        if name == "new" {
            return Some(new_card(
                e,
                "prompt",
                "New prompt: its name, and its text",
                "Write a prompt for this project: ",
                &e.prompts.text,
                true,
                cx,
            ));
        }
        if !e.prompts.names.iter().any(|n| n == name) {
            return None;
        }
        let mut actions = vec![save("prompt", e.prompts.dirty, cx)];
        actions.extend(revert("prompt", e.prompts.dirty, cx));
        actions.push(remove(e, "prompt", name.to_string(), "Remove", cx));
        Some(Card::new(
            format!("Prompt · {name}"),
            code(&e.prompts.text, cx),
            actions,
        ))
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, _window: &mut Window, _cx: &mut App) {
        e.prompts.names = v["prompts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect();
    }
    fn close(&self, e: &mut ProjectEnv) {
        e.prompts.open = None;
    }
    fn open(&self, app: &mut BenchApp, name: String, w: &mut Window, cx: &mut Context<BenchApp>) {
        app.env_open_prompt(name, w, cx);
    }
    fn new_one(&self, e: &mut ProjectEnv) -> Option<(Entity<EditorState>, &'static str)> {
        e.prompts.open = Some("new".into());
        Some((e.prompts.text.clone(), ""))
    }
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        eid: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        let mut name = e.prompts.open.clone()?;
        if name == "new" {
            name = e.new_name.read(cx).value().trim().to_string();
            if name.is_empty() {
                app.toast(NotificationType::Error, "Name it first", window, cx);
                return None;
            }
        }
        Some((
            "PUT",
            format!("/api/projects/{eid}/prompts/{}", enc(&name)),
            json!({"text": e.prompts.text.read(cx).value().to_string()}),
        ))
    }
    fn saved(&self, e: &mut ProjectEnv) {
        e.prompts.dirty = false;
    }
    /// The composer's @ list has them too.
    fn reload(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
        app.load_prompts(window, cx);
    }
    fn revert(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        let Some(e) = app.project_env.as_mut() else {
            return true;
        };
        e.prompts.dirty = false;
        match e.prompts.open.take() {
            Some(name) => {
                app.env_open_prompt(name, window, cx);
                true
            }
            None => false,
        }
    }
    fn remove_path(&self, eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/projects/{eid}/prompts/{}", enc(name)))
    }
}

impl BenchApp {
    /// A prompt's row: its text opens under it to edit (again: closes).
    pub fn env_open_prompt(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if e.prompts.open.as_deref() == Some(&name) && !e.prompts.dirty {
            e.prompts.open = None;
            cx.notify();
            return;
        }
        e.prompts.open = Some(name.clone());
        e.prompts.dirty = false;
        let editor = e.prompts.text.clone();
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
                    let text = v["prompts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|p| p["name"].as_str() == Some(&name))
                        .and_then(|p| p["text"].as_str())
                        .unwrap_or_default()
                        .to_string();
                    editor.update(cx, |s, cx| s.set_value(text, window, cx));
                }
                cx.notify();
            },
        );
    }
}
