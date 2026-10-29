//! Prompts: text snippets inserted from the composer with `@`. A thread offers its project's own
//! and the general ones (`<environment>/prompts/<title>.md`, read through the agent that keeps
//! the thread); they are edited in the project's environment block.

use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rest;

impl BenchApp {
    /// The open thread's prompts (its project's and the general ones).
    pub(crate) fn load_prompts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.thread(cx).project.clone();
        let target = if project.is_empty() {
            self.local_conn()
                .map(|c| (c, crate::app::model::ProjectKey::HOST.to_string()))
        } else {
            self.env_target(&project)
        };
        let Some((conn, eid)) = target else {
            return;
        };
        let path = format!("/api/projects/{eid}/prompts");
        self.background(
            window,
            cx,
            move || rest::get(&conn, &path),
            |this, r, _, cx| {
                if let Ok(v) = r {
                    this.threads.prompts = v["prompts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|p| crate::thread::composer::prompts::Prompt {
                            title: p["name"].as_str().unwrap_or_default().to_string(),
                            body: p["text"].as_str().unwrap_or_default().to_string(),
                            path: p["name"].as_str().unwrap_or_default().to_string(),
                        })
                        .collect();
                    cx.notify();
                }
            },
        );
    }
}

/// A saved text snippet (Settings → Prompts), inserted from the composer with `@`: a Markdown
/// file `<agent home>/prompts/<title>.md` on the agent's machine.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    pub title: String,
    pub body: String,
    /// Where it lives; empty until first saved.
    pub path: String,
}
