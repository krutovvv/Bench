//! The person's inserts: tools put into a thread with "/" (the terminal, the task lists, the
//! utilities, the browser's tab). A thread is a document of messages and inserts;
//! an insert is the person's, sits where it was put, and stays live. The agent keeps them (a Pi
//! entry where each was put, a document of their state: agent/src/inserts.ts); Bench shows its
//! list, changes it through the agent, and takes the agent's answer as the truth. Each kind's
//! look and behaviour: `crate::inserts`.

use gpui_kit::*;
use serde_json::{Map, Value, json};

use crate::app::BenchApp;
use crate::app::model::Insert;
use crate::client::rpc::Cmd;
use crate::inserts::kind_of;
use crate::thread::{ThreadRef, ThreadView};

impl ThreadView {
    /// "/" put `slash_key` into this thread. The app puts it in once this view is done drawing or
    /// handling the key, since it comes back to this view (`new_insert`).
    pub fn put_insert(&mut self, slash_key: String, window: &mut Window, cx: &mut Context<Self>) {
        let (app, view) = (self.app.clone(), cx.entity());
        window.defer(cx, move |window, cx| {
            app.update(cx, |app, cx| app.put_insert(&view, slash_key, window, cx))
                .ok();
        });
    }

    /// The insert `slash_key` makes here: after the last message, or — a message waiting in the
    /// composer — after that one, whose place `send` fixes as it sends. It is where the person
    /// looks now.
    pub fn new_insert(&mut self, slash_key: &str, cx: &mut Context<Self>) -> Option<Insert> {
        let (key, state) = BenchApp::insert_spec(slash_key);
        kind_of(&key)?;
        let draft = !self.composer.read(cx).value().trim().is_empty();
        let ins = Insert {
            id: format!("i{}", &uuid::Uuid::new_v4().simple().to_string()[..10]),
            key,
            state,
            folded: false,
            after: self.thread.journal.messages() + usize::from(draft),
        };
        if draft {
            self.insert_draft.push(ins.id.clone());
        }
        self.follow = true;
        cx.notify();
        Some(ins)
    }

    /// The inserts that waited with a message in the composer now sit after it: the thread's
    /// document remembers the place, since Pi's entry stays where it was written.
    pub fn anchor_draft_inserts(&mut self, after: usize, cx: &mut Context<Self>) {
        let ids = std::mem::take(&mut self.insert_draft);
        if ids.is_empty() {
            return;
        }
        let at = self.thread_ref();
        self.app
            .update(cx, |app, cx| {
                for id in ids {
                    app.patch_insert(&at, &id, json!({ "after": after }), cx);
                }
            })
            .ok();
    }
}

/// A new chat's inserts until its session exists.
const DRAFT: &str = "draft";

impl BenchApp {
    /// A thread's inserts, in order.
    pub fn inserts_of(&self, at: &ThreadRef) -> Vec<Insert> {
        self.inserts.get(&at.key()).cloned().unwrap_or_default()
    }

