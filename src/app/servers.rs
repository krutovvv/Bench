//! SSH mode: the servers of `~/.ssh/config` in the sidebar, a terminal per server in the main
//! area — only a terminal. The same file feeds the gateway list: the servers there that can
//! run the agent (Node 22.18+) show up as gateways by themselves; Bench installs and starts the
//! agent there on connect.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::BenchApp;
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
    /// Its block in the thread shows only its head.
    pub collapsed: bool,
    /// Sent to the background: it keeps running, its block is a head with a badge, the status
    /// bar lists it.
    pub background: bool,
    /// The insert it belongs to (each terminal insert has a shell of its own); "" none.
    pub insert: String,
    _wake: Task<()>,
}

pub struct Ssh {
    /// The sidebar shows servers and the main area their terminals.
    pub on: bool,
    pub servers: Vec<Server>,
    /// alias -> `user@hostname` (`ssh -G`).
    pub resolved: HashMap<String, String>,
    /// alias -> whether the agent can run there (for the gateway list only); None while checking.
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
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let input = |ph: &'static str, window: &mut Window, cx: &mut Context<BenchApp>| {
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

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    pub fn tab_for(&self, alias: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.alias == alias && t.insert.is_empty())
    }

    /// The shell of a terminal insert.
    pub fn tab_of_insert(&self, insert: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.insert == insert)
    }
}

fn collapsed_path() -> std::path::PathBuf {
    connection::state_dir().join("collapsed.json")
}

impl BenchApp {
    /// Fold or unfold a sidebar group (its header was clicked).
    pub fn toggle_group(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.ssh.collapsed.remove(&key) {
            self.ssh.collapsed.insert(key);
        }
        // The demo folds and unfolds in memory: the person's own folds stay as they were.
        if self.demo {
            cx.notify();
            return;
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
        // Sessions or servers: either leaves Tools.
        self.tools.on = false;
        self.tasks.on = false;
        self.panel = super::Panel::Chat;
        if on {
            self.reload_servers(cx);
            // With nothing open, the terminal of this machine.
            if self.ssh.tabs.is_empty() {
                self.open_server(crate::term::LOCAL.to_string(), window, cx);
            }
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
        // The demo shows made-up servers, never the person's ~/.ssh/config.
        if self.demo {
            return;
        }
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

    pub fn host_checked(&mut self, alias: String, r: HostCheck, cx: &mut Context<Self>) {
        self.ssh.status.insert(alias, Some(r));
        self.rebuild_targets();
        cx.notify();
    }

    /// Gateways: This Mac + the servers of `~/.ssh/config` that answered with a Node the agent
    /// runs on (+ the one in use, whatever it answers).
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
        if let Some(t) = self.spawn_terminal_tab(&alias, window, cx) {
            self.activate_tab(t, window, cx);
        }
    }

    /// A new terminal tab (ssh to `alias`, or a local shell for `term::LOCAL`); its index.
    pub fn spawn_terminal_tab(
        &mut self,
        alias: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        // The desktop demo never opens a real shell or ssh (the browser one shows a note).
        if self.demo && cfg!(not(target_family = "wasm")) {
            return None;
        }
        let (tx, rx) = async_channel::bounded::<()>(1);
        let palette = crate::ui::terminal_palette(cx.theme());
        match Session::spawn(alias, palette, tx) {
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
                    alias: alias.to_string(),
                    session,
                    focus: cx.focus_handle(),
                    collapsed: false,
                    background: false,
                    insert: String::new(),
                    _wake: wake,
                });
                Some(self.ssh.tabs.len() - 1)
            }
            Err(e) => {
                self.toast(
                    NotificationType::Error,
                    format!("Terminal: {e:#}"),
                    window,
                    cx,
                );
                None
            }
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
        } else if tab.alias != crate::term::LOCAL {
            // The last server closed: back to this machine's terminal (closing that one leaves
            // the window empty).
            self.open_server(crate::term::LOCAL.to_string(), window, cx);
        }
        cx.notify();
    }

    /// ⌘-click in a terminal: a URL opens in the browser; a path — absolute, `~/…`, or relative
    /// to the terminal's folder (`wp-config.php` in an `ls`) — opens in the mini IDE on that
    /// machine: a file in the editor (at its `:line`), a folder as the tree. A file under the
    /// tree already open keeps that tree.
    pub fn open_terminal_link(
        &mut self,
        alias: String,
        link: crate::term::TermLink,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if link.target.contains("://") {
            let _ = std::process::Command::new("/usr/bin/open")
                .arg(&link.target)
                .spawn();
            return;
        }
        let path = if link.target.starts_with('/') || link.target.starts_with('~') {
            link.target.clone()
        } else {
            match link.cwd.as_deref() {
                Some(cwd) => format!("{}/{}", cwd.trim_end_matches('/'), link.target),
                // No folder known: relative to home (where a new shell and `ssh host cmd` start).
                None => format!("~/{}", link.target),
            }
        };
        let line = link.line;
        if alias == crate::term::LOCAL {
            self.open_local_link(&path, line, window, cx);
            return;
        }
        let a = alias.clone();
        let l = path.clone();
        // `~` and "file or folder" are the server's to answer.
        self.background(
            window,
            cx,
            move || {
                let out = crate::ssh::run(
                    &a,
                    &format!(
                        "p={}; case \"$p\" in \"~\") p=\"$HOME\";; \"~/\"*) p=\"$HOME/${{p#??}}\";; esac; \
                         if [ -d \"$p\" ]; then echo \"d $p\"; elif [ -f \"$p\" ]; then echo \"f $p\"; fi",
                        crate::ssh::q(&l)
                    ),
                    None,
                    std::time::Duration::from_secs(15),
                )?;
                Ok(String::from_utf8_lossy(&out).trim().to_string())
            },
            move |this, r, window, cx| match r.as_deref().ok().and_then(|o| o.split_once(' ')) {
                Some((_, found)) => this.open_in_zed(found, line, Some(&alias), cx),
                _ => this.toast(
                    NotificationType::Info,
                    format!("{alias}: no file or folder at {path}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// ⌘-click on a path in this machine's terminal: it opens in Zed.
    fn open_local_link(
        &mut self,
        link: &str,
        line: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = match link.strip_prefix("~/") {
            Some(rest) => format!("{home}/{rest}"),
            None if link == "~" => home,
            None => link.to_string(),
        };
        let p = std::path::Path::new(&path);
        if p.exists() {
            self.open_in_zed(&path, line, None, cx);
        } else {
            self.toast(
                NotificationType::Info,
                format!("No file or folder at {link}"),
                window,
                cx,
            );
        }
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
        self.ssh.form = None;
        self.reload_servers(cx);
        // Added in a project's thread: the project's server (its row in the agent's store).
        if !self.thread_project.is_empty() {
            if let Some(conn) = self.conn.clone() {
                let body = serde_json::json!({
                    "name": s.alias, "kind": "ssh", "host": if s.hostname.is_empty() { s.alias.clone() } else { s.hostname.clone() },
                    "user": s.user, "source": format!("ssh_config:{}", s.alias), "project": self.thread_project,
                });
                self.ssh_projects.insert(s.alias.clone(), self.thread_project.clone());
                let profile = self.current_profile();
                self.background(window, cx, move || crate::rest::send_json(&conn, "POST", "/api/hosts", &profile, body), |_, _, _, _| {});
            }
        }
        // Into the thread it was added from.
        self.attach_tool(format!("ssh:{}", s.alias), window, cx);
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
}
