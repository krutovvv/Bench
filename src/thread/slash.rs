//! The composer's pop-up list: slash commands while the input is a bare `/word` (no space yet),
//! saved prompts while the last word starts with `@`. Filtered as you type; ↑↓ move, Tab/Enter
//! pick, Esc hides it. A `/` elsewhere, or a word that is not a command (a path like /Users/…),
//! is just text.

use gpui_kit::*;

use super::{ThreadRef, ThreadView};
use crate::app::BenchApp;
use crate::app::model::SlashCommand;

/// `@word` at the end of the composer text: (byte offset of the `@`, the word).
fn prompt_token(text: &str) -> Option<(usize, String)> {
    let start = text.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    let token = &text[start..];
    token
        .starts_with('@')
        .then(|| (start, token[1..].to_lowercase()))
}

impl BenchApp {
    /// /browser shows the server browser's tab: offered where the thread's environment has that
    /// browser's MCP server (a project's own, or Host's), not where another browser works.
    pub fn browser_offered(&self, at: &ThreadRef) -> bool {
        if self.demo {
            return true;
        }
        match self.places.projects.iter().find(|p| p.id == at.project) {
            Some(p) => p.browser,
            None => self.places.host_browser,
        }
    }

    /// The slash commands threads offer, as this Mac's agent lists them.
    pub(crate) fn load_commands(&mut self, cx: &mut Context<Self>) {
        self.request("", protocol::ListCommands {}, cx, |this, r, _, _| {
            if let Ok(list) = r {
                this.defaults.commands = list
                    .into_iter()
                    .map(|c| SlashCommand {
                        name: c.name,
                        desc: c.description,
                        category: "Thread".into(),
                        key: String::new(),
                    })
                    .collect();
            }
        });
    }

    /// `/goal ship it` → ("/goal", "ship it") when `/goal` is a known command.
    pub fn parse_command(&self, text: &str) -> Option<(String, String)> {
        if !text.starts_with('/') {
            return None;
        }
        let (name, arg) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        self.defaults.commands
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(name))
            .then(|| (name.to_string(), arg.trim().to_string()))
    }
}

impl ThreadView {
    /// Rows for the pop-up list: commands for a bare `/word`, prompts for a trailing `@word`.
    pub fn slash_matches(&self, cx: &App) -> Vec<SlashCommand> {
        let text = self.composer.read(cx).value();
        let Some(app) = self.app(cx).filter(|_| !self.slash_closed) else {
            return vec![];
        };
        if let Some((_, q)) = prompt_token(&text) {
            return app
                .prompts
                .iter()
                .filter(|p| !p.path.is_empty())
                .filter(|p| {
                    p.title.to_lowercase().contains(&q) || p.body.to_lowercase().contains(&q)
                })
                .map(|p| SlashCommand {
                    name: format!("@{}", p.title),
                    desc: p
                        .body
                        .lines()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or_default()
                        .to_string(),
                    category: "Prompt".into(),
                    key: String::new(),
                })
                .collect();
        }
        let at = self.thread_ref();
        // `/tasks today`, `/tools json`…: the tools this thread can attach.
        if let Some(rows) = app.tool_slash(&at, &text) {
            return rows;
        }
        if !text.starts_with('/') || text.contains(char::is_whitespace) {
            return vec![];
        }
        let q = text.to_lowercase();
        let mut first: Vec<SlashCommand> = vec![];
        let mut rest: Vec<SlashCommand> = vec![];
        // Tools only: the session's commands (/new, /model…) are the window's buttons here.
        let browser = app.browser_offered(&at);
        for c in BenchApp::tool_words()
            .iter()
            .filter(|c| browser || c.name != "/browser")
        {
            let name = c.name.to_lowercase();
            if name.starts_with(&q) {
                first.push(c.clone());
            } else if q.len() > 2 && name.contains(&q[1..]) {
                rest.push(c.clone());
            }
        }
        first.extend(rest);
        first.truncate(60);
        first
    }

    /// Put the highlighted command into the composer ("/goal "); false when the list is closed.
    pub fn complete_slash(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let matches = self.slash_matches(cx);
        let Some(c) = matches.get(self.slash_sel.min(matches.len().saturating_sub(1))) else {
            return false;
        };
        // A tool: attached to the thread at once; the line is cleared.
        if !c.key.is_empty() {
            let key = c.key.clone();
            self.composer
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.note_draft(cx);
            self.put_insert(key, window, cx);
            return true;
        }
        let text = self.composer.read(cx).value().to_string();
        let value = match (c.category.as_str(), prompt_token(&text)) {
            // A prompt replaces the `@word` it was found by.
            ("Prompt", Some((start, _))) => {
                let title = &c.name[1..];
                let body = self
                    .app(cx)
                    .and_then(|a| a.prompts.iter().find(|p| p.title == title))
                    .map(|p| p.body.clone())
                    .unwrap_or_default();
                format!("{}{body}", &text[..start])
            }
            _ => format!("{} ", c.name),
        };
        self.composer
            .update(cx, |s, cx| s.set_value(value, window, cx));
        self.note_draft(cx);
        cx.notify();
        true
    }

    /// ↑ ↓ Tab Esc while the list is open; returns whether the key was used.
    pub fn slash_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let n = self.slash_matches(cx).len();
        if n == 0 {
            return false;
        }
        match key {
            "up" => self.slash_sel = (self.slash_sel + n - 1) % n,
            "down" => self.slash_sel = (self.slash_sel + 1) % n,
            "tab" | "enter" => return self.complete_slash(window, cx),
            "escape" => self.slash_closed = true,
            _ => return false,
        }
        self.slash_scroll.scroll_to_item(self.slash_sel);
        cx.notify();
        true
    }

    pub fn pick_slash(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.slash_sel = i;
        self.complete_slash(window, cx);
        self.composer.update(cx, |s, cx| s.focus(window, cx));
    }
}
