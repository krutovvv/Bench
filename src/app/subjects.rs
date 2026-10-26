//! What a thread is opened with: its project's environment at its top (`env:<id>`, the one
//! subject left), and the `/` list that puts inserts into it (`app::inserts`). A server, a tool
//! or the tasks opened from the keyboard are a new thread with that insert in it.

use gpui_kit::*;

use super::BenchApp;
use crate::devtools::{GROUPS, Tool};

impl BenchApp {
    /// The latest thread about `key` (or the one `subject_target` names), or a new chat that
    /// will carry it.
    pub fn open_subject(&mut self, key: String, cx: &mut Context<Self>) {
        self.panel = super::Panel::Chat;
        // A form in the chat area (a new server) gives way to the thread.
        self.ssh.form = None;
        self.files_remote.form = false;
        self.project_form = None;
        let target = match self.subject_target.take() {
            Some(t) => t,
            None => self
                .sessions
                .iter()
                .find(|s| s.subject == key)
                .map(|s| s.id.clone()),
        };
        match target {
            Some(id) => self.open_session(id, cx),
            None => {
                self.new_session(cx);
                self.new_subject = key;
            }
        }
        cx.notify();
    }

    /// A thread with `key` in it, as the demo starts one: an environment, or a new thread
    /// with that insert.
    pub fn open_subject_key(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        if key.starts_with("env:") {
            self.open_subject(key, cx);
        } else {
            self.new_thread_with(key, window, cx);
        }
    }

    /// A new thread with one insert in it (its session is made at once).
    pub fn new_thread_with(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = super::Panel::Chat;
        self.new_session(cx);
        self.put_insert(key, window, cx);
    }

    /// `/ssh`, `/ftp`, `/browser`, `/tasks`, `/tool` and what follows: the tools that can be
    /// attached to this thread, filtered by the rest of the line. None when the line is not one.
    pub fn tool_slash(&self, text: &str) -> Option<Vec<crate::model::SlashCommand>> {
        let rest = text.strip_prefix('/')?;
        let (word, q) = match rest.split_once(char::is_whitespace) {
            Some((w, q)) => (w, q.trim().to_lowercase()),
            None => return None,
        };
        // A project's thread offers the project's own accesses only (a thread outside projects:
        // those of no project). This Mac's terminal is /terminal.
        let proj = self.thread_project.clone();
        let ours = |p: &str| p == proj;
        let row = |name: String, desc: String, key: String| crate::model::SlashCommand {
            name,
            desc,
            category: "Tool".into(),
            key,
        };
        let mut rows: Vec<crate::model::SlashCommand> = match word.to_lowercase().as_str() {
            "terminal" => {
                let host = self.thread_host();
                let desc = if host.is_empty() {
                    "this Mac's terminal".to_string()
                } else {
                    format!("the terminal on {host}, where this project runs")
                };
                vec![row("/terminal".into(), desc, "ssh:local".into())]
            }
            "ssh" => {
                let mut v = vec![];
                v.extend(
                    self.ssh
                        .servers
                        .iter()
                        .filter(|s| {
                            ours(
                                self.ssh_projects
                                    .get(&s.alias)
                                    .map(String::as_str)
                                    .unwrap_or(""),
                            )
                        })
                        .map(|s| {
                            row(
                                format!("/ssh {}", s.alias),
                                s.hostname.clone(),
                                format!("ssh:{}", s.alias),
                            )
                        }),
                );
                v.push(row(
                    "/ssh New server…".into(),
                    "add one to ~/.ssh/config".into(),
                    "new:ssh".into(),
                ));
                v
            }
            "ftp" => {
                let mut v: Vec<_> = self
                    .files_remote
                    .hosts
                    .iter()
                    .filter(|h| ours(&h.project))
                    .map(|h| {
                        row(
                            format!("/ftp {}", h.name),
                            format!("{}@{}", h.user, h.host),
                            format!("files:{}", h.id),
                        )
                    })
                    .collect();
                v.push(row(
                    "/ftp New host…".into(),
                    "FTP, FTPS or SFTP".into(),
                    "new:ftp".into(),
                ));
                v
            }
            "browser" => vec![row(
                "/browser".into(),
                "the server's browser: its tab, live".into(),
                "browser".into(),
            )],
            "tasks" => crate::tasks::LISTS
                .iter()
                .map(|l| {
                    row(
                        format!("/tasks {}", l.name()),
                        "the task lists, on this one".into(),
                        format!("tasks:{}", l.name().to_lowercase()),
                    )
                })
                .collect(),
            "tools" | "tool" => GROUPS
                .iter()
                .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
                .map(|(t, g)| {
                    row(
                        format!("/tools {}", t.name()),
                        g.to_string(),
                        format!("tools:{}", tool_key(t)),
                    )
                })
                .collect(),
            _ => return None,
        };
        if !q.is_empty() {
            rows.retain(|r| {
                r.name.to_lowercase().contains(&q) || r.desc.to_lowercase().contains(&q)
            });
        }
        Some(rows)
    }

    /// The words that put an insert in, for the list of a bare `/word`.
    pub fn tool_words() -> Vec<crate::model::SlashCommand> {
        crate::ui::inserts::words()
    }

