//! The person's inserts: tools put into a thread with "/" (the terminal, the
//! utilities, the browser's tab). A thread is a document of messages and inserts; an insert is
//! the person's, sits where it was put, and stays live. The agent keeps them (a Pi entry where
//! each was put, a document of their state: agent/src/runtime/inserts.ts); Bench changes them through
//! the agent and reads them from the thread's journal — the app keeps that list for the inserts'
//! kinds (`inserts_from`). Each kind's look and behaviour: `crate::inserts`.

use gpui_kit::*;
use protocol::InsertPatch;
use serde_json::{Map, Value};

use crate::app::BenchApp;
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
                    let patch = InsertPatch {
                        after: Some(after as u64),
                        ..Default::default()
                    };
                    app.patch_insert(&at, &id, patch, cx);
                }
            })
            .ok();
    }
}

/// A change of an insert's own state alone (its list, its tool, its tab).
pub fn new_state(state: Value) -> InsertPatch {
    InsertPatch {
        state: state.as_object().cloned(),
        ..Default::default()
    }
}

/// A new chat's inserts until it is a thread.
const DRAFT: &str = "draft";

impl BenchApp {
    /// A thread's inserts, in order.
    pub fn inserts_of(&self, at: &ThreadRef) -> Vec<Insert> {
        self.inserts.by_thread.get(&at.key()).cloned().unwrap_or_default()
    }

    /// What a "/" row puts in: its kind's key and first state (`InsertKind::spec`); a key no
    /// kind knows as it is.
    pub fn insert_spec(slash_key: &str) -> (String, Map<String, Value>) {
        crate::inserts::kinds()
            .iter()
            .find_map(|k| k.spec(slash_key))
            .unwrap_or_else(|| (slash_key.to_string(), Map::new()))
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
        let at = view.read(cx).thread_ref();
        if self.conn_of(&self.thread_host(&at)).is_none() {
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
        self.inserts.by_thread.entry(at.key()).or_default().push(ins.clone());
        match at.id.is_some() {
            true => self.send_insert(&at, &ins, cx),
            false => self.create_thread(&at, cx),
        }
        // The newest of a shared kind is the live one.
        if let Some(name) = kind.shared() {
            self.inserts.live.insert(name.to_string(), ins.id.clone());
        }
        kind.opened(self, &at, &ins, window, cx);
        cx.notify();
    }

    fn send_insert(&self, at: &ThreadRef, ins: &Insert, cx: &mut Context<Self>) {
        let Some(thread) = at.id.clone() else {
            return;
        };
        let put = protocol::PutInsert {
            thread,
            id: ins.id.clone(),
            key: ins.key.clone(),
            state: ins.state.clone(),
        };
        let at = at.clone();
        self.request(&self.thread_host(&at), put, cx, move |this, r, window, cx| {
            if let Err(e) = r {
                this.insert_refused(&at, e, window, cx);
            }
        });
    }

    /// The agent refused an insert's change: said, and the thread is read again — its journal
    /// is the truth.
    fn insert_refused(
        &mut self,
        at: &ThreadRef,
        e: anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toast(
            gpui_kit::component::notification::NotificationType::Error,
            format!("The insert didn't change: {e:#}"),
            window,
            cx,
        );
        self.fetch_thread(at.clone(), cx);
    }

    /// The new chat is a thread now: its inserts go to its agent.
    pub fn inserts_created(&mut self, at: &ThreadRef, cx: &mut Context<Self>) {
        if let Some(list) = self.inserts.by_thread.remove(DRAFT) {
            for ins in &list {
                self.send_insert(at, ins, cx);
            }
            self.inserts.by_thread.insert(at.key(), list);
        }
    }

    /// The thread's inserts as its journal has them now (the view showing it read them), for
    /// the inserts' kinds.
    pub fn inserts_from(&mut self, view: &Entity<ThreadView>, cx: &mut Context<Self>) {
        let v = view.read(cx);
        let key = v.thread_ref().key();
        let list = v
            .thread
            .journal
            .insert_list()
            .into_iter()
            .map(|(id, s)| Insert::new(id, s))
            .collect();
        self.inserts.by_thread.insert(key, list);
    }

    /// Change an insert (its key, state, folded, removed, place): at once on screen; the thread's
    /// `inserts` event brings the agent's version.
    pub fn patch_insert(
        &mut self,
        at: &ThreadRef,
        id: &str,
        patch: InsertPatch,
        cx: &mut Context<Self>,
    ) {
        if let Some(list) = self.inserts.by_thread.get_mut(&at.key()) {
            if patch.removed == Some(true) {
                list.retain(|i| i.id != id);
            } else if let Some(ins) = list.iter_mut().find(|i| i.id == id) {
                if let Some(k) = &patch.key {
                    ins.key = k.clone();
                }
                if let Some(s) = &patch.state {
                    ins.state = s.clone();
                }
                if let Some(f) = patch.folded {
                    ins.folded = f;
                }
            }
        }
        // A new chat's inserts reach the agent when it is made (`inserts_created`).
        if let Some(thread) = at.id.clone() {
            let change = protocol::PatchInsert {
                thread,
                id: id.to_string(),
                patch,
            };
            let at = at.clone();
            self.request(&self.thread_host(&at), change, cx, move |this, r, window, cx| {
                if let Err(e) = r {
                    this.insert_refused(&at, e, window, cx);
                }
            });
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
        let patch = InsertPatch {
            removed: Some(true),
            ..Default::default()
        };
        self.patch_insert(at, id, patch, cx);
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
}

/// What the window keeps of the inserts: each thread's list as its journal has them, the live one
/// of the kinds that are live once, and each kind's own state (the terminals' shells, the
/// utilities, the server browser's tabs).
pub struct InsertsState {
    /// By `ThreadRef::key` ("draft" for a new chat).
    pub by_thread: std::collections::HashMap<String, Vec<Insert>>,
    /// Of the kinds whose state is app-wide (tools), the live insert per kind.
    pub live: std::collections::HashMap<String, String>,
    /// /terminal: the inserts' shells.
    pub terminal: crate::inserts::terminal::state::Terminals,
    /// The bottom dock's terminals (`terminal/panel.rs`).
    pub dock_terms: crate::inserts::terminal::panel::DockTerms,
    /// /tools: the utilities of `devtools`.
    pub tools: crate::inserts::tools::state::Tools,
    /// /browser: each thread's tab on the server's browser, live or as its last frame.
    pub browser: crate::inserts::browser::state::BrowserState,
}

impl InsertsState {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        Self {
            by_thread: Default::default(),
            live: Default::default(),
            terminal: Default::default(),
            dock_terms: Default::default(),
            tools: crate::inserts::tools::state::Tools::new(window, cx),
            browser: Default::default(),
        }
    }
}

/// A piece the person put into a thread with "/" (the terminal, the utilities, the
/// browser's tab): kept by the agent as a Pi entry where it was put and a document of its state
/// (agent/src/runtime/inserts.ts), read from the thread's journal (`Thread::insert_list`). Its look and
/// behaviour: `crate::inserts`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Insert {
    pub id: String,
    /// What it is: "ssh:local" (the terminal on the thread's machine), "tools",
    /// "browser".
    pub key: String,
    /// Its own state (the list, the tool, the browser's tab).
    pub state: serde_json::Map<String, Value>,
    pub folded: bool,
}

impl Insert {
    pub fn new(id: String, s: protocol::InsertState) -> Self {
        Self {
            id,
            key: s.key,
            state: s.state,
            folded: s.folded,
        }
    }

    /// A string field of its state.
    pub fn get(&self, field: &str) -> String {
        self.state
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
}
