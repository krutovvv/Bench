//! Settings → Prompts: the user's text snippets, kept per agent profile as Markdown files
//! (`<profile home>/prompts/<title>.md`, like skills) and inserted from the composer with `@`.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::model::Prompt;
use crate::rest;

impl HermesApp {
    /// Read the profile's prompts (on profile switch and when the tab opens).
    pub(super) fn load_prompts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest(window, cx, rest::prompts, |this, r, window, cx| {
            if let Ok(list) = r {
                this.prompts = list;
                if this.panel == super::Panel::Prompts {
                    let i = this
                        .prompt_open
                        .filter(|i| *i < this.prompts.len())
                        .or((!this.prompts.is_empty()).then_some(0));
                    this.prompt_open = None;
                    if let Some(i) = i {
                        this.open_prompt(i, window, cx);
                    }
                }
                cx.notify();
            }
        });
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

    /// A new, unsaved prompt (it becomes a file on Save).
    pub fn new_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompts.push(Prompt {
            title: "New prompt".into(),
            ..Default::default()
        });
        let i = self.prompts.len() - 1;
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
        let old = self
            .prompts
            .get(i)
            .map(|p| p.path.clone())
            .unwrap_or_default();
        let (t, b) = (title.clone(), body.clone());
        self.rest(
            window,
            cx,
            move |c, p| rest::save_prompt(c, p, &old, &t, &b),
            move |this, r, window, cx| match r {
                Ok(path) => {
                    if let Some(p) = this.prompts.get_mut(i) {
                        *p = Prompt { title, body, path };
                    }
                    this.toast(NotificationType::Success, "Saved", window, cx);
                    cx.notify();
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Prompt: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// First press arms, second deletes the file.
    pub fn delete_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.prompt_open else { return };
        if !self.prompt_delete_armed {
            self.prompt_delete_armed = true;
            cx.notify();
            return;
        }
        self.prompt_delete_armed = false;
        let path = self
            .prompts
            .get(i)
            .map(|p| p.path.clone())
            .unwrap_or_default();
        if i < self.prompts.len() {
            self.prompts.remove(i);
        }
        self.prompt_open = None;
        if !self.prompts.is_empty() {
            self.open_prompt(i.min(self.prompts.len() - 1), window, cx);
        }
        if !path.is_empty() {
            self.rest(
                window,
                cx,
                move |c, p| rest::delete_prompt(c, p, &path),
                |this, r, window, cx| {
                    if let Err(e) = r {
                        this.toast(
                            NotificationType::Error,
                            format!("Prompt: {e:#}"),
                            window,
                            cx,
                        );
                    }
                },
            );
        }
        cx.notify();
    }
}
