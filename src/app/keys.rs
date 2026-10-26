//! Keyboard shortcuts for the whole window. One handler at the root sees every key first and
//! takes only ⌘ combinations plus Esc in a few states (recording, a form, settings); everything
//! else — Esc in vim, ⌃C in a terminal, typing — goes on to the focused field or terminal.

use gpui_kit::*;

use super::{BenchApp, Panel};
use crate::theme::Size;

impl BenchApp {
    /// Returns true when the key was a shortcut (and is not passed on).
    pub fn global_key(
        &mut self,
        ks: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let m = &ks.modifiers;
        // ↑ ↓ Tab Enter for the `/` list are answered by a keystroke interceptor (main.rs):
        // bindings run before key listeners, and the composer binds ↑ ↓ to the caret, so this
        // handler is too late for them. Escapes come here.
        if ks.key == "escape" && !m.modified() {
            return self.escape(cx);
        }
        if !m.platform || m.control || m.alt {
            return false;
        }
        match (ks.key.as_str(), m.shift) {
            ("n", false) => {
                self.new_session(cx);
                self.composer.update(cx, |s, cx| s.focus(window, cx));
            }
            ("n", true) => self.open_new_window(cx),
            ("2", false) if crate::bench::SSH => {
                self.open_server_thread(crate::term::LOCAL.to_string(), window, cx)
            }
            ("3", false) if crate::bench::TOOLS => {
                let tool = self.tools.tool;
                self.open_tool_thread(tool, window, cx)
            }
            ("4", false) if crate::bench::TASKS => {
                self.open_task_list(crate::tasks::List::Today, window, cx)
            }
            ("\\", false) => self.toggle_sidebar(cx),
            // ⌘⌫ / ⌘⌦: delete the open thread (the first press arms it in the sidebar, the
            // second deletes), as Move to Trash in Finder; a bare ⌫ stays the composer's.
            ("backspace" | "delete", false)
                if self.panel == Panel::Chat && self.session.is_some() =>
            {
                let id = self.session.clone().unwrap_or_default();
                self.delete_session(id, window, cx);
            }
            // Dictation without its button: start, and again to stop and transcribe.
            ("d", true) => self.toggle_mic(window, cx),
            ("f", false) => self.open_search(window, cx),
            (",", false) => {
                let panel = if self.panel == Panel::Providers {
                    Panel::Chat
                } else {
                    Panel::Providers
                };
                self.open_panel(panel, window, cx);
            }
            (".", false) if self.live.is_some() => self.stop(cx),
            ("=" | "+", _) => self.step_size(1, window, cx),
            ("-", false) => self.step_size(-1, window, cx),
            _ => return false,
        }
        true
    }

    /// Esc: cancel a recording (nothing is transcribed), close the quick-open list, the server
    /// form or the settings. Otherwise it belongs to whatever has focus.
    fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        if self.recorder.take().is_some() {
            self.wave_ticker = None;
            self.send_after_transcribe = false;
        } else if self.ssh.form.is_some() {
            self.ssh.form = None;
        } else if self.panel.is_settings() {
            self.panel = Panel::Chat;
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// ⌘F or the sidebar's magnifier: focus the sidebar's search.
    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = self.search.clone();
        // A hidden sidebar comes back to show its search.
        self.sidebar_hidden = false;
        search.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// ⌘\ or the toggle in the sidebar's top strip (the header's, while it is hidden).
    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_hidden = !self.sidebar_hidden;
        cx.notify();
    }

    /// ⌘+ / ⌘−: the Appearance size, one step.
    fn step_size(&mut self, step: i32, window: &mut Window, cx: &mut Context<Self>) {
        let sizes = [Size::Small, Size::Normal, Size::Large];
        let at = sizes
            .iter()
            .position(|s| *s == self.appearance.size)
            .unwrap_or(1) as i32;
        let next = sizes[(at + step).clamp(0, 2) as usize];
        if next != self.appearance.size {
            let a = crate::theme::Appearance {
                size: next,
                ..self.appearance
            };
            self.set_appearance(a, window, cx);
        }
    }
}
