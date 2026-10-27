//! Threads: open, new, delete, search, the profile, and where the app was left.

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::IndexPath;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::connection;
use crate::client::rest;
use crate::client::rpc::Cmd;

use crate::app::{BenchApp, now_secs};
use crate::workspace::Center;

impl BenchApp {
    pub(crate) fn reset_chat(&mut self, cx: &mut Context<Self>) {
        self.active_thread.processes.clear();
        self.active_thread.process_open = None;
        self.active_thread.session = None;
        self.active_thread.runtime = None;
        self.inserts.remove("draft");
        self.creating = false;
        self.queue_edit = None;
        // A new chat starts on the profile's level.
        self.active_thread.effort_other = None;
        self.active_thread.effort = self.profile_effort;
        self.active_thread.journal = Default::default();
        self.active_thread.texts.clear();
        self.active_thread.history_error = None;
        self.active_thread.live = None;
        self.active_thread.queued = None;
        self.active_thread.usage = None;
        self.active_thread.fallback_on = None;
        self.playing = None;
        self.media_playing = None;
        self.stop_video(None, cx);
        self.active_thread.approval = None;
        self.active_thread.clarify = None;
        self.active_thread.todos.clear();
        // Speeds describe the model and server, not the chat: keep them, restart the counters.
        self.speed.restart();
        self.active_thread.project = None;
        self.new_project = None;
        self.reset_scroller(cx);
    }

    pub(crate) fn reset_scroller(&mut self, cx: &mut Context<Self>) {
        self.open_steps.clear();
        self.active_thread.open_work.clear();
        self.open_source = None;
        cx.notify();
    }