    /// A row picked from the `/` list: a form (a new server or host), or an insert.
    pub fn attach_tool(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        match key.as_str() {
            // From the composer: the form shows at the thread's end.
            "new:ssh" => {
                self.form_after = None;
                return self.edit_server(None, None, window, cx);
            }
            "new:ftp" => {
                self.form_after = None;
                return self.new_host_form(window, cx);
            }
            _ => {}
        }
        // Everything else is an insert, put where the conversation is now.
        self.put_insert(key, window, cx);
    }

    /// A server's terminal in a new thread.
    pub fn open_server_thread(
        &mut self,
        alias: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = if alias.is_empty() {
            "ssh:local".to_string()
        } else {
            format!("ssh:{alias}")
        };
        self.new_thread_with(key, window, cx);
    }

    /// A terminal insert's own shell on `alias`, started if there is none, it ended, or the
    /// insert switched to another server.
    pub fn insert_terminal(
        &mut self,
        insert: &str,
        alias: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some(t) = self.ssh.tab_of_insert(insert) {
            let tab = &self.ssh.tabs[t];
            if tab.alias == alias && !tab.session.exited.load(std::sync::atomic::Ordering::SeqCst) {
                return Some(t);
            }
            self.ssh.tabs[t].session.kill();
            self.ssh.tabs.remove(t);
        }
        let t = self.spawn_terminal_tab(alias, window, cx)?;
        self.ssh.tabs[t].insert = insert.to_string();
        Some(t)
    }

    /// A tool in a new thread.
    pub fn open_tool_thread(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.new_thread_with(format!("tools:{}", tool_key(tool)), window, cx);
    }

    /// A thread's name until it has a title of its own: a project's environment.
    pub fn subject_title(&self) -> Option<String> {
        let s = self.current_subject();
        let id = s.strip_prefix("env:")?;
        let name = self
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| crate::app::LOCAL_HOST.into());
        Some(format!("{name}: environment"))
    }

    /// The open thread's key for things kept per thread: its session id, or "draft".
    pub fn thread_key(&self) -> String {
        self.session.clone().unwrap_or_else(|| "draft".into())
    }

    /// "!command": typed into the thread's terminal (its last terminal insert); a thread with
    /// none gets this Mac's, in the thread's folder.
    pub fn run_bang(&mut self, command: String, window: &mut Window, cx: &mut Context<Self>) {
        let last = self
            .thread_inserts()
            .into_iter()
            .rev()
            .find(|i| i.key.starts_with("ssh:"));
        let (ins, fresh) = match last {
            Some(i) => (i, false),
            None => {
                self.put_insert("ssh:local".into(), window, cx);
                match self
                    .thread_inserts()
                    .into_iter()
                    .rev()
                    .find(|i| i.key == "ssh:local")
                {
                    Some(i) => (i, true),
                    None => return,
                }
            }
        };
        let alias = match &ins.key[4..] {
            "local" | "" => self.thread_machine(),
            a => a.to_string(),
        };
        if let Some(t) = self.insert_terminal(&ins.id, &alias, window, cx) {
            // Start where the thread works.
            if fresh {
                if let Some(dir) = self.folder_of_current() {
                    self.ssh.tabs[t]
                        .session
                        .write(format!("cd {}\r", shell_quote(&dir)).as_bytes());
                }
            }
            if !command.is_empty() {
                self.ssh.tabs[t]
                    .session
                    .write(format!("{command}\r").as_bytes());
            }
            self.ssh.tabs[t].background = false;
            self.ssh.tabs[t].focus.focus(window, cx);
        }
        cx.notify();
    }

    /// The open thread's folder, when it works in one.
    fn folder_of_current(&self) -> Option<String> {
        self.session
            .as_ref()
            .and_then(|id| self.folder_of(id))
            .or_else(|| self.new_project.clone())
    }

    /// What the thread's live block shows right now, for the agent (sent with each message, not
    /// shown): the terminal's last lines, the folder and open file of a host, the tool's input
    /// and output. Task lists need none: the agent reads them with its tool.
    pub fn block_context(&self, cx: &App) -> String {
        [self.subject_context(cx), self.inserts_context(cx)]
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// The thread's own block: a project's environment (the soul is in the agent's prompt already).
    fn subject_context(&self, _cx: &App) -> String {
        if !self.current_subject().starts_with("env:") {
            return String::new();
        }
        let Some(e) = &self.project_env else {
            return String::new();
        };
        let mcp: Vec<&str> = e.mcp.iter().map(|m| m.0.as_str()).collect();
        let skills: Vec<&str> = e.skills.iter().map(|k| k.name.as_str()).collect();
        format!(
            "The environment block shows the folder {}, MCP servers: {}, skills: {}.",
            e.dir,
            if mcp.is_empty() {
                "none".to_string()
            } else {
                mcp.join(", ")
            },
            if skills.is_empty() {
                "none".to_string()
            } else {
                skills.join(", ")
            },
        )
    }
}

/// A tool's name in a subject: "base64-image".
pub fn tool_key(tool: Tool) -> String {
    tool.name()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
