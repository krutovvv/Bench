//! Settings → Prompts: the user's own text snippets, kept on this Mac (the same for every
//! gateway) and inserted from the composer with `@`.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::connection;
use crate::model::Prompt;

fn path() -> std::path::PathBuf {
    connection::state_dir().join("prompts.json")
}

pub(super) fn load() -> Vec<Prompt> {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

impl HermesApp {
    fn store_prompts(&self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = std::fs::create_dir_all(connection::state_dir());
        let json = serde_json::to_vec_pretty(&self.prompts).unwrap_or_default();
        if let Err(e) = std::fs::write(path(), json) {
            self.toast(NotificationType::Error, format!("Prompts: {e}"), window, cx);
        }
    }

    pub fn open_prompt(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.prompts.get(i).cloned() else {
            return;
        };
        self.prompt_open = Some(i);
        self.prompt_delete_armed = false;
        self.prompt_title
            .update(cx, |s, cx| s.set_value(p.title, window, cx));
        self.prompt_editor
            .update(cx, |s, cx| s.set_value(p.body, window, cx));
        cx.notify();
    }

    pub fn new_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompts.push(Prompt {
            title: "New prompt".into(),
            body: String::new(),
        });
        let i = self.prompts.len() - 1;
        self.store_prompts(window, cx);
        self.open_prompt(i, window, cx);
        self.prompt_title.update(cx, |s, cx| s.focus(window, cx));
    }

    pub fn save_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.prompt_open else { return };
        let title = self.prompt_title.read(cx).value().trim().to_string();
        if title.is_empty() {
            self.toast(
                NotificationType::Warning,
                "Give the prompt a name",
                window,
                cx,
            );
            return;
        }
        let body = self.prompt_editor.read(cx).value().to_string();
        if let Some(p) = self.prompts.get_mut(i) {
            *p = Prompt { title, body };
        }
        self.store_prompts(window, cx);
        self.toast(NotificationType::Success, "Saved", window, cx);
        cx.notify();
    }

    /// First press arms, second deletes.
    pub fn delete_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.prompt_open else { return };
        if !self.prompt_delete_armed {
            self.prompt_delete_armed = true;
            cx.notify();
            return;
        }
        self.prompt_delete_armed = false;
        if i < self.prompts.len() {
            self.prompts.remove(i);
        }
        self.store_prompts(window, cx);
        self.prompt_open = None;
        if !self.prompts.is_empty() {
            self.open_prompt(i.min(self.prompts.len() - 1), window, cx);
        }
        cx.notify();
    }
}
