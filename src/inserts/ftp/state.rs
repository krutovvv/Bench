//! A server's files as a thread's subject (`files:<host id>`): FTP, FTPS and SFTP hosts from the
//! agent's store (`/api/hosts`, imported from Transmit and ~/.ssh/config). The block lists a
//! folder; a folder opens in place, a file opens in an editor under the list, ⌘S / Save puts it
//! back. The agent works on the same files with its `remote_files` tool.

use gpui_kit::component::input::{EditorState, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::BenchApp;
use crate::rest;

#[derive(Clone, Debug, Default)]
pub struct RemoteHost {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub user: String,
    /// The project it belongs to; "" = shared by every project.
    pub project: String,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub modified: String,
}

pub struct RemoteFiles {
    pub hosts: Vec<RemoteHost>,
    /// The host the block shows, its folder and what is in it.
    pub host: Option<String>,
    pub path: String,
    pub entries: Vec<Entry>,
    pub loading: bool,
    pub error: Option<String>,
    /// The file open under the list.
    pub file: Option<String>,
    pub editor: Entity<EditorState>,
    pub dirty: bool,
    /// The insert the browser shows now (its id): another one restores its own place first.
    pub owner: Option<String>,
    /// The host's password, typed when the server refused the login.
    pub password: Entity<InputState>,
    /// The new host's form, in the chat area while open (the Files group's "+").
    pub form: bool,
    pub f_name: Entity<InputState>,
    /// 0 FTP, 1 FTPS, 2 SFTP.
    pub f_kind: usize,
    pub f_host: Entity<InputState>,
    pub f_port: Entity<InputState>,
    pub f_user: Entity<InputState>,
    pub f_password: Entity<InputState>,
    pub f_path: Entity<InputState>,
}

/// The kinds a file host can be, in the form's order.
pub const HOST_KINDS: [&str; 3] = ["ftp", "ftps", "sftp"];

impl RemoteFiles {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .scroll_beyond_last_line(Some(0))
                .line_number(true)
                .soft_wrap(false)
        });
        let password = cx.new(|cx| InputState::new(window, cx).placeholder("Password"));
        let mut field = |cx: &mut Context<BenchApp>| cx.new(|cx| InputState::new(window, cx));
        let (f_name, f_host, f_port, f_user, f_path) =
            (field(cx), field(cx), field(cx), field(cx), field(cx));
        let f_password = cx.new(|cx| InputState::new(window, cx));
        Self {
            password,
            form: false,
            f_name,
            f_kind: 0,
            f_host,
            f_port,
            f_user,
            f_password,
            f_path,
            hosts: vec![],
            host: None,
            path: String::new(),
            entries: vec![],
            loading: false,
            error: None,
            file: None,
            editor,
            dirty: false,
            owner: None,
        }
    }
}

fn host_of(v: &Value) -> RemoteHost {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    RemoteHost {
        id: s("id"),
        name: s("name"),
        kind: s("kind"),
        host: s("host"),
        user: s("user"),
        project: s("project"),
    }
}

