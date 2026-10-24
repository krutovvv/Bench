//! A thread's subject: what it is about, drawn live at its top, as in Zed's Delta (a notebook's
//! cell): a task list (`tasks:today`), a server's terminal (`ssh:<alias>`), a tool (`tool:<name>`).
//! The sidebar lists the subjects; a subject has many threads (one job each): a click opens the
//! latest, its "+" a new one, and the open subject's threads are listed under it.
//! Nothing is a separate mode: everything is a thread.

use gpui_kit::*;

use super::HermesApp;
use crate::devtools::{GROUPS, Tool};

impl HermesApp {
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
            None => self.sessions.iter().find(|s| s.subject == key).map(|s| s.id.clone()),
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

    /// Any subject by its key, as the sidebar would open it.
    pub fn open_subject_key(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(list) = Self::subject_list(&key) {
            self.open_task_list(list, window, cx);
        } else if let Some(alias) = Self::subject_server(&key) {
            self.open_server_thread(alias, window, cx);
        } else if let Some(id) = Self::subject_files(&key) {
            self.open_files_thread(id, window, cx);
        } else if let Some(tool) = Self::subject_tool(&key) {
            self.open_tool_thread(tool, window, cx);
        } else if Self::subject_browser(&key).is_some() {
            // That browser session if it exists, else a new one.
            match self.sessions.iter().find(|s| s.subject == key).map(|s| s.id.clone()) {
                Some(id) => self.open_browser_thread(id, cx),
                None => self.new_browser_thread(cx),
            }
        } else {
            self.open_subject(key, cx);
        }
    }

    /// `/ssh`, `/ftp`, `/browser`, `/tasks`, `/tool` and what follows: the tools that can be
    /// attached to this thread, filtered by the rest of the line. None when the line is not one.
    pub fn tool_slash(&self, text: &str) -> Option<Vec<crate::model::SlashCommand>> {
        let rest = text.strip_prefix('/')?;
        let (word, q) = match rest.split_once(char::is_whitespace) {
            Some((w, q)) => (w, q.trim().to_lowercase()),
            None => return None,
        };
        let row = |name: String, desc: String, key: String| crate::model::SlashCommand {
            name,
            desc,
            category: "Tool".into(),
            key,
        };
        let mut rows: Vec<crate::model::SlashCommand> = match word.to_lowercase().as_str() {
            "ssh" => {
                let mut v = vec![row("/ssh Terminal".into(), "this Mac".into(), format!("ssh:{}", crate::term::LOCAL))];
                v.extend(self.ssh.servers.iter().map(|s| row(format!("/ssh {}", s.alias), s.hostname.clone(), format!("ssh:{}", s.alias))));
                v.push(row("/ssh New server…".into(), "add one to ~/.ssh/config".into(), "new:ssh".into()));
                v
            }
            "ftp" => {
                let mut v: Vec<_> = self
                    .files_remote
                    .hosts
                    .iter()
                    .map(|h| row(format!("/ftp {}", h.name), format!("{}@{}", h.user, h.host), format!("files:{}", h.id)))
                    .collect();
                v.push(row("/ftp New host…".into(), "FTP, FTPS or SFTP".into(), "new:ftp".into()));
                v
            }
            "browser" => vec![row("/browser".into(), "the server's browser, each step's page live".into(), "browser:new".into())],
            "tasks" => crate::tasks::LISTS
                .iter()
                .map(|l| row(format!("/tasks {}", l.name()), "the task list".into(), Self::tasks_subject(l)))
                .collect(),
            "tool" => GROUPS
                .iter()
                .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
                .map(|(t, g)| row(format!("/tool {}", t.name()), g.to_string(), format!("tool:{}", tool_key(t))))
                .collect(),
            _ => return None,
        };
        if !q.is_empty() {
            rows.retain(|r| r.name.to_lowercase().contains(&q) || r.desc.to_lowercase().contains(&q));
        }
        Some(rows)
    }

    /// The words that attach a tool, for the list of a bare `/word`.
    pub fn tool_words() -> Vec<crate::model::SlashCommand> {
        [
            ("/ssh", "A server's terminal in this thread"),
            ("/ftp", "A file host's folders and files in this thread"),
            ("/browser", "The server's browser: each step's page live"),
            ("/tasks", "A task list in this thread"),
            ("/tool", "An everyday tool (Base64, regex, cron…) in this thread"),
        ]
        .into_iter()
        .map(|(n, d)| crate::model::SlashCommand { name: n.into(), desc: d.into(), category: "Tool".into(), key: String::new() })
        .collect()
    }

