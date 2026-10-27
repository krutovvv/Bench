//! What the Explorer opens: a file — on the project's machine, or the agent's own (its soul,
//! prompts, skills are files there) — in an editor of its own, or a part of the project's
//! environment that is not a file (an MCP server, a language server, a scheduled job), drawn by
//! its `Section` (`env/`). A file opens in the middle as a tab, or beside; a part in the middle.

use gpui_kit::component::input::{EditorState, InputEvent};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::app::{BenchApp, Center};
use crate::rest;

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

/// What a tab, or the pane beside, holds.
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
}

impl DocKey {
    /// The project it belongs to.
    pub fn project(&self) -> &str {
        match self {
            DocKey::File(f) => &f.project,
            DocKey::Settings { project } | DocKey::Part { project, .. } => project,
        }
    }
}

/// An open file: its editor, the text as saved, whether it changed since.
pub struct Doc {
    pub editor: Entity<EditorState>,
    pub saved: String,
    pub dirty: bool,
    pub loading: bool,
    /// Not text, or too large to be read whole: nothing to edit or save here (Zed opens it).
    pub binary: bool,
    /// The line to put the caret on once it is read (⌘⇧F's hit).
    pub goto: Option<u32>,
}

impl BenchApp {
    /// Open `key` in the middle: its tab (added once), shown.
    pub fn open_doc(&mut self, key: DocKey, window: &mut Window, cx: &mut Context<Self>) {
        match &key {
            DocKey::File(f) => self.ensure_doc(f.clone(), window, cx),
            // Its fields are the environment's: read once, kept while typed in.
            DocKey::Settings { project } => {
                if !self.envs.contains_key(project) {
                    self.load_env(project, window, cx);
                }
            }
            DocKey::Part {
                project,
                kind,
                name,
            } => {
                if !self.envs.contains_key(project) {
                    self.load_env(project, window, cx);
                }
                // A project has one open part (its fields are its section's): another one's
                // unsaved draft is not dropped for it.
                if let Some((s, open)) = self.env_dirty_card(project) {
                    if (s.key(), open.as_str()) != (*kind, name.as_str()) {
                        let what = if open == "new" {
                            "the new one".to_string()
                        } else {
                            open
                        };
                        self.toast(
                            NotificationType::Warning,
                            format!("Save or revert {what} first"),
                            window,
                            cx,
                        );
                        return;
                    }
                }
                // The project's other part gives way: its tab closes.
                if let Some(old) = self.explorer.tabs.iter().position(|t| {
                    matches!(t, DocKey::Part { project: p, .. } if p == project) && *t != key
                }) {
                    self.explorer.tabs.remove(old);
                }
                self.env_card(project, kind, name.clone(), window, cx);
            }
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
        cx.notify();
    }

    /// A file's editor, made and read once.
    fn ensure_doc(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let key = DocKey::File(f.clone());
        if self.explorer.docs.contains_key(&key) {
            return;
        }
        let editor = crate::env::editor("text", window, cx);
        let k = key.clone();
        cx.subscribe(&editor, move |this, _, ev: &InputEvent, cx| {
            if let (InputEvent::Change, Some(d)) = (ev, this.explorer.docs.get_mut(&k)) {
                if !d.loading {
                    d.dirty = true;
                    cx.notify();
                }
            }
        })
        .detach();
        self.explorer.docs.insert(
            key,
            Doc {
                editor,
                saved: String::new(),
                dirty: false,
                loading: true,
                binary: false,
                goto: None,
            },
        );
        self.read_doc(f, window, cx);
    }

    /// Read the file into its editor (again: what was typed is dropped).
    pub fn read_doc(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo {
            if let Some(d) = self.explorer.docs.get_mut(&DocKey::File(f.clone())) {
                let text = crate::demo::file_text(&f.path);
                let language = crate::lang::language_for_text(&f.path, &text);
                d.editor.update(cx, |s, cx| {
                    s.set_highlighter(language, cx);
                    s.set_value(text.clone(), window, cx)
                });
                d.saved = text;
                d.loading = false;
            }
            return;
        }
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = self.current_profile();
        let (machine, path) = (f.machine.clone(), f.path.clone());
        self.background(
            window,
            cx,
            move || rest::read_text(&conn, &profile, machine.as_deref(), &path),
            move |this, r, window, cx| {
                let key = DocKey::File(f.clone());
                let Some(d) = this.explorer.docs.get_mut(&key) else {
                    return;
                };
                match r {
                    Ok(t) => {
                        d.binary = t.binary || t.truncated;
                        d.loading = true;
                        let language = crate::lang::language_for_text(&f.path, &t.text);
                        d.editor.update(cx, |s, cx| {
                            s.set_highlighter(language, cx);
                            s.set_value(t.text.clone(), window, cx)
                        });
                        d.saved = t.text;
                        d.dirty = false;
                        d.loading = false;
                        if let Some(line) = d.goto.take() {
                            let at = gpui_kit::component::input::Position::new(line, 0);
                            d.editor
                                .update(cx, |s, cx| s.set_cursor_position(at, window, cx));
                        }
                    }
                    Err(err) => {
                        d.loading = false;
                        this.toast(NotificationType::Error, format!("{err:#}"), window, cx);
                    }
                }
                cx.notify();
            },
        );
    }

    /// ⌘S, a document's Save: a file is written; a part saves as its section does.
    pub fn save_doc(&mut self, key: DocKey, window: &mut Window, cx: &mut Context<Self>) {
        let f = match key {
            DocKey::Part { project, kind, .. } => return self.env_save(&project, kind, window, cx),
            // Each group of the settings has its own Save.
            DocKey::Settings { .. } => return,
            DocKey::File(f) => f,
        };
        let Some(d) = self
            .explorer
            .docs
            .get(&DocKey::File(f.clone()))
            .filter(|d| !d.binary && !d.loading)
        else {
            return;
        };
        let text = d.editor.read(cx).value().to_string();
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = self.current_profile();
        let (machine, path, body) = (f.machine.clone(), f.path.clone(), text.clone());
        self.background(
            window,
            cx,
            move || rest::write_text(&conn, &profile, machine.as_deref(), &path, &body),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    if let Some(d) = this.explorer.docs.get_mut(&DocKey::File(f.clone())) {
                        d.saved = text;
                        d.dirty = false;
                    }
                    this.after_write(&f, window, cx);
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// The text as saved, back in the editor.
    pub fn revert_doc(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = self.explorer.docs.get_mut(&DocKey::File(f)) {
            let text = d.saved.clone();
            d.loading = true;
            d.editor.update(cx, |s, cx| s.set_value(text, window, cx));
            d.loading = false;
            d.dirty = false;
        }
        cx.notify();
    }

    /// The document a command means (⌘S, ⌘L): the center's.
    pub fn focused_doc(&self) -> Option<DocKey> {
        self.explorer
            .active
            .clone()
            .filter(|_| self.center == Center::Doc)
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
        let dirty = match &key {
            DocKey::File(_) => self.explorer.docs.get(&key).is_some_and(|d| d.dirty),
            DocKey::Part { .. } => self.part_dirty(&key),
            // Its typed fields stay in the environment until saved or reverted.
            DocKey::Settings { .. } => false,
        };
        let armed = format!("close:{key:?}");
        if dirty && self.explorer.armed.as_deref() != Some(&armed) {
            self.explorer.armed = Some(armed);
            cx.notify();
            return;
        }
        self.explorer.armed = None;
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
        if let DocKey::Part {
            project,
            kind,
            name,
        } = &key
        {
            if let Some(e) = self.envs.get_mut(project) {
                if e.card.as_ref().is_some_and(|(k, n)| k == kind && n == name) {
                    for s in crate::env::SECTIONS {
                        s.close(e);
                    }
                    e.card = None;
                }
            }
        }
        // Unsaved changes asked for already: the next opening reads the file again.
        if dirty {
            self.explorer.docs.remove(&key);
        }
        self.drop_unused(&key);
        cx.notify();
    }

    /// Whether `key` is a part whose draft is not saved (it is its project's open part).
    pub fn part_dirty(&self, key: &DocKey) -> bool {
        let DocKey::Part {
            project,
            kind,
            name,
        } = key
        else {
            return false;
        };
        self.env_dirty_card(project)
            .is_some_and(|(s, open)| s.key() == *kind && open == *name)
    }

    /// A file's editor goes when no tab holds it and nothing is unsaved.
    pub(super) fn drop_unused(&mut self, key: &DocKey) {
        let held = self.explorer.tabs.contains(key);
        if !held && self.explorer.docs.get(key).is_some_and(|d| !d.dirty) {
            self.explorer.docs.remove(key);
        }
    }

    /// A file written here (or made, renamed, removed): what shows it is read again — its folder's
    /// listing, and the environment when it is the agent's text (the soul, a prompt, a skill).
    pub fn after_write(&mut self, f: &FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let env = self.envs.get(&f.project);
        let env_dir = env.map(|e| e.dir.clone());
        let skills: Vec<String> = env
            .map(|e| {
                e.instructions
                    .skills
                    .iter()
                    .map(|k| k.dir.clone())
                    .collect()
            })
            .unwrap_or_default();
        let agents = env_dir.is_some_and(|d| !d.is_empty() && f.within(&d))
            || skills.iter().any(|d| f.within(d));
        if agents && f.machine.is_none() {
            let id = f.project.clone();
            self.load_env(&id, window, cx);
            self.load_prompts(window, cx);
        }
    }
}