    /// What a "/" row puts in: "tasks:<list>", "tools:<tool>"; the rest as they are
    /// ("ssh:local", "browser"…).
    pub fn insert_spec(slash_key: &str) -> (String, Map<String, Value>) {
        let mut state = Map::new();
        let (kind, value) = slash_key.split_once(':').unwrap_or((slash_key, ""));
        let key = match (kind, value) {
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

    /// Put an insert into a thread (its view: the open thread's, or the one beside), after its
    /// last message: every "/" puts a new one (ten terminals if asked). A new chat becomes a
    /// thread at once: its session is created now, not with the first message. Not called while
    /// that view draws or handles a key: `ThreadView::put_insert` defers it.
    pub fn put_insert(
        &mut self,
        view: &Entity<ThreadView>,
        slash_key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // An insert shows once the agent has written it into the thread: with no agent yet (just
        // started, or reconnecting) it would wait unseen.
        if self.profile.is_none() {
            self.toast(
                gpui_kit::component::notification::NotificationType::Info,
                "The agent is not connected yet",
                window,
                cx,
            );
            return;
        }
        let Some(ins) = view.update(cx, |v, cx| v.new_insert(&slash_key, cx)) else {
            return;
        };
        let at = view.read(cx).thread_ref();
        let Some(kind) = kind_of(&ins.key) else {
            return;
        };
        self.inserts.entry(at.key()).or_default().push(ins.clone());
        match at.session.clone() {
            Some(session) => self.send_insert(&at, &session, &ins),
            None => self.create_thread_now(&at),
        }
        // The newest of a shared kind is the live one.
        if let Some(name) = kind.shared() {
            self.insert_live.insert(name.to_string(), ins.id.clone());
        }
        kind.opened(self, &at, &ins, window, cx);
        cx.notify();
    }

    fn send_insert(&self, at: &ThreadRef, session: &str, ins: &Insert) {
        self.cmd_at(
            at,
            Cmd::Insert {
                session: session.to_string(),
                id: ins.id.clone(),
                key: ins.key.clone(),
                state: Value::Object(ins.state.clone()),
            },
        );
    }

    /// A new chat's session, made before any message (an insert was put into it, or a
    /// project's "+": the thread shows in the sidebar at once).
    pub(crate) fn create_thread_now(&mut self, at: &ThreadRef) {
        if self.creating || at.runtime.is_some() {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.creating = true;
        self.cmd_at(
            at,
            self.create_cmd(profile, self.new_project.clone(), &at.project),
        );
    }

    /// The new chat's session exists: its inserts go to the agent.
    pub fn inserts_created(&mut self, at: &ThreadRef, session: &str) {
        self.creating = false;
        if let Some(list) = self.inserts.remove(DRAFT) {
            for ins in &list {
                self.send_insert(at, session, ins);
            }
            self.inserts.insert(session.to_string(), list);
        }
    }

    /// Change an insert (its key, state, folded, removed): at once on screen, then the agent's
    /// answer replaces the list.
    pub fn patch_insert(&mut self, at: &ThreadRef, id: &str, patch: Value, cx: &mut Context<Self>) {
        if let Some(list) = self.inserts.get_mut(&at.key()) {
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
                if let Some(a) = patch["after"].as_u64() {
                    ins.after = a as usize;
                }
            }
        }
        // A draft's inserts reach the agent when its session does (`inserts_created`).
        if let Some(session) = at.session.clone() {
            self.cmd_at(
                at,
                Cmd::PatchInsert {
                    session,
                    id: id.to_string(),
                    patch,
                },
            );
        }
        cx.notify();
    }

    pub fn remove_insert(&mut self, at: &ThreadRef, id: &str, cx: &mut Context<Self>) {
        let ins = self.inserts_of(at).into_iter().find(|i| i.id == id);
        if let Some(ins) = ins {
            if let Some(kind) = kind_of(&ins.key) {
                kind.removed(self, at, &ins, cx);
            }
        }
        self.patch_insert(at, id, json!({ "removed": true }), cx);
    }

    /// The agent's list for a thread: the truth.
    pub fn on_inserts(&mut self, session: String, list: Vec<Insert>, cx: &mut Context<Self>) {
        self.inserts.insert(session, list);
        cx.notify();
    }

    /// What every insert of a thread shows now, for the agent (sent with each message, never
    /// shown): the terminal's last lines, the tool's input and output.
    pub fn inserts_context(&self, at: &ThreadRef, cx: &App) -> String {
        self.inserts_of(at)
            .iter()
            .filter_map(|ins| kind_of(&ins.key).map(|k| k.context(self, at, ins, cx)))
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Whether a thread shows the browser (unfolded): its frames flow only then.
    pub fn thread_has_browser(&self, at: &ThreadRef) -> bool {
        self.inserts_of(at)
            .iter()
            .any(|i| i.key == "browser" && !i.folded)
    }
}
