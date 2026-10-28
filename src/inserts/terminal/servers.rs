//! The servers of `~/.ssh/config` (a new project picks its machine from them) and the shells of
//! the terminal inserts. A server's agent is added in the gear (`settings/agent_form.rs`).

use std::sync::Arc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::connection;
use crate::inserts::terminal::session::Session;
use crate::system::sshconfig::{self, Server};

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
    pub servers: Vec<Server>,
    pub tabs: Vec<Tab>,
    next_tab: usize,
    /// Collapsed sidebar groups, both modes ("s:<group>" sessions, "srv:<folder>" servers);
    /// remembered in `collapsed.json`.
    pub collapsed: std::collections::HashSet<String>,
    /// Fractional wheel lines not yet scrolled.
    pub scroll_acc: f32,
}

impl Ssh {
    pub fn new() -> Self {
        Self {
            servers: sshconfig::load(),
            tabs: vec![],
            next_tab: 0,
            collapsed: std::fs::read(collapsed_path())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default(),
            scroll_acc: 0.,
        }
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

    /// Terminals answer color queries (vim's background check) in the current theme.
    pub fn repaint_terminals(&mut self, cx: &mut Context<Self>) {
        let palette = crate::inserts::terminal::view::palette(cx.theme());
        for t in &self.ssh.tabs {
            t.session.set_palette(palette);
        }
    }

    /// Re-read `~/.ssh/config` (edited elsewhere too).
    pub fn reload_servers(&mut self, cx: &mut Context<Self>) {
        // The demo shows made-up servers, never the person's ~/.ssh/config.
        if self.demo {
            return;
        }
        self.ssh.servers = sshconfig::load();
        cx.notify();
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
        let palette = crate::inserts::terminal::view::palette(cx.theme());
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

    /// ⌘-click in a terminal: a URL opens in the browser; a path — absolute, `~/…`, or relative
    /// to the terminal's folder (`wp-config.php` in an `ls`) — opens in the Explorer: the file
    /// in the editor at its `:line`, in the open thread's project, on that machine.
    pub fn open_terminal_link(
        &mut self,
        alias: String,
        link: crate::inserts::terminal::session::TermLink,
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
        let project = self.thread_ref(cx).project_or_host();
        if alias == crate::inserts::terminal::session::LOCAL {
            self.open_local_link(&project, &path, line, window, cx);
            return;
        }
        let a = alias.clone();
        let l = path.clone();
        // `~` and "file or folder" are the server's to answer.
        self.background(
            window,
            cx,
            move || {
                let out = crate::system::ssh::run(
                    &a,
                    &format!(
                        "p={}; case \"$p\" in \"~\") p=\"$HOME\";; \"~/\"*) p=\"$HOME/${{p#??}}\";; esac; \
                         if [ -d \"$p\" ]; then echo \"d $p\"; elif [ -f \"$p\" ]; then echo \"f $p\"; fi",
                        crate::system::ssh::q(&l)
                    ),
                    None,
                    std::time::Duration::from_secs(15),
                )?;
                Ok(String::from_utf8_lossy(&out).trim().to_string())
            },
            move |this, r, window, cx| match r.as_deref().ok().and_then(|o| o.split_once(' ')) {
                Some(("f", found)) => {
                    this.open_terminal_file(&project, Some(&alias), found, line, window, cx)
                }
                Some((_, found)) => this.toast(
                    NotificationType::Info,
                    format!("{alias}: {found} is a folder, not a file"),
                    window,
                    cx,
                ),
                _ => this.toast(
                    NotificationType::Info,
                    format!("{alias}: no file or folder at {path}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// ⌘-click on a path in this machine's terminal: it opens in the Explorer.
    fn open_local_link(
        &mut self,
        project: &str,
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
        if p.is_file() {
            self.open_terminal_file(project, None, &path, line, window, cx);
        } else {
            self.toast(
                NotificationType::Info,
                format!("No file at {link}"),
                window,
                cx,
            );
        }
    }

    /// A terminal's ⌘-click path in the Explorer: its tab on that machine, the caret at `line`.
    fn open_terminal_file(
        &mut self,
        project: &str,
        machine: Option<&str>,
        path: &str,
        line: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let f = crate::explorer::FileRef {
            project: project.to_string(),
            machine: machine.map(|m| m.to_string()),
            path: path.to_string(),
        };
        self.show_explorer(project.to_string(), window, cx);
        self.open_doc(crate::explorer::DocKey::File(f.clone()), window, cx);
        if let Some(line) = line {
            self.goto_line(f, line, window, cx);
        }
    }
}
