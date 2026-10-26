//! `/files`: the project's own folder in a thread — on this Mac, or on the server of a remote
//! project (through that server's agent, `/api/fs/...`). Folders open in place, a file opens under
//! the insert in an editor with its highlighting; Save writes it back. One live browser for the
//! app (`InsertKind::shared`); each insert keeps its folder and file in its state.

use std::sync::Arc;

use gpui_kit::component::input::{EditorState, InputState, TextareaState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::connection::Connection;
use crate::rest;
use serde_json::{Value, json};

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
    /// A new file or folder being named, in a card under its insert.
    pub form: Option<FileForm>,
}

/// A new file or folder, asked for under the `/files` or `/ftp` insert that wants it: a name,
/// and for a file what it holds.
pub struct FileForm {
    pub insert: String,
    /// On the `/ftp` insert's host (else in `/files`'s folder).
    pub remote: bool,
    pub folder: bool,
    pub name: Entity<InputState>,
    pub content: Entity<TextareaState>,
}

impl LocalFiles {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .scroll_beyond_last_line(Some(0))
                .line_number(true)
                .soft_wrap(false)
        });
        Self {
            owner: None,
            path: String::new(),
            entries: vec![],
            loading: false,
            error: None,
            file: None,
            saved: String::new(),
            truncated: false,
            editor,
            form: None,
        }
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
        self.projects
            .iter()
            .find(|p| p.id == self.thread_project)
            .and_then(|p| p.folder.clone())
            .unwrap_or_default()
    }

    /// Show a folder (and a file in it), as an insert last left it.
    pub fn show_dir(
        &mut self,
        path: String,
        file: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                        entries.sort_by(|a, b| {
                            b.2.cmp(&a.2)
                                .then(a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                        });
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
    pub fn open_local_entry(
        &mut self,
        path: String,
        dir: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                    Ok(t) if t.binary => this.toast(
                        NotificationType::Info,
                        "Not a text file: open it in Zed",
                        window,
                        cx,
                    ),
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
        let (Some((conn, profile)), Some(path)) =
            (self.files_conn(), self.local_files.file.clone())
        else {
            return;
        };
        if self.local_files.truncated {
            self.toast(
                NotificationType::Error,
                "Too big to show whole: not saved",
                window,
                cx,
            );
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

    /// "New file" / "New folder" on an insert's head: the card that names it.
    pub fn open_file_form(
        &mut self,
        insert: String,
        remote: bool,
        folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if folder {
                "Folder name"
            } else {
                "File name, e.g. notes.md"
            })
        });
        name.update(cx, |s, cx| s.focus(window, cx));
        let content = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(6, 100_000)
                .placeholder("What it holds (may stay empty)")
        });
        self.local_files.form = Some(FileForm {
            insert,
            remote,
            folder,
            name,
            content,
        });
        cx.notify();
    }

    /// Create what the card names, in the folder the insert shows, then show the folder again.
    pub fn create_from_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.local_files.form.as_ref() else {
            return;
        };
        let name = f.name.read(cx).value().trim().to_string();
        if name.is_empty() || name.contains('/') {
            self.toast(NotificationType::Error, "A name, without /", window, cx);
            return;
        }
        let text = f.content.read(cx).value().to_string();
        let (folder, remote) = (f.folder, f.remote);
        let profile = self.current_profile();
        if remote {
            let (Some(conn), Some(id)) = (self.conn.clone(), self.files_remote.host.clone()) else {
                return;
            };
            let path = format!("{}/{name}", self.files_remote.path.trim_end_matches('/'));
            self.background(
                window,
                cx,
                move || {
                    if folder {
                        rest::send_json(
                            &conn,
                            "POST",
                            &format!("/api/hosts/{id}/mkdir"),
                            &profile,
                            json!({"path": path}),
                        )
                    } else {
                        rest::send_json(
                            &conn,
                            "PUT",
                            &format!("/api/hosts/{id}/file"),
                            &profile,
                            json!({"path": path, "text": text}),
                        )
                    }
                },
                |this, r, window, cx| match r {
                    Ok(_) => {
                        this.local_files.form = None;
                        let p = this.files_remote.path.clone();
                        this.list_remote(p, window, cx);
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                },
            );
        } else {
            let Some((conn, profile)) = self.files_conn() else {
                return;
            };
            let path = format!("{}/{name}", self.local_files.path.trim_end_matches('/'));
            self.background(
                window,
                cx,
                move || {
                    if folder {
                        rest::send_json(
                            &conn,
                            "POST",
                            "/api/fs/mkdir",
                            &profile,
                            json!({"path": path}),
                        )
                        .map(|_| ())
                    } else {
                        rest::write_text(&conn, &profile, &path, &text)
                    }
                },
                |this, r, window, cx| match r {
                    Ok(()) => {
                        this.local_files.form = None;
                        let p = this.local_files.path.clone();
                        this.show_dir(p, String::new(), window, cx);
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                },
            );
        }
    }

    /// A row's trash: the first click arms it, the second removes the file or folder (a local
    /// folder with what is in it; a host's folder only when empty).
    pub fn delete_entry(
        &mut self,
        remote: bool,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = format!("entry:{path}");
        if self.confirm_delete.as_deref() != Some(key.as_str()) {
            self.confirm_delete = Some(key);
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        let profile = self.current_profile();
        if remote {
            let (Some(conn), Some(id)) = (self.conn.clone(), self.files_remote.host.clone()) else {
                return;
            };
            let q = path.clone();
            self.background(
                window,
                cx,
                move || {
                    rest::send_json(
                        &conn,
                        "DELETE",
                        &format!(
                            "/api/hosts/{id}/file?path={}",
                            crate::inserts::ftp::state::enc(&q)
                        ),
                        &profile,
                        Value::Null,
                    )
                },
                move |this, r, window, cx| match r {
                    Ok(_) => {
                        if this.files_remote.file.as_deref() == Some(path.as_str()) {
                            this.files_remote.file = None;
                        }
                        let p = this.files_remote.path.clone();
                        this.list_remote(p, window, cx);
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                },
            );
        } else {
            let Some((conn, profile)) = self.files_conn() else {
                return;
            };
            let q = path.clone();
            self.background(
                window,
                cx,
                move || {
                    rest::send_json(
                        &conn,
                        "POST",
                        "/api/fs/delete",
                        &profile,
                        json!({"path": q}),
                    )
                },
                move |this, r, window, cx| match r {
                    Ok(_) => {
                        if this.local_files.file.as_deref() == Some(path.as_str()) {
                            this.local_files.file = None;
                        }
                        let p = this.local_files.path.clone();
                        this.show_dir(p, String::new(), window, cx);
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                },
            );
        }
    }
}