    pub fn select_profile(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let idx = self.profiles.iter().position(|p| p.name == name);
        self.profile_choice.update(cx, |s, cx| {
            s.set_selected_index(idx.map(IndexPath::new), window, cx)
        });
        self.profile = Some(name.clone());
        self.load_profile_effort(window, cx);
        if let Some(profile) = self.profile.clone() {
            self.catalog.clear();
            self.cmd(Cmd::Catalog { profile });
        }
        self.prompts.clear();
        self.prompt_open = None;
        self.load_prompts(window, cx);
        self.sessions.clear();
        self.tg_owner.clear();
        self.tg_people.clear();
        self.tg_visible = None;
        self.skills.clear();
        self.skill_open = None;
        self.reset_chat(cx);
        // Keep the session to resume across this reset (startup picks the profile first).
        if self.resume_session.is_none() {
            self.save_place();
        }
        self.cmd(Cmd::Sessions(name));
        if self.center != Center::Chat {
            let panel = self.center;
            self.open_center(panel, window, cx);
        }
        cx.notify();
    }

    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.center = Center::Chat;
        // Asking again for the thread already on screen (its inserts fell out of step, a retry):
        // keep what the person has — the turns they are reading, the draft, the messages waiting
        // in the queue — and only fetch the history afresh. Resetting it first was felt as the
        // thread changing under them: the queue and their own unwritten message vanished with it.
        if self.active_thread.session.as_deref() == Some(id.as_str()) {
            self.cmd(Cmd::Open {
                profile,
                session: id,
            });
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        self.project_form = None;
        if let Some(prev) = self.active_thread.session.clone() {
            self.thread_cache
                .insert(prev, std::mem::take(&mut self.active_thread.journal));
        }
        self.reset_chat(cx);
        self.active_thread.session = Some(id.clone());
        // A thread seen before shows its last state now, and catches up when its journal comes.
        if let Some(t) = self.thread_cache.remove(&id) {
            self.active_thread.journal = t;
        }
        self.active_thread.thread_project = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.project.clone())
            .unwrap_or_default();
        if self.demo {
            self.active_thread.journal = self.demo_threads.get(&id).cloned().unwrap_or_default();
            self.active_thread.processes = if id == "d-landing" {
                crate::demo::processes(now_secs())
            } else {
                Vec::new()
            };
            cx.notify();
            return;
        }
        self.save_place();
        self.cmd(Cmd::Open {
            profile,
            session: id,
        });
        cx.notify();
    }

    pub fn new_session(&mut self, cx: &mut Context<Self>) {
        self.center = Center::Chat;
        self.reset_chat(cx);
        self.save_place();
        cx.notify();
    }

    /// Deleting a thread asks once: its row shows Delete · Cancel (⌫ again, or Delete, deletes).
    pub fn arm_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.confirm_delete = Some(id);
        cx.notify();
    }

    /// The thread goes (after `arm_delete`, or ⌘⌫).
    pub fn delete_session(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        let Some(profile) = self.profile.clone() else {
            return;
        };
        // Whose thread it is, read before it leaves the list: a server's thread is its agent's to
        // delete (asked after, it looked like this Mac's — this Mac's agent said done, and the
        // server's list brought it back).
        let host = self.host_of_session(&id);
        if !host.is_empty() && self.conn_of(&host).is_none() {
            self.toast(
                NotificationType::Error,
                format!("Could not delete: {host}'s agent is not connected"),
                window,
                cx,
            );
            return;
        }
        if self.active_thread.session.as_deref() == Some(&id) {
            self.reset_chat(cx);
        }
        self.sessions.retain(|s| s.id != id);
        let runtime = self.opened.remove(&id);
        let rid = id.clone();
        let remote = (!host.is_empty()).then(|| self.conn_of(&host)).flatten();
        if let Some(conn) = remote {
            let p = profile.clone();
            self.background(
                window,
                cx,
                move || rest::delete_session(&conn, &p, &rid),
                move |this, r, window, cx| match r {
                    Ok(()) => this.refresh_sessions(),
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("Could not delete: {e:#}"),
                        window,
                        cx,
                    ),
                },
            );
            return;
        }
        self.rest(
            window,
            cx,
            move |c, p| rest::delete_session(c, p, &rid),
            move |this, r, window, cx| match r {
                Ok(()) => this.refresh_sessions(),
                // Older gateways without the REST route: the JSON-RPC delete.
                Err(e) if e.to_string().starts_with("404") || e.to_string().starts_with("405") => {
                    this.cmd(Cmd::Delete {
                        profile,
                        session: id,
                        runtime,
                    })
                }
                Err(e) => {
                    this.toast(
                        NotificationType::Error,
                        format!("Could not delete: {e:#}"),
                        window,
                        cx,
                    );
                    this.refresh_sessions();
                }
            },
        );
        cx.notify();
    }

    pub fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        cx.notify();
    }

    /// Debounced: only the last query typed within 300 ms goes to the gateway.
    pub(crate) fn search_sessions(
        &mut self,
        q: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if q == self.search_query {
            return;
        }
        self.search_seq += 1;
        let seq = self.search_seq;
        self.search_query = q.clone();
        self.search_hits = None;
        cx.notify();
        if q.is_empty() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.search_seq != seq {
                    return;
                }
                this.rest(
                    window,
                    cx,
                    move |conn, profile| rest::search_sessions(conn, profile, &q),
                    move |this, r, _, cx| {
                        if this.search_seq == seq {
                            this.search_hits = Some(r.unwrap_or_default());
                            cx.notify();
                        }
                    },
                );
            });
        })
        .detach();
    }

    /// Where this target was last: (profile, open session).
    pub(crate) fn last_place(&self) -> (Option<String>, Option<String>) {
        let all: HashMap<String, (Option<String>, Option<String>)> =
            std::fs::read(connection::state_dir().join("last-place.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
        all.get(&self.target.label()).cloned().unwrap_or_default()
    }

    /// Remember the profile and session for this target (called whenever either changes).
    pub(crate) fn save_place(&self) {
        if self.demo {
            return;
        }
        let path = connection::state_dir().join("last-place.json");
        let mut all: HashMap<String, (Option<String>, Option<String>)> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        all.insert(
            self.target.label(),
            (self.profile.clone(), self.active_thread.session.clone()),
        );
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path, serde_json::to_vec(&all).unwrap_or_default());
    }
}
