//! Keyboard shortcuts for the whole window. One handler at the root sees every key first and
//! takes only ⌘ combinations plus Esc in a few states (recording, a form, settings); everything
//! else — Esc in vim, ⌃C in a terminal, typing — goes on to the focused field or terminal.

use gpui_kit::*;

use super::{HermesApp, Panel};
use crate::theme::Size;

impl HermesApp {
    /// Returns true when the key was a shortcut (and is not passed on).
    pub fn global_key(
        &mut self,
        ks: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let m = &ks.modifiers;
        if ks.key == "escape" && !m.modified() {
            return self.escape(cx);
        }
        if !m.platform || m.control || m.alt {
            return false;
        }
        match (ks.key.as_str(), m.shift) {
            // Save the file open in the mini IDE.
            ("s", false) if self.file.is_some() => self.save_file(window, cx),
            // New chat / new server, as the mode's bottom button.
            ("n", false) if self.tasks.on => self.new_task(window, cx),
            // Done / not done for the open task.
            ("k", false) if self.tasks.on && self.tasks.selected.is_some() => {
                let id = self.tasks.selected.clone().unwrap_or_default();
                self.toggle_task_done(id, window, cx);
            }
            ("n", false) => {
                if self.ssh.on {
                    self.edit_server(None, None, window, cx);
                } else {
                    self.new_session(cx);
                    self.composer.update(cx, |s, cx| s.focus(window, cx));
                }
            }
            ("n", true) => self.open_new_window(cx),
            ("1", false) => self.set_ssh_mode(false, window, cx),
            ("2", false) if crate::bench::SSH => self.set_ssh_mode(true, window, cx),
            ("3", false) if crate::bench::TOOLS => self.toggle_tools(window, cx),
            ("4", false) if crate::bench::TASKS => self.toggle_tasks(window, cx),
            ("b", false) if crate::bench::IDE => self.toggle_ide(window, cx),
            ("b", true) if self.ide_open() => {
                self.ide.expanded = !self.ide.expanded;
                cx.notify();
            }
            ("p", false) if crate::bench::IDE => self.open_quick_open(window, cx),
            // Rich Text to Markdown shows the paste rendered (no editor has focus): ⌘V pastes
            // the clipboard's formatting there.
            ("v", false)
                if self.tools.on
                    && self.tools.tool == crate::devtools::Tool::HtmlToMarkdown
                    && !self.tools.html_source
                    && !self.tools.search.focus_handle(cx).is_focused(window) =>
            {
                self.paste_tool_input(window, cx)
            }
            // In the mini IDE's editor: the next occurrence gets a cursor too.
            ("d", false) if self.file_editor.focus_handle(cx).is_focused(window) => {
                self.select_next_occurrence(cx)
            }
            ("f", false) => {
                let search = if self.tasks.on {
                    self.tasks.search.clone()
                } else if self.ssh.on {
                    self.ssh.search.clone()
                } else {
                    self.search.clone()
                };
                search.update(cx, |s, cx| s.focus(window, cx));
            }
            (",", false) => {
                let panel = if self.panel.is_settings() {
                    Panel::Chat
                } else {
                    Panel::Settings
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
        } else if self.ide.quick.is_some() {
            self.ide.quick = None;
        } else if self.ide.edit.is_some() {
            self.ide.edit = None;
        } else if self.ssh.on && self.ssh.form.is_some() {
            self.ssh.form = None;
        } else if self.panel.is_settings() {
            self.panel = Panel::Chat;
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// ⌘B: hide the mini IDE, or open it on what the window shows (the server's terminal, else
    /// the open chat).
    pub(crate) fn toggle_ide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ide_open() {
            self.close_ide(cx);
        } else if self.ssh.on {
            match self.ssh.active_tab().map(|t| t.alias.clone()) {
                // This machine's terminal: its files, from the home folder.
                Some(alias) if alias == crate::term::LOCAL => {
                    self.open_ide(std::env::var("HOME").ok(), window, cx)
                }
                Some(alias) => self.open_ide_server(alias, window, cx),
                None => {}
            }
        } else {
            match self.session.clone() {
                Some(id) => self.open_ide_for_session(id, window, cx),
                None => self.open_ide(None, window, cx),
            }
        }
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
