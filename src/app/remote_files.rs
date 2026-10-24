//! A server's files as a thread's subject (`files:<host id>`): FTP, FTPS and SFTP hosts from the
//! agent's store (`/api/hosts`, imported from Transmit and ~/.ssh/config). The block lists a
//! folder; a folder opens in place, a file opens in an editor under the list, ⌘S / Save puts it
//! back. The agent works on the same files with its `remote_files` tool.

use gpui_kit::component::input::{EditorState, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::HermesApp;
use crate::rest;

#[derive(Clone, Debug, Default)]
pub struct RemoteHost {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub user: String,
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
    /// The host's password, typed when the server refused the login.
    pub password: Entity<InputState>,
}

impl RemoteFiles {
    pub fn new(window: &mut Window, cx: &mut Context<HermesApp>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .soft_wrap(false)
        });
        let password = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Password"));
        Self {
            password,
            hosts: vec![],
            host: None,
            path: String::new(),
            entries: vec![],
            loading: false,
            error: None,
            file: None,
            editor,
            dirty: false,
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
    }
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl HermesApp {
    /// The file hosts (FTP, FTPS, SFTP) from the agent's store, once per connection.
    pub fn load_remote_hosts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, "/api/hosts"),
            |this, r, _, cx| {
                if let Ok(v) = r {
                    this.files_remote.hosts = v["hosts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(host_of)
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

    /// A file host in the sidebar: its thread, the files on top.
    pub fn open_files_thread(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.files_remote.host.as_deref() != Some(&id) {
            self.files_remote.host = Some(id.clone());
            self.files_remote.file = None;
            self.list_remote(String::new(), window, cx);
        }
        self.open_subject(format!("files:{id}"), cx);
    }

    /// The thread's host is shown (after a restart, or opening the thread from elsewhere).
    pub fn ensure_remote_listing(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.files_remote.host.as_deref() != Some(id) {
            self.files_remote.host = Some(id.to_string());
            self.files_remote.file = None;
            self.list_remote(String::new(), window, cx);
        }
    }

    pub fn list_remote(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(id)) = (self.conn.clone(), self.files_remote.host.clone()) else {
            return;
        };
        self.files_remote.loading = true;
        self.files_remote.error = None;
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &format!("/api/hosts/{id}/ls?path={}", enc(&path))),
            |this, r, _, cx| {
                let f = &mut this.files_remote;
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
                                modified: e["modified"].as_str().unwrap_or_default().chars().take(16).collect::<String>().replace('T', " "),
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
    pub fn open_remote_entry(&mut self, name: String, dir: bool, window: &mut Window, cx: &mut Context<Self>) {
        let base = self.files_remote.path.trim_end_matches('/').to_string();
        let path = if name == ".." {
            base.rsplit_once('/').map(|(p, _)| if p.is_empty() { "/".to_string() } else { p.to_string() }).unwrap_or_else(|| "/".into())
        } else {
            format!("{base}/{name}")
        };
        if dir {
            self.files_remote.file = None;
            self.list_remote(path, window, cx);
            return;
        }
        let (Some(conn), Some(id)) = (self.conn.clone(), self.files_remote.host.clone()) else {
            return;
        };
        let profile = self.current_profile();
        let p = path.clone();
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &format!("/api/hosts/{id}/file?path={}&max=2000000", enc(&p))),
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
                        None => this.toast(NotificationType::Info, "Not a text file: download it with the agent", window, cx),
                    },
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    pub fn save_host_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(id), Some(path)) = (
            self.conn.clone(),
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
            move || rest::send_json(&conn, "PUT", &format!("/api/hosts/{id}/file"), &profile, json!({"path": path, "text": text})),
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
                let out = std::process::Command::new("/usr/bin/security").args(&args).output()?;
                if !out.status.success() {
                    anyhow::bail!("no password for {} in the keychain", h.host);
                }
                Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string())
            },
            |this, r, window, cx| match r {
                Ok(p) => this.store_host_password(p, window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    fn store_host_password(&mut self, password: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(id)) = (self.conn.clone(), self.files_remote.host.clone()) else {
            return;
        };
        if password.is_empty() {
            return;
        }
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "PUT", &format!("/api/hosts/{id}"), &profile, json!({"password": password})),
            |this, r, window, cx| match r {
                Ok(_) => {
                    this.files_remote.password.update(cx, |s, cx| s.set_value("", window, cx));
                    this.list_remote(String::new(), window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    /// The host a thread is about (`files:<id>`).
    pub fn subject_files(subject: &str) -> Option<String> {
        subject.strip_prefix("files:").map(str::to_string)
    }
}
