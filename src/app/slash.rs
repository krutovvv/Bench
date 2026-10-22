//! The composer's pop-up list: slash commands while the input is a bare `/word` (no space yet),
//! saved prompts while the last word starts with `@`. Filtered as you type; ↑↓ move, Tab/Enter
//! pick, Esc hides it. A `/` elsewhere, or a word that is not a command (a path like /Users/…),
//! is just text.

use gpui_kit::*;

use super::HermesApp;
use crate::model::SlashCommand;

impl HermesApp {
    /// Commands matching the composer text, when the list should show.
    pub fn slash_matches(&self, cx: &App) -> Vec<SlashCommand> {
        let text = self.composer.read(cx).value();
        if self.slash_closed || !text.starts_with('/') || text.contains(char::is_whitespace) {
            return vec![];
        }
        let q = text.to_lowercase();
        let mut first: Vec<SlashCommand> = vec![];
        let mut rest: Vec<SlashCommand> = vec![];
        for c in &self.catalog {
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

    /// `/goal ship it` → ("/goal", "ship it") when `/goal` is a known command.
    pub fn parse_command(&self, text: &str) -> Option<(String, String)> {
        if !text.starts_with('/') {
            return None;
        }
        let (name, arg) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        self.catalog
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(name))
            .then(|| (name.to_string(), arg.trim().to_string()))
    }

    /// Put the highlighted command into the composer ("/goal "); false when the list is closed.
    pub fn complete_slash(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let matches = self.slash_matches(cx);
        let Some(c) = matches.get(self.slash_sel.min(matches.len().saturating_sub(1))) else {
            return false;
        };
        let value = format!("{} ", c.name);
        self.composer
            .update(cx, |s, cx| s.set_value(value, window, cx));
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
            "tab" => return self.complete_slash(window, cx),
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
