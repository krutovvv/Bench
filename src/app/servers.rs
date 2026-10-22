//! SSH mode: the servers of `~/.ssh/config` in the sidebar, a terminal per server in the main
//! area — only a terminal. The same file feeds the gateway list: a server there that runs
//! Hermes shows up as a gateway by itself; nothing is installed from the app.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::connection::{self, HostCheck};
use crate::model::Target;
use crate::sshconfig::{self, Server};
use crate::term::Session;

/// The next window starts in SSH mode (on this server, unless empty).
pub static NEXT_SSH: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// An open terminal: one ssh connection.
pub struct Tab {
    pub id: usize,
    pub alias: String,
    pub session: Arc<Session>,
    pub focus: FocusHandle,
    _wake: Task<()>,
}

pub struct Ssh {
    /// The sidebar shows servers and the main area their terminals.
    pub on: bool,
    pub servers: Vec<Server>,
    /// alias -> `user@hostname` (`ssh -G`).
    pub resolved: HashMap<String, String>,
    /// alias -> whether Hermes runs there (for the gateway list only); None while checking.
    pub status: HashMap<String, Option<HostCheck>>,
    pub tabs: Vec<Tab>,
    pub active: Option<usize>,
    next_tab: usize,
    pub search: Entity<InputState>,
    pub query: String,
    /// The server form: Some(None) = new, Some(Some(alias)) = editing.
    pub form: Option<Option<String>>,
    pub f_alias: Entity<InputState>,
    pub f_host: Entity<InputState>,
    pub f_user: Entity<InputState>,
    pub f_port: Entity<InputState>,
    pub f_folder: Entity<InputState>,
    pub f_path: Entity<InputState>,
    pub f_key: Entity<InputState>,
    pub armed_delete: Option<String>,
    /// Collapsed sidebar groups, both modes ("s:<group>" sessions, "srv:<folder>" servers);
    /// remembered in `collapsed.json`.
    pub collapsed: std::collections::HashSet<String>,
    /// Fractional wheel lines not yet scrolled.
    pub scroll_acc: f32,
    _subscriptions: Vec<Subscription>,
}

impl Ssh {
    pub fn new(window: &mut Window, cx: &mut Context<HermesApp>) -> Self {
        let input = |ph: &'static str, window: &mut Window, cx: &mut Context<HermesApp>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(ph))
        };
        let search = input("Search servers", window, cx);
        let f_alias = input("prod-web-1", window, cx);
        let f_host = input("203.0.113.10 or host.example.com", window, cx);
        let f_user = input("root", window, cx);
        let f_port = input("22", window, cx);
        let f_folder = input("Production", window, cx);
        let f_path = input("/var/www", window, cx);
        let f_key = input("~/.ssh/id_ed25519", window, cx);
        let mut subscriptions =
            vec![
                cx.subscribe_in(&search, window, |this, state, ev: &InputEvent, _, cx| {
                    if let InputEvent::Change = ev {
                        this.ssh.query = state.read(cx).value().trim().to_lowercase();
                        cx.notify();
                    }
                }),
            ];
        for field in [
            &f_alias, &f_host, &f_user, &f_port, &f_key, &f_folder, &f_path,
        ] {
            subscriptions.push(cx.subscribe_in(
                field,
                window,
                |this, _, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.save_server(window, cx);
                    }
                },
            ));
        }
        Self {
            on: false,
            servers: sshconfig::load(),
            resolved: HashMap::new(),
            status: HashMap::new(),
            tabs: vec![],
            active: None,
            next_tab: 0,
            search,
            query: String::new(),
            form: None,
            f_alias,
            f_host,
            f_user,
            f_port,
            f_folder,
            f_path,
            f_key,
            armed_delete: None,
            collapsed: std::fs::read(collapsed_path())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default(),
            scroll_acc: 0.,
            _subscriptions: subscriptions,
        }
    }

    /// Folders in display order (top level first, then by name), each with the indices of its
    /// servers that match the search.
    pub fn groups(&self) -> Vec<(String, Vec<usize>)> {
        let mut folders: Vec<String> = self.servers.iter().map(|s| s.folder.clone()).collect();
        folders.sort_by_key(|f| (!f.is_empty(), f.to_lowercase()));
        folders.dedup();
        let q = &self.query;
        folders
            .into_iter()
            .map(|f| {
                let rows: Vec<usize> = (0..self.servers.len())
                    .filter(|&i| {
                        let s = &self.servers[i];
                        s.folder == f
                            && (q.is_empty()
                                || s.alias.to_lowercase().contains(q)
                                || s.hostname.to_lowercase().contains(q)
                                || s.folder.to_lowercase().contains(q))
                    })
                    .collect();
                (f, rows)
            })
            .filter(|(_, rows)| !rows.is_empty())
            .collect()
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    pub fn tab_for(&self, alias: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.alias == alias)
    }

    pub fn is_open(&self, alias: &str) -> bool {
        self.tab_for(alias)
            .is_some_and(|t| !self.tabs[t].session.exited.load(Ordering::SeqCst))
    }
}

