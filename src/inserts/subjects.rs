//! What a thread is opened with: the `/` list that puts inserts into it (`app::inserts`). The
//! terminal, a tool or the tasks opened from the keyboard are a new thread with that insert in it.

use gpui_kit::*;

use crate::app::BenchApp;
use crate::thread::{ThreadRef, ThreadView};
use devtools::{GROUPS, Tool};

impl BenchApp {
    /// A thread with `key` in it, as the demo starts one: a new thread with that insert.
    pub fn open_subject_key(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        self.new_thread_with(key, window, cx);
    }

    /// A new thread with one insert in it (its session is made at once).
    pub fn new_thread_with(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        self.center = crate::workspace::Center::Chat;
        self.new_session(cx);
        let view = self.active_thread.clone();
        self.put_insert(&view, key, window, cx);
    }

    /// `/terminal`, `/browser`, `/tasks`, `/tools` and what follows: the tools that can be
    /// attached to this thread, filtered by the rest of the line. None when the line is not one.
    pub fn tool_slash(
        &self,
        at: &ThreadRef,
        text: &str,
    ) -> Option<Vec<crate::app::model::SlashCommand>> {
        let rest = text.strip_prefix('/')?;
        let (word, q) = match rest.split_once(char::is_whitespace) {
            Some((w, q)) => (w, q.trim().to_lowercase()),
            None => return None,
        };
        let row = |name: String, desc: String, key: String| crate::app::model::SlashCommand {
            name,
            desc,
            category: "Tool".into(),
            key,
        };
        let mut rows: Vec<crate::app::model::SlashCommand> = match word.to_lowercase().as_str() {
            "terminal" => {
                let host = self.thread_host(at);
                let desc = if host.is_empty() {
                    "this Mac's terminal".to_string()
                } else {
                    format!("the terminal on {host}, where this project runs")
                };
                vec![row("/terminal".into(), desc, "ssh:local".into())]
            }
            "browser" => vec![row(
                "/browser".into(),
                "the server's browser: its tab, live".into(),
                "browser".into(),
            )],
            "tasks" => crate::inserts::tasks::model::LISTS
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
    pub fn tool_words() -> Vec<crate::app::model::SlashCommand> {
        crate::inserts::words()
    }

    /// This machine's terminal (the thread's: its project's machine) in a new thread.
    pub fn open_terminal_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_thread_with("ssh:local".into(), window, cx);
    }

    /// A terminal insert's own shell on `alias`, started if there is none or it ended.
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

    /// "!command": typed into the thread's terminal (its last terminal insert); a thread with
    /// none gets this Mac's, in the thread's folder. Not called while `view` draws or handles a
    /// key: `ThreadView::run_bang` defers it.
    pub fn run_bang(
        &mut self,
        view: &Entity<ThreadView>,
        command: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = view.read(cx).thread_ref();
        let last = self
            .inserts_of(&at)
            .into_iter()
            .rev()
            .find(|i| i.key == "ssh:local");
        let (ins, fresh) = match last {
            Some(i) => (i, false),
            None => {
                self.put_insert(view, "ssh:local".into(), window, cx);
                let at = view.read(cx).thread_ref();
                match self
                    .inserts_of(&at)
                    .into_iter()
                    .rev()
                    .find(|i| i.key == "ssh:local")
                {
                    Some(i) => (i, true),
                    None => return,
                }
            }
        };
        let alias = self.thread_machine(&at);
        if let Some(t) = self.insert_terminal(&ins.id, &alias, window, cx) {
            // Start where the thread works.
            if fresh {
                if let Some(dir) = self.folder_of_thread(&at) {
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

    /// A thread's folder, when it works in one.
    fn folder_of_thread(&self, at: &ThreadRef) -> Option<String> {
        at.session
            .as_ref()
            .and_then(|id| self.folder_of(id))
            .or_else(|| self.new_project.clone())
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

pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

impl ThreadView {
    /// "!command" from this thread's composer: the app types it into the thread's terminal once
    /// this view is done handling the key (`BenchApp::run_bang`).
    pub fn run_bang(&mut self, command: String, window: &mut Window, cx: &mut Context<Self>) {
        let (app, view) = (self.app.clone(), cx.entity());
        window.defer(cx, move |window, cx| {
            app.update(cx, |app, cx| app.run_bang(&view, command, window, cx))
                .ok();
        });
    }
}
