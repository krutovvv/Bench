//! `/files`: the project's own folder in a thread — on this Mac, or on the server of a remote
//! project (through that server's agent, `/api/fs/...`). Folders open in place, a file opens under
//! the insert in an editor with its highlighting; Save writes it back. One live browser for the
//! app (`InsertKind::shared`); each insert keeps its folder and file in its state.

use std::sync::Arc;

use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::BenchApp;
use crate::connection::Connection;
use crate::rest;

pub struct LocalFiles {
    /// The insert it shows now (its id): another one restores its own place first.
    pub owner: Option<String>,
    pub path: String,
    /// (name, full path, is a folder), folders first.
    pub entries: Vec<(String, String, bool)>,
    pub loading: bool,
    pub error: Option<String>,
    /// The open file, and its text as last saved (edits show as unsaved against it).
    pub file: Option<String>,
    pub saved: String,
    /// Too big to show whole: shown, never saved back.
    pub truncated: bool,
    pub editor: Entity<EditorState>,
}

impl LocalFiles {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let editor = cx.new(|cx| EditorState::new(window, cx).scroll_beyond_last_line(Some(0)).line_number(true).soft_wrap(false));
        Self { owner: None, path: String::new(), entries: vec![], loading: false, error: None, file: None, saved: String::new(), truncated: false, editor }
    }

    /// The open file was edited since it was opened or saved.
    pub fn dirty(&self, cx: &App) -> bool {
        self.file.is_some() && self.editor.read(cx).value().as_ref() != self.saved.as_str()
    }
}

impl BenchApp {
    /// The agent the open thread's project lives with (a remote project's server's).
    fn files_conn(&self) -> Option<(Arc<Connection>, String)> {
        let project = self.thread_project.clone();
        match self.env_target(&project) {
            Some((conn, _)) => Some((conn, self.current_profile())),
            None => self.conn.clone().map(|c| (c, self.current_profile())),
        }
    }

    /// The thread's project's folder ("" when it has none: the agent's home).
    pub fn project_folder(&self) -> String {
        self.projects.iter().find(|p| p.id == self.thread_project).and_then(|p| p.folder.clone()).unwrap_or_default()
    }

    /// Show a folder (and a file in it), as an insert last left it.
    pub fn show_dir(&mut self, path: String, file: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((conn, profile)) = self.files_conn() else {
            return;
        };
        let f = &mut self.local_files;
        f.loading = true;
        f.error = None;
        f.path = path.clone();
        let asked = path.clone();
        self.background(
            window,
            cx,
            move || rest::list_dir(&conn, &profile, &path),
            move |this, r, _, cx| {
                let f = &mut this.local_files;
                if f.path != asked {
                    return;
                }
                f.loading = false;
                match r {
                    Ok(mut entries) => {
                        entries.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
                        f.entries = entries;
                    }
                    Err(e) => f.error = Some(format!("{e:#}")),
                }
                cx.notify();
            },
        );
        if !file.is_empty() {
            self.open_local_file(file, window, cx);
        }
        cx.notify();
    }

    /// A row's click: a folder opens in place, a file under the insert.
    pub fn open_local_entry(&mut self, path: String, dir: bool, window: &mut Window, cx: &mut Context<Self>) {
        if dir {
            self.show_dir(path, String::new(), window, cx);
        } else {
            self.open_local_file(path, window, cx);
        }
    }

    pub fn open_local_file(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((conn, profile)) = self.files_conn() else {
            return;
        };
        let p = path.clone();
        self.background(
            window,
            cx,
            move || rest::read_text(&conn, &profile, &p),
            move |this, r, window, cx| {
                match r {
                    Ok(t) if t.binary => this.toast(NotificationType::Info, "Not a text file: open it in Zed", window, cx),
                    Ok(t) => {
                        let language = crate::lang::language_for_text(&path, &t.text);
                        let f = &mut this.local_files;
                        f.file = Some(path);
                        f.saved = t.text.clone();
                        f.truncated = t.truncated;
                        f.editor.update(cx, |s, cx| {
                            s.set_highlighter(language, cx);
                            s.set_value(t.text, window, cx)
                        });
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    pub fn save_local_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some((conn, profile)), Some(path)) = (self.files_conn(), self.local_files.file.clone()) else {
            return;
        };
        if self.local_files.truncated {
            self.toast(NotificationType::Error, "Too big to show whole: not saved", window, cx);
            return;
        }
        let text = self.local_files.editor.read(cx).value().to_string();
        let sent = text.clone();
        self.background(
            window,
            cx,
            move || rest::write_text(&conn, &profile, &path, &text),
            move |this, r, window, cx| {
                match r {
                    Ok(()) => this.local_files.saved = sent,
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }
}