fn collapsed_path() -> std::path::PathBuf {
    connection::state_dir().join("collapsed.json")
}

impl HermesApp {
    /// Fold or unfold a sidebar group (its header was clicked).
    pub fn toggle_group(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.ssh.collapsed.remove(&key) {
            self.ssh.collapsed.insert(key);
        }
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(
            collapsed_path(),
            serde_json::to_vec(&self.ssh.collapsed).unwrap_or_default(),
        );
        cx.notify();
    }

    pub fn set_ssh_mode(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.ssh.on = on;
        self.panel = super::Panel::Chat;
        if on {
            self.reload_servers(cx);
            if let Some(t) = self.ssh.active_tab() {
                t.focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// Terminals answer color queries (vim's background check) in the current theme.
    pub fn repaint_terminals(&mut self, cx: &mut Context<Self>) {
        let palette = crate::ui::terminal_palette(cx.theme());
        for t in &self.ssh.tabs {
            t.session.set_palette(palette);
        }
    }

    /// Re-read `~/.ssh/config` (edited elsewhere too) and resolve the targets.
    pub fn reload_servers(&mut self, cx: &mut Context<Self>) {
        self.ssh.servers = sshconfig::load();
        let aliases: Vec<String> = self.ssh.servers.iter().map(|s| s.alias.clone()).collect();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move {
                    aliases
                        .into_iter()
                        .filter_map(|a| sshconfig::resolve(&a).map(|r| (a, r)))
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.ssh.resolved.extend(found);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Ask a server whether Hermes is there (in the background; one check at a time per host).
    pub fn check_host(&mut self, alias: String, cx: &mut Context<Self>) {
        if matches!(self.ssh.status.get(&alias), Some(None)) {
            return;
        }
        self.ssh.status.insert(alias.clone(), None);
        cx.spawn(async move |this, cx| {
            let a = alias.clone();
            let r = cx
                .background_spawn(async move { connection::host_check(&a) })
                .await;
            let _ = this.update(cx, |this, cx| this.host_checked(alias, r, cx));
        })
        .detach();
        cx.notify();
    }

    pub fn host_checked(&mut self, alias: String, r: HostCheck, cx: &mut Context<Self>) {
        self.ssh.status.insert(alias, Some(r));
        self.rebuild_targets();
        cx.notify();
    }

    /// Gateways: This Mac + the servers of `~/.ssh/config` that answered with a working Hermes
    /// (+ the one in use, whatever it answers).
    fn rebuild_targets(&mut self) {
        let mut targets = vec![Target::Local];
        targets.extend(
            self.ssh
                .servers
                .iter()
                .filter(|s| matches!(self.ssh.status.get(&s.alias), Some(Some(HostCheck::Ready))))
                .map(|s| Target::Ssh(s.alias.clone())),
        );
        if !targets.contains(&self.target) {
            targets.push(self.target.clone());
        }
        self.targets = targets;
    }

    /// Called when the gateway menu opens: re-read `~/.ssh/config` and ask its servers again;
    /// one that now runs Hermes joins the list, one that stopped leaves it.
    pub fn refresh_targets(&mut self, cx: &mut Context<Self>) {
        self.reload_servers(cx);
        self.rebuild_targets();
        for s in self.ssh.servers.clone() {
            self.check_host(s.alias, cx);
        }
    }

    pub fn pick_target(&mut self, t: Target, window: &mut Window, cx: &mut Context<Self>) {
        self.ssh.on = false;
        if self.target != t || self.conn.is_none() {
            self.connect(t, window, cx);
        }
        cx.notify();
    }

    /// One click: open the server's terminal (or bring back the one already open).
    pub fn open_server(&mut self, alias: String, window: &mut Window, cx: &mut Context<Self>) {
        self.ssh.form = None;
        self.ssh.armed_delete = None;
        if let Some(t) = self.ssh.tab_for(&alias) {
            if !self.ssh.tabs[t].session.exited.load(Ordering::SeqCst) {
                self.activate_tab(t, window, cx);
                return;
            }
            self.ssh.tabs.remove(t);
        }
        let (tx, rx) = async_channel::bounded::<()>(1);
        let palette = crate::ui::terminal_palette(cx.theme());
        match Session::spawn(&alias, palette, tx) {
            Ok(session) => {
                let wake = cx.spawn(async move |this, cx| {
                    while rx.recv().await.is_ok() {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                });
                self.ssh.next_tab += 1;
                self.ssh.tabs.push(Tab {
                    id: self.ssh.next_tab,
                    alias: alias.clone(),
                    session,
                    focus: cx.focus_handle(),
                    _wake: wake,
                });
                let t = self.ssh.tabs.len() - 1;
                self.activate_tab(t, window, cx);
            }
            Err(e) => self.toast(NotificationType::Error, format!("SSH: {e:#}"), window, cx),
        }
    }

    fn activate_tab(&mut self, t: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.ssh.active = Some(t);
        self.ssh.tabs[t].focus.focus(window, cx);
        cx.notify();
    }

    pub fn reconnect_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(alias) = self.ssh.active_tab().map(|t| t.alias.clone()) {
            self.open_server(alias, window, cx);
        }
    }

    pub fn close_server_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.ssh.active else { return };
        let tab = self.ssh.tabs.remove(t);
        tab.session.kill();
        self.ssh.active = None;
        if !self.ssh.tabs.is_empty() {
            self.activate_tab(t.min(self.ssh.tabs.len() - 1), window, cx);
        }
        cx.notify();
    }

    /// ⌘-click in a server's terminal: a URL opens in the browser; a path (absolute, or `~/…`)
    /// opens in the mini IDE on that server — a file in the editor, a folder as the tree.
    pub fn open_terminal_link(
        &mut self,
        alias: String,
        link: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if link.contains("://") {
            let _ = std::process::Command::new("/usr/bin/open")
                .arg(&link)
                .spawn();
            return;
        }
        if !(link.starts_with('/') || link.starts_with("~/")) {
            return;
        }
        let a = alias.clone();
        let l = link.clone();
        // `~` and "file or folder" are the server's to answer.
        self.background(
            window,
            cx,
            move || {
                let out = crate::ssh::run(
                    &a,
                    &format!(
                        "p={}; case \"$p\" in \"~/\"*) p=\"$HOME/${{p#??}}\";; esac; \
                         if [ -d \"$p\" ]; then echo \"d $p\"; elif [ -f \"$p\" ]; then echo \"f $p\"; fi",
                        crate::ssh::q(&l)
                    ),
                    None,
                    std::time::Duration::from_secs(15),
                )?;
                Ok(String::from_utf8_lossy(&out).trim().to_string())
            },
            move |this, r, window, cx| match r.as_deref().ok().and_then(|o| o.split_once(' ')) {
                Some(("d", dir)) => {
                    this.open_ide_server_at(alias.clone(), Some(dir.to_string()), window, cx);
                }
                Some(("f", file)) => {
                    let file = file.to_string();
                    let dir = file.rsplit_once('/').map_or("/".to_string(), |(d, _)| d.to_string());
                    this.open_ide_server_at(alias.clone(), Some(dir), window, cx);
                    this.open_file(file, window, cx);
                }
                _ => this.toast(
                    NotificationType::Info,
                    format!("{alias}: no file or folder at {link}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// A new window in the same mode: SSH mode opens there too (no server picked yet).
    pub fn open_new_window(&mut self, cx: &mut Context<Self>) {
        if self.ssh.on {
            *NEXT_SSH.lock().unwrap() = Some(String::new());
        }
        self.open_window(None, cx);
    }

    // ---- the server form -----------------------------------------------------

    pub fn edit_server(
        &mut self,
        alias: Option<String>,
        folder: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = alias
            .as_ref()
            .and_then(|a| self.ssh.servers.iter().find(|s| &s.alias == a))
            .cloned()
            .unwrap_or(Server {
                folder: folder.unwrap_or_default(),
                ..Default::default()
            });
        for (field, value) in [
            (&self.ssh.f_alias, s.alias),
            (&self.ssh.f_host, s.hostname),
            (&self.ssh.f_user, s.user),
            (&self.ssh.f_port, s.port),
            (&self.ssh.f_folder, s.folder),
            (&self.ssh.f_path, s.path),
            (&self.ssh.f_key, s.identity),
        ] {
            field.update(cx, |st, cx| st.set_value(value, window, cx));
        }
        self.ssh.form = Some(alias);
        self.ssh.armed_delete = None;
        self.ssh.f_alias.update(cx, |st, cx| st.focus(window, cx));
        cx.notify();
    }

    pub fn save_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(old) = self.ssh.form.clone() else {
            return;
        };
        let v = |e: &Entity<InputState>, cx: &App| e.read(cx).value().trim().to_string();
        let s = Server {
            alias: v(&self.ssh.f_alias, cx),
            hostname: v(&self.ssh.f_host, cx),
            user: v(&self.ssh.f_user, cx),
            port: v(&self.ssh.f_port, cx),
            folder: v(&self.ssh.f_folder, cx),
            path: v(&self.ssh.f_path, cx),
            identity: v(&self.ssh.f_key, cx),
            shared: false,
        };
        if let Err(e) = sshconfig::save(old.as_deref(), &s) {
            self.toast(
                NotificationType::Error,
                format!("~/.ssh/config: {e:#}"),
                window,
                cx,
            );
            return;
        }
        self.ssh.form = None;
        self.ssh.resolved.remove(&s.alias);
        self.reload_servers(cx);
        if let Some(old) = old.filter(|o| *o != s.alias) {
            if let Some(t) = self.ssh.tab_for(&old) {
                self.ssh.tabs[t].alias = s.alias.clone();
            }
        }
        self.open_server(s.alias, window, cx);
    }

    /// "Choose…" beside the key field: the system file picker; a key in the home folder is
    /// written as `~/…`.
    pub fn pick_key_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose key".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(p) = paths.first() else { return };
            let value = sshconfig::tilde(p);
            let _ = this.update_in(cx, |this, window, cx| {
                this.ssh
                    .f_key
                    .update(cx, |s, cx| s.set_value(value, window, cx));
            });
        })
        .detach();
    }

    /// First press arms, the second removes the Host block (a backup of the file is kept).
    pub fn delete_server(&mut self, alias: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.ssh.armed_delete.as_deref() != Some(alias.as_str()) {
            self.ssh.armed_delete = Some(alias);
            cx.notify();
            return;
        }
        self.ssh.armed_delete = None;
        match sshconfig::delete(&alias) {
            Ok(()) => {
                if self.ssh.form == Some(Some(alias.clone())) {
                    self.ssh.form = None;
                }
                self.reload_servers(cx);
            }
            Err(e) => self.toast(
                NotificationType::Error,
                format!("~/.ssh/config: {e:#}"),
                window,
                cx,
            ),
        }
        cx.notify();
    }
}
