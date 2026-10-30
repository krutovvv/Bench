//! Threads: open one, start a new one, delete one, search them, and where the app was left.

use std::time::Duration;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use protocol::ThreadId;

use crate::client::connection;
use crate::client::rest;

use crate::app::BenchApp;
use crate::workspace::Center;

impl BenchApp {
    /// The center's view back to a new chat (another thread is opened in it next, or none).
    pub(crate) fn reset_chat(&mut self, cx: &mut Context<Self>) {
        self.inserts.by_thread.remove("draft");
        self.threads.creating = false;
        // A new chat starts on the agent's default level.
        let effort = self.defaults.effort;
        self.active_thread.update(cx, |v, cx| v.reset(effort, cx));
        self.media.playing = None;
        self.media.audio = None;
        self.stop_video(None, cx);
        self.new_project.folder = None;
        cx.notify();
    }

    /// Open the thread `id` of the agent `host` in the center.
    pub fn open_thread(&mut self, host: String, id: ThreadId, cx: &mut Context<Self>) {
        self.center = Center::Chat;
        // Asking again for the thread already on screen (a retry): keep what the person has —
        // the turns they are reading, the draft, the messages waiting in the queue — and only
        // fetch the history afresh.
        let at = self.thread_ref(cx);
        if at.host == host && at.id.as_ref() == Some(&id) {
            self.fetch_thread(at, cx);
            cx.notify();
            return;
        }
        self.armed.clear();
        self.new_project.form = None;
        self.unbeside(&host, &id, cx);
        let prev = self.active_thread.update(cx, |v, _| {
            let journal = std::mem::take(&mut v.thread.journal);
            v.thread.id.clone().map(|s| (s, journal))
        });
        if let Some((prev, journal)) = prev {
            self.threads.cache.insert(prev.to_string(), journal);
        }
        self.reset_chat(cx);
        // A thread seen before shows its last state now, and catches up when its journal comes.
        let journal = self.threads.cache.remove(id.as_str());
        let project = self.project_of_thread(&host, &id);
        self.active_thread.update(cx, |v, cx| {
            v.thread.host = host.clone();
            v.thread.id = Some(id.clone());
            v.thread.journal = journal.unwrap_or_default();
            v.thread.project = project;
            cx.notify();
        });
        self.save_place(cx);
        let at = self.thread_ref(cx);
        self.fetch_thread(at, cx);
        cx.notify();
    }

    /// A thread the sidebar lists, opened: its row says which agent keeps it.
    pub fn open_listed(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        let host = self.host_of_thread(&id);
        self.open_thread(host, id, cx);
    }

    /// A thread the sidebar lists, deleted by the agent its row names.
    pub fn delete_listed(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        let host = self.host_of_thread(&id);
        self.delete_thread(host, id, cx);
    }

    /// Read a thread from its agent (`thread.open`): every view showing it takes it; a view
    /// that has it already only catches up from its head.
    pub(crate) fn fetch_thread(&mut self, at: crate::thread::ThreadRef, cx: &mut Context<Self>) {
        let Some(id) = at.id.clone() else {
            return;
        };
        let since = self
            .views_of(&at.host, &id, cx)
            .first()
            .map_or(0, |v| v.read(cx).thread.journal.head());
        let host = at.host.clone();
        self.request(
            &host.clone(),
            protocol::OpenThread {
                thread: id.clone(),
                since,
            },
            cx,
            move |this, r, window, cx| {
                let views = this.views_of(&host, &id, cx);
                match r {
                    Ok(open) => {
                        for view in &views {
                            view.update(cx, |v, cx| {
                                v.thread.on_history(&id, open.clone());
                                cx.notify();
                            });
                        }
                        if let Some(view) = views.first() {
                            this.inserts_from(view, cx);
                        }
                        if views.iter().any(|v| *v == this.active_thread) {
                            this.resolve_repo(window, cx);
                        }
                    }
                    // Its history could not be read: said where it would show.
                    Err(e) => {
                        for view in &views {
                            view.update(cx, |v, cx| {
                                v.thread.history_error = Some(format!("{e:#}"));
                                cx.notify();
                            });
                        }
                    }
                }
            },
        );
    }