pub(crate) fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl BenchApp {
    /// The file hosts (FTP, FTPS, SFTP) from the agent's store, once per connection.
    /// A file host's trash in a picker: the first click arms it, the second removes the host
    /// (and its password) from the agent's store.
    pub fn remove_file_host(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let key = format!("host:{id}");
        if self.confirm_delete.as_deref() != Some(key.as_str()) {
            self.confirm_delete = Some(key);
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        let Some(conn) = self.local_conn() else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "DELETE",
                    &format!("/api/hosts/{}", enc(&id)),
                    &profile,
                    Value::Null,
                )
            },
            |this, r, window, cx| match r {
                Ok(_) => this.load_remote_hosts(window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    pub fn load_remote_hosts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, "/api/hosts"),
            |this, r, _, cx| {
                if let Ok(v) = r {
                    let all: Vec<RemoteHost> = v["hosts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(host_of)
                        .collect();
                    // Which project each ~/.ssh/config server belongs to (its row in the store).
                    this.ssh_projects = all
                        .iter()
                        .filter(|h| h.kind == "ssh" && !h.project.is_empty())
                        .map(|h| (h.name.clone(), h.project.clone()))
                        .collect();
                    this.files_remote.hosts = all
                        .into_iter()
                        .filter(|h| matches!(h.kind.as_str(), "ftp" | "ftps" | "sftp"))
                        .collect();
                    this.files_remote
                        .hosts
                        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                }
                cx.notify();
            },
        );
    }

    /// The Files group's "+": the new host's form takes the chat area.
    pub fn new_host_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = crate::app::Panel::Chat;
        self.ssh.form = None;
        let f = &self.files_remote;
        for e in [
            &f.f_name,
            &f.f_host,
            &f.f_port,
            &f.f_user,
            &f.f_password,
            &f.f_path,
        ] {
            e.update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.files_remote.f_kind = 0;
        self.files_remote.form = true;
        self.files_remote
            .f_name
            .update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// Save the form's host in the agent's store and open its thread.
    pub fn save_host(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        let f = &self.files_remote;
        let v = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
        let (name, host) = (v(&f.f_name), v(&f.f_host));
        if host.is_empty() {
            self.toast(
                NotificationType::Error,
                "The host's address is empty".to_string(),
                window,
                cx,
            );
            return;
        }
        let port = v(&f.f_port).parse::<u16>().ok();
        let password = f.f_password.read(cx).value().to_string();
        let path = v(&f.f_path);
        let body = json!({
            "name": if name.is_empty() { host.clone() } else { name },
            "kind": HOST_KINDS[f.f_kind.min(2)],
            "host": host,
            "port": port,
            "user": v(&f.f_user),
            "password": if password.is_empty() { Value::Null } else { json!(password) },
            "path": if path.is_empty() { Value::Null } else { json!(path) },
            // Made in a project's thread: that project's.
            "project": self.thread_project,
        });
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/hosts", &profile, body),
            |this, r, window, cx| match r {
                Ok(v) => {
                    this.files_remote.form = false;
                    this.load_remote_hosts(window, cx);
                    if let Some(id) = v["host"]["id"].as_str() {
                        // Into the thread's file host insert (put in if there is none).
                        this.put_insert(format!("files:{id}"), window, cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    /// The thread's host is shown (after a restart, or opening the thread from elsewhere).
    pub fn ensure_remote_listing(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.files_remote.host.as_deref() != Some(id) {
            // Nothing of the previous host stays on screen (its folder, files, error, open file),
            // whatever this one answers.
            let f = &mut self.files_remote;
            f.host = Some(id.to_string());
            f.path = String::new();
            f.entries.clear();
            f.error = None;
            f.file = None;
            f.dirty = false;
            self.list_remote(String::new(), window, cx);
        }
    }

    pub fn list_remote(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(id)) = (self.local_conn(), self.files_remote.host.clone()) else {
            return;
        };
        self.files_remote.loading = true;
        self.files_remote.error = None;
        let profile = self.current_profile();
        let asked = id.clone();
        self.background(
            window,
            cx,
            move || {
                rest::get_raw(
                    &conn,
                    &profile,
                    &format!("/api/hosts/{id}/ls?path={}", enc(&path)),
                )
            },
            move |this, r, _, cx| {
                let f = &mut this.files_remote;
                // A late answer from a host the block has left.
                if f.host.as_deref() != Some(&asked) {
                    return;
                }
                f.loading = false;
                match r {
                    Ok(v) => {
                        f.path = v["path"].as_str().unwrap_or_default().to_string();
                        f.entries = v["entries"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|e| Entry {
                                name: e["name"].as_str().unwrap_or_default().to_string(),
                                dir: e["type"].as_str() == Some("dir"),
                                size: e["size"].as_u64().unwrap_or(0),
                                modified: e["modified"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .chars()
                                    .take(16)
                                    .collect::<String>()
                                    .replace('T', " "),
                            })
                            .collect();
                    }
                    Err(e) => f.error = Some(format!("{e:#}")),
                }
                cx.notify();
            },
        );
    }

    /// A row's click: a folder opens in place, a file opens in the editor under the list.
    pub fn open_remote_entry(
        &mut self,
        name: String,
        dir: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let base = self.files_remote.path.trim_end_matches('/').to_string();
        let path = if name == ".." {
            base.rsplit_once('/')
                .map(|(p, _)| {
                    if p.is_empty() {
                        "/".to_string()
                    } else {
                        p.to_string()
                    }
                })
                .unwrap_or_else(|| "/".into())
        } else {
            format!("{base}/{name}")
        };
        if dir {
            self.files_remote.file = None;
            self.list_remote(path, window, cx);
            return;
        }
        self.open_remote_path(path, window, cx);
    }

    /// A host's folder and open file as an insert last showed them (its thread reopened, or
    /// another insert showed another host meanwhile). Nothing changes while edits are unsaved.
    pub fn show_files_at(
        &mut self,
        host: &str,
        path: &str,
        file: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.files_remote.dirty {
            return;
        }
        let f = &mut self.files_remote;
        f.host = Some(host.to_string());
        f.path = String::new();
        f.entries.clear();
        f.error = None;
        f.file = None;
        self.list_remote(path.to_string(), window, cx);
        if !file.is_empty() {
            self.open_remote_path(file.to_string(), window, cx);
        }
    }

    /// A file of the shown host, in the editor.
    pub fn open_remote_path(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(id)) = (self.local_conn(), self.files_remote.host.clone()) else {
            return;
        };
        let profile = self.current_profile();
        let p = path.clone();
        self.background(
            window,
            cx,
            move || {
                rest::get_raw(
                    &conn,
                    &profile,
                    &format!("/api/hosts/{id}/file?path={}&max=2000000", enc(&p)),
                )
            },
            move |this, r, window, cx| {
                match r {
                    Ok(v) => match v["text"].as_str() {
                        Some(text) => {
                            let language = crate::lang::language_for_text(&path, text);
                            this.files_remote.file = Some(path);
                            this.files_remote.dirty = false;
                            this.files_remote.editor.update(cx, |s, cx| {
                                s.set_highlighter(language, cx);
                                s.set_value(text.to_string(), window, cx)
                            });
                        }
                        None => this.toast(
                            NotificationType::Info,
                            "Not a text file: download it with the agent",
                            window,
                            cx,
                        ),
                    },
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    pub fn save_host_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(id), Some(path)) = (
            self.local_conn(),
            self.files_remote.host.clone(),
            self.files_remote.file.clone(),
        ) else {
            return;
        };
        let text = self.files_remote.editor.read(cx).value().to_string();
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "PUT",
                    &format!("/api/hosts/{id}/file"),
                    &profile,
                    json!({"path": path, "text": text}),
                )
            },
            |this, r, window, cx| {
                match r {
                    Ok(_) => {
                        this.files_remote.dirty = false;
                        this.toast(NotificationType::Success, "Saved to the server", window, cx);
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    /// The typed password: stored with the host (the person keeps them in Bench's store), then
    /// the folder is read again.
    pub fn save_host_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = self.files_remote.password.read(cx).value().to_string();
        self.store_host_password(password, window, cx);
    }

    /// Transmit keeps its passwords in the keychain: ask for this host's item (macOS asks the
    /// person to allow it), then store it with the host.
    pub fn password_from_keychain(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(h) = self
            .files_remote
            .host
            .as_ref()
            .and_then(|id| self.files_remote.hosts.iter().find(|h| &h.id == id))
            .cloned()
        else {
            return;
        };
        self.background(
            window,
            cx,
            move || {
                let mut args = vec!["find-internet-password", "-s", h.host.as_str()];
                if !h.user.is_empty() {
                    args.extend(["-a", h.user.as_str()]);
                }
                args.push("-w");
                let out = std::process::Command::new("/usr/bin/security")
                    .args(&args)
                    .output()?;
                if !out.status.success() {
                    anyhow::bail!("no password for {} in the keychain", h.host);
                }
                Ok(String::from_utf8_lossy(&out.stdout)
                    .trim_end_matches('\n')
                    .to_string())
            },
            |this, r, window, cx| match r {
                Ok(p) => this.store_host_password(p, window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    fn store_host_password(
        &mut self,
        password: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(conn), Some(id)) = (self.local_conn(), self.files_remote.host.clone()) else {
            return;
        };
        if password.is_empty() {
            return;
        }
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "PUT",
                    &format!("/api/hosts/{id}"),
                    &profile,
                    json!({"password": password}),
                )
            },
            |this, r, window, cx| match r {
                Ok(_) => {
                    this.files_remote
                        .password
                        .update(cx, |s, cx| s.set_value("", window, cx));
                    this.list_remote(String::new(), window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }
}
