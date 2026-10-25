//! The person's inserts: tools put into a thread with "/" (a server's terminal, a file host, the
//! task lists, the utilities, the browser's tab). A thread is a document of messages and inserts;
//! an insert is the person's, sits where it was put, and stays live. The agent keeps them (a Pi
//! entry where each was put, a document of their state: agent/src/inserts.ts); Bench shows its
//! list, changes it through the agent, and takes the agent's answer as the truth. Each kind's
//! look and behaviour: `crate::ui::inserts`.

use gpui_kit::*;
use serde_json::{Map, Value, json};

use super::BenchApp;
use crate::gateway::Cmd;
use crate::model::Insert;
use crate::ui::inserts::kind_of;

/// A new chat's inserts until its session exists.
const DRAFT: &str = "draft";

impl BenchApp {
    /// The open thread's inserts, in order.
    pub fn thread_inserts(&self) -> Vec<Insert> {
        self.inserts.get(&self.thread_key()).cloned().unwrap_or_default()
    }

    /// What a "/" row puts in: "files:<id>" → files on that host, "tasks:<list>", "tools:<tool>";
    /// the rest as they are ("ssh:<alias>", "ssh", "files", "browser"…).
    pub fn insert_spec(slash_key: &str) -> (String, Map<String, Value>) {
        let mut state = Map::new();
        let (kind, value) = slash_key.split_once(':').unwrap_or((slash_key, ""));
        let key = match (kind, value) {
            ("files", v) if !v.is_empty() => {
                state.insert("host".into(), json!(v));
                "files"
            }
            ("tasks", v) if !v.is_empty() => {
                state.insert("list".into(), json!(v));
                "tasks"
            }
            ("tools" | "tool", v) if !v.is_empty() => {
                state.insert("tool".into(), json!(v));
                "tools"
            }
            ("browser", _) => "browser",
            _ => return (slash_key.to_string(), state),
        };
        (key.to_string(), state)
    }

    /// Put an insert into the open thread, after its last message: every "/" puts a new one (ten
    /// terminals if asked). A new chat becomes a thread at once: its session is created now, not
    /// with the first message.
    pub fn put_insert(&mut self, slash_key: String, window: &mut Window, cx: &mut Context<Self>) {
        let (key, state) = Self::insert_spec(&slash_key);
        let Some(kind) = kind_of(&key) else {
            return;
        };
        let thread = self.thread_key();
        let ins = Insert {
            id: format!("i{}", &uuid::Uuid::new_v4().simple().to_string()[..10]),
            key,
            state,
            folded: false,
            after: self.turns.len(),
        };
        self.inserts.entry(thread.clone()).or_default().push(ins.clone());
        match self.session.clone() {
            Some(session) => self.send_insert(&session, &ins),
            None => self.create_thread_now(),
        }
        // The newest of a shared kind is the live one.
        if let Some(name) = kind.shared() {
            self.insert_live.insert(name.to_string(), ins.id.clone());
        }
        kind.opened(self, &ins, window, cx);
        // The insert is where the person looks now.
        self.follow = true;
        cx.notify();
    }

    fn send_insert(&self, session: &str, ins: &Insert) {
        self.cmd(Cmd::Insert {
            session: session.to_string(),
            id: ins.id.clone(),
            key: ins.key.clone(),
            state: Value::Object(ins.state.clone()),
        });
    }

    /// A new chat's session, made before any message (an insert was put into it, or a
    /// project's "+": the thread shows in the sidebar at once).
    pub(crate) fn create_thread_now(&mut self) {
        if self.creating || self.runtime.is_some() {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.creating = true;
        self.cmd(self.create_cmd(profile, self.new_project.clone()));
    }

    /// The new chat's session exists: its inserts go to the agent.
    pub fn inserts_created(&mut self, session: &str) {
        self.creating = false;
        if let Some(list) = self.inserts.remove(DRAFT) {
            for ins in &list {
                self.send_insert(session, ins);
            }
            self.inserts.insert(session.to_string(), list);
        }
    }

    /// Change an insert (its key, state, folded, removed): at once on screen, then the agent's
    /// answer replaces the list.
    pub fn patch_insert(&mut self, id: &str, patch: Value, cx: &mut Context<Self>) {
        let thread = self.thread_key();
        if let Some(list) = self.inserts.get_mut(&thread) {
            if patch["removed"] == json!(true) {
                list.retain(|i| i.id != id);
            } else if let Some(ins) = list.iter_mut().find(|i| i.id == id) {
                if let Some(k) = patch["key"].as_str() {
                    ins.key = k.to_string();
                }
                if let Some(s) = patch["state"].as_object() {
                    ins.state = s.clone();
                }
                if let Some(f) = patch["folded"].as_bool() {
                    ins.folded = f;
                }
            }
        }
        // A draft's inserts reach the agent when its session does (`inserts_created`).
        if let Some(session) = self.session.clone() {
            self.cmd(Cmd::PatchInsert { session, id: id.to_string(), patch });
        }
        cx.notify();
    }

    /// An insert's state as it is on screen now (the folder a file host shows…): sent when it
    /// differs from what the agent has.
    pub fn sync_insert_state(&mut self, id: &str, state: Map<String, Value>, cx: &mut Context<Self>) {
        let same = self
            .inserts
            .get(&self.thread_key())
            .and_then(|v| v.iter().find(|i| i.id == id))
            .is_none_or(|i| i.state == state);
        if !same {
            self.patch_insert(id, json!({ "state": state }), cx);
        }
    }

    pub fn remove_insert(&mut self, id: &str, cx: &mut Context<Self>) {
        let ins = self.thread_inserts().into_iter().find(|i| i.id == id);
        if let Some(ins) = ins {
            if let Some(kind) = kind_of(&ins.key) {
                kind.removed(self, &ins, cx);
            }
        }
        self.patch_insert(id, json!({ "removed": true }), cx);
    }

    /// The agent's list for a thread: the truth.
    pub fn on_inserts(&mut self, session: String, list: Vec<Insert>, cx: &mut Context<Self>) {
        self.inserts.insert(session, list);
        cx.notify();
    }

    /// What every insert of the thread shows now, for the agent (sent with each message).
    pub fn inserts_context(&self, cx: &App) -> String {
        self.thread_inserts()
            .iter()
            .filter_map(|ins| kind_of(&ins.key).map(|k| k.context(self, ins, cx)))
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Whether the open thread shows the browser (unfolded): its frames flow only then.
    pub fn thread_has_browser(&self) -> bool {
        self.thread_inserts().iter().any(|i| i.key == "browser" && !i.folded)
    }
}
