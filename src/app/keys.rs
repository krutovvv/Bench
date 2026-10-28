//! Keyboard shortcuts for the whole window. One handler at the root sees every key first and
//! takes only ⌘ combinations plus Esc in a few states (recording, a form, settings); everything
//! else — Esc in vim, ⌃C in a terminal, typing — goes on to the focused field or terminal.

use gpui_kit::*;

use super::{BenchApp, Center};
use crate::workspace::Panel;
use bench_ui::theme::Size;

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
        // The rest of the window's keys are the menu bar's (`menu.rs`): it shows them. These stay
        // here, the window's only when they are not a field's.
        match (ks.key.as_str(), m.shift) {
            ("2", false) if crate::app::shortcuts::SSH => self.open_terminal_thread(window, cx),
            // ⌘T: another terminal below the center, when it shows.
            ("t", false) if self.bottom_dock.shows(Panel::Terminal) => {
                self.dock_term_add(window, cx)
            }
            ("3", false) if crate::app::shortcuts::TOOLS => {
                let tool = self.inserts.tools.tool;
                self.open_tool_thread(tool, window, cx)
            }
            // ⌘⌫ / ⌘⌦: delete the open thread — asked once, as ⌫ ⌫ in the sidebar
            // (`sidebar_items.rs`); a bare ⌫ elsewhere is the composer's, and ⌘⌫ in a field
            // deletes to the line's start when no thread is open in the center.
            ("backspace" | "delete", false)
                if self.center == Center::Chat && self.thread(cx).id.is_some() =>
            {
                self.delete_open_thread(cx)
            }
            _ => return false,
        }
        true
    }

    /// Esc: cancel a recording (nothing is transcribed), close the quick-open list, the server
    /// form or the settings. Otherwise it belongs to whatever has focus.
    fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        // The new project's card is the topmost layer while it is open.
        if self.new_project.form.is_some() {
            self.cancel_project_form(cx);
            return true;
        }
        if self.sidebar.renaming.is_some() {
            self.cancel_rename(cx);
            return true;
        }
        // The Explorer's ⌘P, a name being typed in its tree, a comment, ⌘⇧F's results: the latest first.
        let ex = &mut self.explorer;
        if ex.quick.take().is_some()
            || ex.naming.take().is_some()
            || ex.comment.take().is_some()
            || ex.find.take().is_some()
            || self.armed.clear()
        {
            cx.notify();
            return true;
        }
        if self.media.recorder.take().is_some() {
            self.media.wave_ticker = None;
            self.media.send_after_transcribe = false;
        } else if self.center.is_settings() {
            self.center = Center::Chat;
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// ⌘F or the sidebar's magnifier: focus the sidebar's search.
    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = self.sidebar.search.clone();
        // A hidden sidebar comes back to show its search.
        self.left_dock.open = true;
        search.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// ⌘\ or the toggle in the sidebar's top strip (the header's, while it is hidden).
    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.left_dock.toggle();
        cx.notify();
    }

    /// ⌘+ / ⌘−: the Appearance size, one step.
    pub(crate) fn step_size(&mut self, step: i32, window: &mut Window, cx: &mut Context<Self>) {
        let sizes = [Size::Small, Size::Normal, Size::Large];
        let at = sizes
            .iter()
            .position(|s| *s == self.appearance.size)
            .unwrap_or(1) as i32;
        let next = sizes[(at + step).clamp(0, 2) as usize];
        if next != self.appearance.size {
            let a = bench_ui::theme::Appearance {
                size: next,
                ..self.appearance.clone()
            };
            self.set_appearance(a, window, cx);
        }
    }
}
