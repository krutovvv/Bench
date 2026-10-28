//! What the Explorer opens, as tabs of the middle: `FileRef` (a file where the agent reaches it),
//! `DocKey` (a tab's identity: a file, its diff, a part of the environment, the project's
//! settings, its Problems), `Doc` (an open file's editor and state); opening, closing and saving
//! a tab, whatever it holds (each kind is an `Item`, `item.rs`).

use gpui_kit::component::input::EditorState;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;

/// A file as the Explorer reaches it: through the agent of `project`, on `machine` when that is
/// not the agent's own (a project this Mac's agent works on over SSH).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileRef {
    pub project: String,
    pub machine: Option<String>,
    pub path: String,
}

impl FileRef {
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The folder it is in.
    pub fn parent(&self) -> FileRef {
        let up = match self.path.trim_end_matches('/').rsplit_once('/') {
            Some(("", _)) => "/".to_string(),
            Some((p, _)) => p.to_string(),
            None => ".".to_string(),
        };
        FileRef {
            path: up,
            ..self.clone()
        }
    }

    /// `name` (or `a/b`) inside this folder.
    pub fn join(&self, name: &str) -> FileRef {
        FileRef {
            path: format!("{}/{name}", self.path.trim_end_matches('/')),
            ..self.clone()
        }
    }

    /// Whether it is `other` or inside it, on the same machine of the same project.
    pub fn inside(&self, other: &FileRef) -> bool {
        self.project == other.project && self.machine == other.machine && self.within(&other.path)
    }

    /// Whether its path is this folder's or inside it (wherever it is: `inside` asks that too).
    pub fn within(&self, folder: &str) -> bool {
        self.path == folder
            || self
                .path
                .starts_with(&format!("{}/", folder.trim_end_matches('/')))
    }

    /// The key its folder's listing is kept under.
    pub fn listing_key(&self) -> String {
        format!("{}:{}", self.machine.as_deref().unwrap_or(""), self.path)
    }
}

/// What a tab holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DocKey {
    File(FileRef),
    /// A project's settings: its name, where it lives, how it runs, its Telegram bot
    /// (`src/env/page.rs`).
    Settings {
        project: String,
    },
    /// A `Section`'s row in a project's environment: the project (`ProjectEnv::id`), the
    /// section's key and the row's name ("new": a new one's form).
    Part {
        project: String,
        kind: &'static str,
        name: String,
    },
    /// The errors and warnings the project's language servers know (`problems.rs`).
    Problems {
        project: String,
    },
    /// A file's changes against HEAD, read only (`git.rs`).
    Diff(FileRef),
    /// What one turn of a thread changed in a file: the repository's trees at its start and end
    /// (`code/turn_diff.rs`), read only.
    TurnDiff { file: FileRef, turn: Turn },
}

/// A turn's trees in the repository it changed (`agent/src/runtime/turns.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Turn {
    pub root: String,
    pub base: String,
    pub head: String,
}

impl DocKey {
    /// The project it belongs to.
    pub fn project(&self) -> &str {
        match self {
            DocKey::File(f) | DocKey::Diff(f) | DocKey::TurnDiff { file: f, .. } => &f.project,
            DocKey::Settings { project }
            | DocKey::Part { project, .. }
            | DocKey::Problems { project } => project,
        }
    }
}

/// An open file: its editor, the text as saved, whether it changed since.
pub struct Doc {
    pub editor: Entity<EditorState>,
    pub saved: String,
    pub dirty: bool,
    pub loading: bool,
    /// Not text, or too large to be read whole: nothing to edit or save here.
    pub binary: bool,
    /// The line to put the caret on once it is read (⌘⇧F's hit).
    pub goto: Option<u32>,
    /// Its text at HEAD (`git.rs`); None: not in a repository, not committed, not read yet.
    pub head: Option<String>,
    /// Its language servers' check under way (`lsp.rs`); a newer one replaces it.
    pub checking: Task<()>,
}

impl Doc {
    /// A document whose text is still to come.
    pub fn new(editor: Entity<EditorState>) -> Self {
        Doc {
            editor,
            saved: String::new(),
            dirty: false,
            loading: true,
            binary: false,
            goto: None,
            head: None,
            checking: Task::ready(()),
        }
    }
}

impl BenchApp {
    /// Open `key` in the middle: its tab (added once), shown.
    pub fn open_doc(&mut self, key: DocKey, window: &mut Window, cx: &mut Context<Self>) {
        if !key.item().open(self, window, cx) {
            return;
        }
        if !self.explorer.tabs.contains(&key) {
            let at = self
                .explorer
                .active
                .as_ref()
                .and_then(|a| self.explorer.tabs.iter().position(|t| t == a))
                .map_or(self.explorer.tabs.len(), |i| i + 1);
            self.explorer.tabs.insert(at, key.clone());
        }
        self.explorer.active = Some(key);
        self.center = Center::Doc;
        self.keep_explorer();
        cx.notify();
    }

    /// The document a command means (⌘S, ⌘L): the center's.
    pub fn focused_doc(&self) -> Option<DocKey> {
        self.explorer
            .active
            .clone()
            .filter(|_| self.center == Center::Doc)
    }

    /// ⌘S, a document's Save: as its `Item` saves.
    pub fn save_doc(&mut self, key: DocKey, window: &mut Window, cx: &mut Context<Self>) {
        key.item().save(self, window, cx);
    }

    /// ⌘S: the document the caret is in.
    pub fn save_active_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.focused_doc() else {
            return false;
        };
        self.save_doc(key, window, cx);
        true
    }

    /// A tab's ×: with unsaved changes asked once; the middle goes to the next tab, or back to the
    /// thread.
    pub fn close_doc(&mut self, key: DocKey, cx: &mut Context<Self>) {
        let item = key.item();
        let dirty = item.dirty(self, cx);
        let armed = format!("close:{key:?}");
        if dirty && !self.armed.confirm(armed) {
            cx.notify();
            return;
        }
        let Some(at) = self.explorer.tabs.iter().position(|t| *t == key) else {
            return;
        };
        self.explorer.tabs.remove(at);
        if self.explorer.active.as_ref() == Some(&key) {
            let next = self
                .explorer
                .tabs
                .get(at)
                .or_else(|| self.explorer.tabs.get(at.wrapping_sub(1)))
                .cloned();
            self.explorer.active = next.clone();
            if next.is_none() && self.center == Center::Doc {
                self.center = Center::Chat;
            }
        }
        item.closed(self, cx);
        // Unsaved changes asked for already: the next opening reads the file again.
        if dirty {
            self.explorer.docs.remove(&key);
        }
        self.drop_unused(&key);
        self.keep_explorer();
        cx.notify();
    }

    /// A file's editor goes when no tab holds it and nothing is unsaved.
    pub(crate) fn drop_unused(&mut self, key: &DocKey) {
        // A file's diff tab reads its document: it stays while either is open.
        let held = match key {
            DocKey::File(f) => self.explorer.tabs.contains(&DocKey::Diff(f.clone())),
            DocKey::Diff(f) => {
                let file = DocKey::File(f.clone());
                if !self.explorer.tabs.contains(&file) {
                    self.drop_unused_now(&file);
                }
                false
            }
            _ => false,
        } || self.explorer.tabs.contains(key);
        if !held {
            self.drop_unused_now(key);
        }
    }

    fn drop_unused_now(&mut self, key: &DocKey) {
        if self.explorer.docs.get(key).is_some_and(|d| !d.dirty) {
            self.explorer.docs.remove(key);
        }
    }
}
