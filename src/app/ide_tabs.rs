//! The mini IDE's tabs: every file opened stays as a tab; switching parks the open one (its
//! text, unsaved edits included, and a server file's sync state) and brings the other back as
//! it was. A tab with unsaved edits shows a dot; closing it, or the IDE, asks twice.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use super::ide::Parked;
use crate::lang::language_for_text;

impl HermesApp {
    /// Before another file takes the editor: keep this one's state.
    pub(super) fn park_current(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.file.take() else {
            return;
        };
        let text = self.file_editor.read(cx).value().to_string();
        self.ide.parked.insert(
            view.path.clone(),
            Parked {
                view,
                text,
                baseline: std::mem::take(&mut self.ide.baseline),
                dirty: std::mem::take(&mut self.ide.dirty),
                remote: self.ide.remote_file.take(),
            },
        );
    }

    /// Show a parked tab again, exactly as it was left. False when `path` isn't parked.
    pub(super) fn restore_parked(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(p) = self.ide.parked.remove(path) else {
            return false;
        };
        let language = language_for_text(path, &p.text);
        self.file_editor.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_value(p.text, window, cx);
        });
        self.file = Some(p.view);
        self.ide.baseline = p.baseline;
        self.ide.dirty = p.dirty;
        self.ide.changed_outside = false;
        let watch = p.remote.is_some();
        self.ide.remote_file = p.remote;
        if watch {
            self.watch_remote_file(window, cx);
        }
        cx.notify();
        true
    }

    /// The editor's text changed (typing or loading): is it different from what is saved?
    pub fn on_editor_change(&mut self, cx: &mut Context<Self>) {
        let editable = self
            .file
            .as_ref()
            .is_some_and(|f| f.loaded && !f.truncated && !f.missing);
        let dirty = editable && self.file_editor.read(cx).value().as_ref() != self.ide.baseline;
        if dirty != self.ide.dirty {
            self.ide.dirty = dirty;
            cx.notify();
        }
    }

    pub fn is_dirty(&self, path: &str) -> bool {
        match self.file.as_ref() {
            Some(f) if f.path == path => self.ide.dirty,
            _ => self.ide.parked.get(path).is_some_and(|p| p.dirty),
        }
    }

    pub fn switch_tab(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file(path, window, cx);
    }

    /// × on a tab. With unsaved edits the first press arms (the × turns red), the second closes.
    pub fn close_tab(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_dirty(&path) && self.ide.armed_close.as_deref() != Some(path.as_str()) {
            self.ide.armed_close = Some(path);
            cx.notify();
            return;
        }
        self.ide.armed_close = None;
        let Some(k) = self.ide.tabs.iter().position(|t| *t == path) else {
            return;
        };
        self.ide.tabs.remove(k);
        self.ide.parked.remove(&path);
        let active = self.file.as_ref().is_some_and(|f| f.path == path);
        if active {
            self.file = None;
            self.ide.remote_file = None;
            self.ide.baseline.clear();
            self.ide.dirty = false;
            self.file_editor
                .update(cx, |s, cx| s.set_value("", window, cx));
            let next = self
                .ide
                .tabs
                .get(k.min(self.ide.tabs.len().saturating_sub(1)));
            if let Some(next) = next.cloned() {
                self.open_file(next, window, cx);
            }
        }
        cx.notify();
    }

    /// Unsaved edits anywhere in the IDE (the open tab or parked ones).
    pub fn ide_unsaved(&self) -> usize {
        self.ide.dirty as usize + self.ide.parked.values().filter(|p| p.dirty).count()
    }

    /// Closing the IDE with unsaved edits asks twice (a toast says what would be lost).
    pub fn request_close_ide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.ide_unsaved();
        if n > 0 && !self.ide.armed_close_all {
            self.ide.armed_close_all = true;
            self.toast(
                NotificationType::Warning,
                format!(
                    "{n} file{} with unsaved edits — close again to discard them",
                    if n == 1 { "" } else { "s" }
                ),
                window,
                cx,
            );
            cx.notify();
            return;
        }
        self.close_ide(cx);
    }

    /// A remote file whose upload just succeeded: the editor's text is now what is saved.
    pub(super) fn mark_saved(&mut self, path: &str, text: String) {
        match self.file.as_ref() {
            Some(f) if f.path == path => {
                self.ide.baseline = text;
                self.ide.dirty = false;
            }
            _ => {
                if let Some(p) = self.ide.parked.get_mut(path) {
                    p.dirty = p.text != text;
                    p.baseline = text;
                }
            }
        }
    }

    /// ⌘D, as in Zed: with nothing selected, select the word at the cursor; then add a cursor
    /// on each next occurrence of the selected text (wrapping to the top), typed into all at once.
    pub fn select_next_occurrence(&mut self, cx: &mut Context<Self>) {
        let (text, range) = {
            let s = self.file_editor.read(cx);
            (s.value().to_string(), s.selected_range())
        };
        if range.is_empty() {
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let at = range.start.min(text.len());
            let start = text[..at]
                .char_indices()
                .rev()
                .take_while(|(_, c)| is_word(*c))
                .last()
                .map_or(at, |(i, _)| i);
            let end = text[at..]
                .char_indices()
                .find(|(_, c)| !is_word(*c))
                .map_or(text.len(), |(i, _)| at + i);
            if start < end {
                self.file_editor
                    .update(cx, |s, cx| s.set_selected_range(start..end, cx));
                self.ide.next_match = Some((start, text[start..end].to_string(), vec![]));
            }
            return;
        }
        let needle = text[range.clone()].to_string();
        // A new run when the first selection changed since the last ⌘D.
        let mut added = match self.ide.next_match.take() {
            Some((anchor, n, added)) if anchor == range.start && n == needle => added,
            _ => vec![],
        };
        // From the last one added (or the first selection) to the end, then from the top; the
        // ones already selected are skipped.
        let from = added.last().map_or(range.end, |s| s + needle.len());
        let starts: Vec<usize> = text
            .match_indices(needle.as_str())
            .map(|(i, _)| i)
            .collect();
        let next = starts
            .iter()
            .chain(starts.iter())
            .skip_while(|s| **s < from)
            .chain(starts.iter())
            .find(|s| **s != range.start && !added.contains(s))
            .copied();
        if let Some(start) = next {
            self.file_editor
                .update(cx, |s, cx| s.add_selection(start..start + needle.len(), cx));
            added.push(start);
        }
        self.ide.next_match = Some((range.start, needle, added));
    }
}