    pub fn new_thread(&mut self, cx: &mut Context<Self>) {
        self.center = Center::Chat;
        self.reset_chat(cx);
        self.save_place(cx);
        cx.notify();
    }

    /// A new thread where the open one is shown: a document in the middle, the thread in the
    /// right dock — the panel's "+" and ⌘N reset that thread to a new one and the document
    /// stays. `new_thread` is the same when no thread sits beside a document: the new one
    /// becomes the center's.
    pub fn new_thread_in_panel(&mut self, cx: &mut Context<Self>) {
        self.reset_chat(cx);
        self.save_place(cx);
        cx.notify();
    }

    /// ⌘N and the panel's "+": the new thread appears where a thread lives — beside a
    /// document, in the dock; otherwise the center shows it.
    pub fn new_thread_here(&mut self, cx: &mut Context<Self>) {
        use crate::workspace::{DockPosition, Panel};
        if self.center == Center::Doc
            && self.shown_panel(DockPosition::Right) == Some(Panel::Thread)
        {
            self.new_thread_in_panel(cx);
        } else {
            self.new_thread(cx);
        }
    }

    /// The project a thread belongs to, as the sidebar lists it ("" = its agent's Host).
    pub(crate) fn project_of_thread(&self, host: &str, id: &ThreadId) -> String {
        self.threads.list
            .iter()
            .find(|s| s.host == host && &s.id == id)
            .map(|s| s.project.clone())
            .unwrap_or_default()
    }

    /// Deleting a thread asks once: its row shows Delete · Cancel (⌫ again, or Delete, deletes).
    pub fn arm_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.armed.arm(crate::app::armed::thread_key(&id));
        cx.notify();
    }

    /// The thread goes (after `arm_delete`, or ⌘⌫), by the agent that keeps it.
    pub fn delete_thread(&mut self, host: String, id: ThreadId, cx: &mut Context<Self>) {
        self.armed.clear();
        let at = self.thread_ref(cx);
        if at.host == host && at.id.as_ref() == Some(&id) {
            self.reset_chat(cx);
        }
        self.threads.list.retain(|s| !(s.host == host && s.id == id));
        self.request(
            &host.clone(),
            protocol::DeleteThread { thread: id },
            cx,
            move |this, r, window, cx| {
                if let Err(e) = r {
                    this.toast(
                        NotificationType::Error,
                        format!("Could not delete: {e:#}"),
                        window,
                        cx,
                    );
                    // The agent still has it: its list says so.
                    this.list_threads(&host, cx);
                }
            },
        );
        cx.notify();
    }

    pub fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.armed.clear();
        cx.notify();
    }

    /// Debounced: only the last query typed within 300 ms goes to the agent.
    pub(crate) fn search_threads(
        &mut self,
        q: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if q == self.sidebar.query {
            return;
        }
        self.sidebar.seq += 1;
        let seq = self.sidebar.seq;
        self.sidebar.query = q.clone();
        self.sidebar.hits = None;
        cx.notify();
        if q.is_empty() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.sidebar.seq != seq {
                    return;
                }
                this.rest(
                    window,
                    cx,
                    move |conn| rest::search_threads(conn, &q),
                    move |this, r, _, cx| {
                        if this.sidebar.seq == seq {
                            this.sidebar.hits = Some(r.unwrap_or_default());
                            cx.notify();
                        }
                    },
                );
            });
        })
        .detach();
    }

    /// The thread the app was last closed on: its agent and its id.
    pub(crate) fn last_place() -> Option<(String, ThreadId)> {
        std::fs::read(connection::state_dir().join("last-thread.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    /// Remember the open thread (called whenever it changes).
    pub(crate) fn save_place(&self, cx: &App) {
        let at = self.thread_ref(cx);
        let place = at.id.map(|id| (at.host, id));
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(
            connection::state_dir().join("last-thread.json"),
            serde_json::to_vec(&place).unwrap_or_default(),
        );
    }
}