    /// A tool picked from the `/` list: its block, ready, at the top of this thread from now on
    /// (a new chat carries it when it is created).
    pub fn attach_tool(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        match key.as_str() {
            "new:ssh" => return self.edit_server(None, None, window, cx),
            "new:ftp" => return self.new_host_form(window, cx),
            _ => {}
        }
        let key = if key == "browser:new" {
            self.load_sites();
            self.ensure_browser(cx);
            format!("browser:{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
        } else {
            key
        };
        if let Some(alias) = Self::subject_server(&key) {
            self.ensure_terminal(&alias, window, cx);
        } else if let Some(id) = Self::subject_files(&key) {
            self.ensure_remote_listing(&id, window, cx);
        } else if let Some(list) = Self::subject_list(&key) {
            self.start_reminders(cx);
            self.load_tasks();
            self.select_task_list(list, window, cx);
        } else if let Some(tool) = Self::subject_tool(&key) {
            self.select_tool(tool, window, cx);
        }
        match self.session.clone() {
            Some(id) => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    s.subject = key.clone();
                }
                self.cmd(crate::gateway::Cmd::SetSubject { session: id, subject: key });
            }
            None => self.new_subject = key,
        }
        cx.notify();
    }

    /// A server: its thread, the terminal on top (opened on first use, kept while the app runs).
    pub fn open_server_thread(&mut self, alias: String, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_terminal(&alias, window, cx);
        self.open_subject(format!("ssh:{alias}"), cx);
        if let Some(t) = self.ssh.tab_for(&alias) {
            self.ssh.tabs[t].focus.focus(window, cx);
        }
    }

    /// The terminal tab of `alias`, started if there is none (or it ended).
    pub fn ensure_terminal(&mut self, alias: &str, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        if let Some(t) = self.ssh.tab_for(alias) {
            if !self.ssh.tabs[t]
                .session
                .exited
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return Some(t);
            }
            self.ssh.tabs.remove(t);
        }
        self.spawn_terminal_tab(alias, window, cx)
    }

    /// A tool: its thread, the tool on top.
    pub fn open_tool_thread(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.select_tool(tool, window, cx);
        self.open_subject(format!("tool:{}", tool_key(tool)), cx);
    }

    /// A subject thread's name until it has a title of its own: the list, server, host or tool.
    pub fn subject_title(&self) -> Option<String> {
        let s = self.current_subject();
        if let Some(l) = Self::subject_list(&s) {
            return Some(l.name());
        }
        if let Some(a) = Self::subject_server(&s) {
            return Some(if a == crate::term::LOCAL { "Terminal".into() } else { a });
        }
        if let Some(id) = Self::subject_files(&s) {
            return self.files_remote.hosts.iter().find(|h| h.id == id).map(|h| h.name.clone());
        }
        if Self::subject_browser(&s).is_some() {
            return Some("Browser".into());
        }
        if let Some(id) = s.strip_prefix("env:") {
            let name = self.projects.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
            return Some(format!("{name}: environment"));
        }
        Self::subject_tool(&s).map(|t| t.name().to_string())
    }

    /// The open thread's key for things kept per thread: its session id, or "draft".
    pub fn thread_key(&self) -> String {
        self.session.clone().unwrap_or_else(|| "draft".into())
    }

    /// The terminal "!" opened in this thread (a tab of its own).
    pub fn thread_terminal(&self) -> Option<usize> {
        let alias = self.thread_terms.get(&self.thread_key())?;
        self.ssh.tab_for(alias)
    }

    /// "!command": a server thread types it into its terminal; any other thread opens a terminal
    /// of its own (this Mac, in the thread's folder) under the conversation and runs it there.
    pub fn run_bang(&mut self, command: String, window: &mut Window, cx: &mut Context<Self>) {
        let subject = self.current_subject();
        let tab = if let Some(alias) = Self::subject_server(&subject) {
            self.ensure_terminal(&alias, window, cx)
        } else {
            match self.thread_terminal().filter(|t| {
                !self.ssh.tabs[*t].session.exited.load(std::sync::atomic::Ordering::SeqCst)
            }) {
                Some(t) => Some(t),
                None => {
                    let t = self.spawn_terminal_tab(crate::term::LOCAL, window, cx);
                    if let Some(t) = t {
                        let alias = format!("thread:{}", self.thread_key());
                        self.ssh.tabs[t].alias = alias.clone();
                        self.thread_terms.insert(self.thread_key(), alias);
                        // Start where the thread works.
                        if let Some(dir) = self.folder_of_current() {
                            self.ssh.tabs[t].session.write(format!("cd {}\r", shell_quote(&dir)).as_bytes());
                        }
                    }
                    t
                }
            }
        };
        if let Some(t) = tab {
            if !command.is_empty() {
                self.ssh.tabs[t].session.write(format!("{command}\r").as_bytes());
            }
            self.ssh.tabs[t].focus.focus(window, cx);
        }
        cx.notify();
    }

    /// The open thread's folder, when it works in one.
    fn folder_of_current(&self) -> Option<String> {
        self.session.as_ref().and_then(|id| self.folder_of(id)).or_else(|| self.new_project.clone())
    }

    /// What the thread's live block shows right now, for the agent (sent with each message, not
    /// shown): the terminal's last lines, the folder and open file of a host, the tool's input
    /// and output. Task lists need none: the agent reads them with its tool.
    pub fn block_context(&self, cx: &App) -> String {
        let subject = self.current_subject();
        let clip = |s: &str, n: usize| -> String {
            if s.chars().count() <= n {
                s.to_string()
            } else {
                format!("{}… [cut]", s.chars().take(n).collect::<String>())
            }
        };
        if let Some(alias) = Self::subject_server(&subject) {
            let name = if alias == crate::term::LOCAL { "this Mac".to_string() } else { alias.clone() };
            return match self.ssh.tab_for(&alias) {
                Some(t) => format!(
                    "The terminal of {name} shows (its last lines):\n```\n{}\n```",
                    self.ssh.tabs[t].session.screen_text(60)
                ),
                None => String::new(),
            };
        }
        if let Some(id) = Self::subject_files(&subject) {
            let f = &self.files_remote;
            if f.host.as_deref() != Some(&id) {
                return String::new();
            }
            let names: Vec<String> = f
                .entries
                .iter()
                .take(80)
                .map(|e| if e.dir { format!("{}/", e.name) } else { e.name.clone() })
                .collect();
            let mut out = format!("The files block is in the folder {} (host id {id}). It lists: {}", f.path, names.join(", "));
            if let Some(file) = &f.file {
                out.push_str(&format!(
                    "\nOpen in its editor: {file}{}\n```\n{}\n```",
                    if f.dirty { " (edited, not saved)" } else { "" },
                    clip(&f.editor.read(cx).value(), 6000)
                ));
            }
            return out;
        }
        if let Some(tool) = Self::subject_tool(&subject) {
            let input = self.tools.input.read(cx).value().to_string();
            let output = self.tools.output.read(cx).value().to_string();
            return format!(
                "The tool block is {} ({}).\nIts input:\n```\n{}\n```\nIts output:\n```\n{}\n```",
                tool.name(),
                tool.hint(),
                clip(&input, 6000),
                clip(&output, 6000)
            );
        }
        // A project's environment: what its block lists (the soul is in the agent's prompt already).
        if subject.starts_with("env:") {
            if let Some(e) = &self.project_env {
                let mcp: Vec<&str> = e.mcp.iter().map(|m| m.0.as_str()).collect();
                let skills: Vec<&str> = e.skills.iter().map(|k| k.0.as_str()).collect();
                return format!(
                    "The environment block shows the folder {}, MCP servers: {}, skills: {}{}.",
                    e.dir,
                    if mcp.is_empty() { "none".to_string() } else { mcp.join(", ") },
                    if skills.is_empty() { "none".to_string() } else { skills.join(", ") },
                    if e.dirty { "; the soul has unsaved edits in the block" } else { "" }
                );
            }
        }
        // A site: the page the latest step is on.
        if Self::subject_browser(&subject).is_some() {
            if let Some(b) = &self.sites.browser {
                let st = b.state.lock().unwrap();
                if let Some(page) = st.active.as_ref().and_then(|id| st.pages.iter().find(|p| &p.id == id)) {
                    return format!("The site's live tab is at {} (title: {}).", page.url, page.title);
                }
            }
        }
        // A terminal "!" opened in this thread.
        match self.thread_terminal() {
            Some(t) => format!("The terminal opened in this thread shows (its last lines):\n```\n{}\n```", self.ssh.tabs[t].session.screen_text(60)),
            None => String::new(),
        }
    }

    /// The server a thread is about (`ssh:<alias>`).
    pub fn subject_server(subject: &str) -> Option<String> {
        subject.strip_prefix("ssh:").map(str::to_string)
    }

    /// The tool a thread is about (`tool:<name>`).
    pub fn subject_tool(subject: &str) -> Option<Tool> {
        let name = subject.strip_prefix("tool:")?;
        GROUPS
            .iter()
            .flat_map(|g| g.tools.iter().copied())
            .find(|t| tool_key(*t) == name)
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
