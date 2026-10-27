//! Drafts: the composer's unsent text, one per thread, kept on this Mac across restarts.

use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The composer's text is the open chat's draft (called on every change).
    pub(crate) fn note_draft(&mut self, cx: &mut Context<Self>) {
        // A new chat keeps no draft: every new chat starts empty.
        let Some(key) = self.draft_key.clone().filter(|k| !k.is_empty()) else {
            return;
        };
        let text = self.composer.read(cx).value().to_string();
        let changed = if text.trim().is_empty() {
            self.drafts.remove(&key).is_some()
        } else {
            self.drafts.insert(key, text.clone()).as_deref() != Some(text.as_str())
        };
        if changed {
            save_drafts(&self.drafts);
        }
    }

    /// Another chat is open (or the new one): the composer shows its draft.
    pub(crate) fn sync_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.active_thread.session.clone().unwrap_or_default();
        if self.draft_key.as_ref() == Some(&key) {
            return;
        }
        // The new chat just became this thread (an insert, a message): what is typed stays.
        let became = self.draft_key.as_deref() == Some("")
            && !key.is_empty()
            && !self.drafts.contains_key(&key);
        let text = if became {
            self.composer.read(cx).value().to_string()
        } else if key.is_empty() {
            String::new()
        } else {
            // A lone "/word" is a command half picked, not a draft.
            self.drafts
                .get(&key)
                .cloned()
                .filter(|t| !(t.starts_with('/') && !t.trim().contains(' ')))
                .unwrap_or_default()
        };
        self.draft_key = Some(key);
        if became {
            self.note_draft(cx);
            return;
        }
        self.composer
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }
}

pub(crate) fn drafts_path() -> std::path::PathBuf {
    crate::client::connection::state_dir().join("drafts.json")
}

pub(crate) fn load_drafts() -> std::collections::HashMap<String, String> {
    std::fs::read(drafts_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub(crate) fn save_drafts(drafts: &std::collections::HashMap<String, String>) {
    let _ = std::fs::create_dir_all(crate::client::connection::state_dir());
    let _ = std::fs::write(
        drafts_path(),
        serde_json::to_vec(drafts).unwrap_or_default(),
    );
}
