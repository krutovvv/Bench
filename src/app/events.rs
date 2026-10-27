//! Gateway events: what the agent says, applied to the window's state.

use gpui_kit::component::IndexPath;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rpc::{Cmd, Evt};

use super::*;

impl BenchApp {
    pub(crate) fn on_event(&mut self, evt: Evt, window: &mut Window, cx: &mut Context<Self>) {
        // An event of one thread goes to that thread's view; the rest of what it does (the status
        // bar, a notification, a toast) is the open thread's only.
        if let Some(rt) = evt.runtime() {
            let open = self.is_current(rt, cx);
            let view = if open {
                Some(self.active_thread.clone())
            } else {
                self.view_of_runtime(rt, cx)
            };
            let Some(view) = view else {
                return;
            };
            view.update(cx, |v, cx| v.on_event(&evt, window, cx));
            // A finished run may have rewritten files where they live: every copy we cached of
            // that agent's machine is stale now, and the next look (a click, a redraw) brings
            // the current one. An open video keeps playing from its copy; it re-shows after.
            if matches!(evt, Evt::Complete { .. }) {
                let label = self.thread_target(&view.read(cx).thread_ref()).label();
                self.media
                    .retain(|k, _| !k.starts_with(&format!("{label}\0")));
            }
            // A command that expanded into a prompt (skills, /goal, …) runs as a turn of its
            // thread, whichever it is.
            if let Evt::Dispatched {
                session,
                send: Some(message),
                ..
            } = &evt
            {
                if let Some(profile) = self.profile.clone() {
                    let at = view.read(cx).thread_ref();
                    let context = self.inserts_context(&at, cx);
                    self.cmd_at(
                        &at,
                        Cmd::Send {
                            profile,
                            session: session.clone(),
                            text: message.clone(),
                            effort: None,
                            context,
                        },
                    );
                    if open {
                        self.speed.request();
                    }
                }
            }
            if !open {
                if let Evt::Complete { .. } = evt {
                    self.refresh_sessions();
                }
                return;
            }
        }
        match evt {
            Evt::Connected(conn) => {
                if let Some(a) = self.agents.get_mut("") {
                    a.conn = Some(conn);
                    a.error = None;
                }
                self.agent_up(String::new(), window, cx);
                self.connecting = false;
                // Tasks live with the agent: another machine, its own tasks.
                self.restart_tasks(cx);
                self.reload_servers(cx);
                self.load_projects(window, cx);
            }
            Evt::Error(e) => {
                self.connecting = false;
                // A send that failed: no run started, and the message never reached the journal.
                for view in self.thread_views() {
                    view.update(cx, |v, cx| {
                        if v.thread.live.is_some() && !v.thread.journal.running() {
                            v.thread.live = None;
                            v.thread.journal.clear_sent();
                            cx.notify();
                        }
                    });
                }
                self.toast(NotificationType::Error, e, window, cx);
            }
            Evt::Profiles(list) => {
                self.profiles = list;
                let labels: Vec<SharedString> = self
                    .profiles
                    .iter()
                    .map(|p| p.label.clone().into())
                    .collect();
                self.profile_choice
                    .update(cx, |s, cx| s.set_items(labels, window, cx));
                // Back where the app was closed: that profile, then that session.
                let (last_profile, last_session) =
                    self.start_place.take().unwrap_or_else(|| self.last_place());
                let pick = self
                    .profiles
                    .iter()
                    .find(|p| Some(&p.name) == last_profile.as_ref())
                    .or_else(|| self.profiles.iter().find(|p| p.name == "deep-research"))
                    .or(self.profiles.first())
                    .map(|p| p.name.clone());
                self.resume_session = last_session.filter(|_| pick == last_profile);
                if let Some(p) = pick {
                    self.select_profile(p, window, cx);
                }
            }
            Evt::Sessions(p, rows) => {
                // BENCH_SUBJECT=tool:base64 (or tasks:today, ssh:rpi, browser:<id>…): open that
                // thread once the sessions are known, for screenshots of each block.
                let subject_at_start = std::env::var("BENCH_SUBJECT")
                    .ok()
                    .filter(|_| !self.subject_opened);
                if self.profile.as_deref() == Some(&p) {
                    let unknown_tg = rows
                        .iter()
                        .any(|s| s.source == "telegram" && !self.tg_owner.contains_key(&s.id));
                    self.notify_finished(&rows, cx);
                    // The servers' threads stay (they come from their own agents).
                    self.sessions.retain(|s| !s.host.is_empty());
                    self.sessions.extend(rows);
                    if let Some(id) = self.resume_session.take() {
                        if self.thread(cx).session.is_none()
                            && self.sessions.iter().any(|s| s.id == id)
                        {
                            self.open_session(id, cx);
                        }
                    }
                    if unknown_tg {
                        self.load_telegram_owners(window, cx);
                    }
                    self.load_places(window, cx);
                }
                // `BENCH_PANEL=providers`: the gear open once the threads are known (for
                // screenshots; the last thread reopening would close it before).
                if std::env::var("BENCH_PANEL").as_deref() == Ok("providers")
                    && !self.subject_opened
                {
                    self.subject_opened = true;
                    // `BENCH_GEAR_AGENT=<host>`: that agent's settings shown.
                    self.gear_agent = std::env::var("BENCH_GEAR_AGENT").unwrap_or_default();
                    // `BENCH_GEAR_TAB=<0|1|2>`: that tab (agents, presets, look).
                    self.gear_tab = std::env::var("BENCH_GEAR_TAB")
                        .ok()
                        .and_then(|t| t.parse().ok())
                        .unwrap_or(0);
                    self.open_center(Center::Providers, window, cx);
                    // `BENCH_AGENT_FORM=1`: with "+ Agent on a server" open.
                    if std::env::var("BENCH_AGENT_FORM").is_ok() {
                        self.open_agent_form(window, cx);
                    }
                }
                if let Some(key) = subject_at_start {
                    self.subject_opened = true;
                    self.open_subject_key(key, window, cx);
                }
            }
            Evt::SessionsChanged => self.refresh_sessions(),
            Evt::Deleted => self.refresh_sessions(),
            Evt::DeleteFailed(e) => {
                let msg = if e.contains("active") {
                    "The session is open in another client (Telegram) — close it there".to_string()
                } else {
                    format!("Could not delete: {e}")
                };
                self.toast(NotificationType::Error, msg, window, cx);
                self.refresh_sessions();
            }
            Evt::History {
                session,
                runtime,
                open,
            } => {
                self.opened.insert(session.clone(), runtime.clone());
                let open_here = self.active_thread.update(cx, |v, cx| {
                    cx.notify();
                    v.thread.on_history(&session, runtime.clone(), open.clone())
                });
                if open_here {
                    self.resolve_project(window, cx);
                } else if let Some(view) = self.beside_thread.clone() {
                    view.update(cx, |v, cx| {
                        cx.notify();
                        v.thread.on_history(&session, runtime, open)
                    });
                }
            }
            Evt::Effort(_, value) => {
                // The session's own level replaces whatever the bar showed (the profile's default
                // or the previous chat's).
                let at = self.efforts.iter().position(|e| *e == value);
                self.active_thread.update(cx, |v, _| {
                    v.thread.effort_pending = false;
                    match at {
                        Some(i) => {
                            v.thread.effort = i;
                            v.thread.effort_other = None;
                        }
                        None if value.is_empty() => v.thread.effort_other = Some("default".into()),
                        None => v.thread.effort_other = Some(value),
                    }
                });
                if let Some(i) = at {
                    self.effort_choice.update(cx, |s, cx| {
                        s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                    });
                }
            }
            Evt::Created { session, runtime } => {
                // In the list at once with its subject: the first message's block state (and the
                // block itself) depend on it, and the sessions list comes back only later.
                let at = self.thread_ref(cx);
                if !self.sessions.iter().any(|s| s.id == session) {
                    self.sessions.insert(
                        0,
                        crate::app::model::SessionRow {
                            id: session.clone(),
                            title: "Untitled".into(),
                            messages: 0,
                            running: true,
                            source: "desktop".into(),
                            subject: self.new_subject.clone(),
                            project: at.project.clone(),
                            host: self.thread_host(&at),
                        },
                    );
                }
                self.opened.insert(session.clone(), runtime.clone());
                let efforts = self.efforts.clone();
                let (queued, effort, at) = self.active_thread.update(cx, |v, cx| {
                    v.thread.session = Some(session.clone());
                    v.thread.runtime = Some(runtime.clone());
                    let effort = v.thread.take_effort(&efforts);
                    cx.notify();
                    (v.thread.queued.take(), effort, v.thread.thread_ref())
                });
                self.inserts_created(&at, &session);
                self.save_place(cx);
                if let (Some(text), Some(profile)) = (queued, self.profile.clone()) {
                    let cmd = match self.parse_command(&text) {
                        Some((name, arg)) => Cmd::Dispatch {
                            profile,
                            session: runtime,
                            name,
                            arg,
                        },
                        None => Cmd::Send {
                            profile,
                            session: runtime,
                            text,
                            effort,
                            context: self.inserts_context(&at, cx),
                        },
                    };
                    self.cmd_at(&at, cmd);
                }
                // Show the new session in the list right away.
                self.refresh_sessions();
            }
            Evt::Inserts(sid, list) => self.on_inserts(sid, list, cx),
            Evt::ResumeFailed(sid, why) => {
                for view in self.thread_views() {
                    view.update(cx, |v, cx| {
                        if v.thread.session.as_deref() == Some(sid.as_str()) {
                            v.thread.history_error = Some(why.clone());
                            cx.notify();
                        }
                    });
                }
            }
            // Not made: the next insert or message tries again (until now a failed create kept the
            // new thread waiting for a session that never came, until another thread was opened).
            Evt::CreateFailed(e) => {
                self.creating = false;
                self.toast(
                    NotificationType::Error,
                    format!("The thread wasn't created: {e}"),
                    window,
                    cx,
                );
            }
            Evt::InsertFailed(sid, e) => {
                self.toast(
                    NotificationType::Error,
                    format!("The insert didn't change: {e}"),
                    window,
                    cx,
                );
                // The agent's list is the truth: read it again.
                if self.thread(cx).session.as_deref() == Some(&sid) {
                    self.open_session(sid, cx);
                }
            }
            Evt::Delta(_, text) | Evt::Reasoning(_, text) => {
                self.speed.token(text.chars().count());
            }
            Evt::ToolStart { .. } => self.speed.tool_call(),
            Evt::ToolDone { .. } => {
                self.speed.request();
                // The agent may have changed the list this thread is about.
                let subject = self.current_subject(cx);
                if subject.starts_with("tasks:") {
                    self.load_tasks();
                }
                if self.thread(cx).project.is_none() {
                    self.resolve_project(window, cx);
                }
            }
            Evt::Catalog(list) => self.catalog = list,
            // A command that printed something: said as a toast, the thread is the agent's.
            Evt::Dispatched {
                output, send: None, ..
            } => {
                if let Some(out) = output.filter(|o| !o.trim().is_empty()) {
                    self.toast(
                        NotificationType::Info,
                        bench_ui::clip(&out, 300),
                        window,
                        cx,
                    );
                }
            }
            Evt::Usage(_, usage) => self.speed.usage(&usage),
            Evt::Complete { session, text } => {
                // Bench in the background: say the open chat is done (in front, it's on screen).
                if !window.is_window_active() {
                    let first = text
                        .lines()
                        .map(str::trim)
                        .find(|l| !l.is_empty())
                        .unwrap_or("Done");
                    let id = self.thread(cx).session.clone().unwrap_or_default();
                    crate::system::notify::post(
                        &id,
                        &self.current_title(cx),
                        &bench_ui::clip(first, 140),
                    );
                }
                self.speed.finish();
                // The answer is over: its speed is the thread's to keep (Bench measured it, the
                // agent stores it where the thread lives).
                let stat = self.speed.stat();
                if !stat.is_empty() {
                    let at = self.thread_ref(cx);
                    self.cmd_at(
                        &at,
                        Cmd::SaveSpeed {
                            session: session.clone(),
                            speed: stat.to_json(),
                        },
                    );
                }
                self.refresh_sessions();
            }
            Evt::Speed(_, stat) => {
                // Reopening a thread (or the app) brings its numbers back.
                self.speed.restore(stat);
            }
            _ => {}
        }
        cx.notify();
    }

    /// The view on screen showing the thread of runtime session `rt`, other than the open one.
    fn view_of_runtime(&self, rt: &str, cx: &App) -> Option<Entity<crate::thread::ThreadView>> {
        self.beside_thread
            .clone()
            .filter(|v| v.read(cx).thread.runtime.as_deref() == Some(rt))
    }

    /// One of the person's own chats (not Telegram, cron or API) went from working to done in a
    /// sessions refresh, and it isn't the one on screen: a notification. The open chat is told
    /// by its own `Complete` (with the answer's first line).
    pub(crate) fn notify_finished(&self, rows: &[crate::app::model::SessionRow], cx: &App) {
        for old in self.sessions.iter().filter(|s| s.running) {
            let done = rows
                .iter()
                .find(|r| r.id == old.id)
                .is_some_and(|r| !r.running);
            let own = crate::app::model::source_label(&old.source) == "Bench";
            if done && own && self.thread(cx).session.as_deref() != Some(old.id.as_str()) {
                let title = if old.title.is_empty() {
                    "Bench"
                } else {
                    &old.title
                };
                crate::system::notify::post(&old.id, title, "The agent finished");
            }
        }
    }
}
