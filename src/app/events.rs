//! Gateway events: what the agent says, applied to the window's state.

use gpui_kit::component::IndexPath;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::client::rpc::{Cmd, Evt};

use super::*;

impl BenchApp {
    pub(crate) fn on_event(&mut self, evt: Evt, window: &mut Window, cx: &mut Context<Self>) {
        // An event of one thread goes to that thread's state; the rest of what it does (the status
        // bar, a notification, a toast) is the open thread's only.
        if let Some(rt) = evt.runtime() {
            if !self.is_current(rt) {
                if let Some(t) = self.thread_beside_mut(rt) {
                    t.on_event(&evt);
                    cx.notify();
                }
                return;
            }
            self.active_thread.on_event(&evt);
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
                if self.active_thread.live.is_some() && !self.active_thread.journal.running() {
                    self.active_thread.live = None;
                    self.active_thread.journal.clear_sent();
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
                    self.notify_finished(&rows);
                    // The servers' threads stay (they come from their own agents).
                    self.sessions.retain(|s| !s.host.is_empty());
                    self.sessions.extend(rows);
                    if let Some(id) = self.resume_session.take() {
                        if self.active_thread.session.is_none()
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
                if self
                    .active_thread
                    .on_history(&session, runtime.clone(), open.clone())
                {
                    self.resolve_project(window, cx);
                } else if let Some(t) = self.beside_thread.as_mut() {
                    t.on_history(&session, runtime, open);
                }
            }
            Evt::Thread(_, e) => {
                if let ::thread::ThreadEvent::Queue(items) = &e {
                    // A message that left the queue (sent, taken back) has nothing left to fix.
                    if self
                        .queue_edit
                        .as_ref()
                        .is_some_and(|(id, _)| !items.iter().any(|q| &q.sub == id))
                    {
                        self.queue_edit = None;
                    }
                }
            }
            Evt::Effort(_, value) => {
                // The session's own level replaces whatever the bar showed (the profile's default
                // or the previous chat's).
                self.effort_pending = false;
                match self.efforts.iter().position(|e| *e == value) {
                    Some(i) => {
                        self.active_thread.effort = i;
                        self.active_thread.effort_other = None;
                        self.effort_choice.update(cx, |s, cx| {
                            s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                        });
                    }
                    None if value.is_empty() => {
                        self.active_thread.effort_other = Some("default".into())
                    }
                    None => self.active_thread.effort_other = Some(value),
                }
            }
            Evt::Created { session, runtime } => {
                // In the list at once with its subject: the first message's block state (and the
                // block itself) depend on it, and the sessions list comes back only later.
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
                            project: self.active_thread.thread_project.clone(),
                            host: self.thread_host(),
                        },
                    );
                }
                self.opened.insert(session.clone(), runtime.clone());
                self.inserts_created(&session);
                self.active_thread.session = Some(session);
                self.save_place();
                self.active_thread.runtime = Some(runtime.clone());
                if let (Some(text), Some(profile)) =
                    (self.active_thread.queued.take(), self.profile.clone())
                {
                    let effort = self.take_effort();
                    match self.parse_command(&text) {
                        Some((name, arg)) => self.cmd(Cmd::Dispatch {
                            profile,
                            session: runtime,
                            name,
                            arg,
                        }),
                        None => {
                            let context = self.block_context(cx);
                            self.cmd(Cmd::Send {
                                profile,
                                session: runtime,
                                text,
                                effort,
                                context,
                            })
                        }
                    }
                }
                // Show the new session in the list right away.
                self.refresh_sessions();
            }
            Evt::Inserts(sid, list) => self.on_inserts(sid, list, cx),
            Evt::ResumeFailed(sid, why) => {
                for t in std::iter::once(&mut self.active_thread).chain(self.beside_thread.as_mut())
                {
                    if t.session.as_deref() == Some(sid.as_str()) {
                        t.history_error = Some(why.clone());
                    }
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
                if self.active_thread.session.as_deref() == Some(&sid) {
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
                let subject = self.current_subject();
                if subject.starts_with("tasks:") {
                    self.load_tasks();
                }
                if self.active_thread.project.is_none() {
                    self.resolve_project(window, cx);
                }
            }
            Evt::Catalog(list) => self.catalog = list,
            Evt::Processes(_, list) => self.on_processes(list, cx),
            Evt::Dispatched {
                session,
                output,
                send,
            } => match send {
                // The command expanded into a prompt (skills, /goal, …): run it as a turn.
                Some(message) => {
                    if let Some(profile) = self.profile.clone() {
                        self.speed.request();
                        let context = self.block_context(cx);
                        self.cmd(Cmd::Send {
                            profile,
                            session,
                            text: message,
                            effort: None,
                            context,
                        });
                    }
                }
                None => {
                    // A command that printed something: said as a toast, the thread is the agent's.
                    if let Some(out) = output.filter(|o| !o.trim().is_empty()) {
                        self.toast(
                            NotificationType::Info,
                            bench_ui::clip(&out, 300),
                            window,
                            cx,
                        );
                    }
                }
            },
            // The free answer's field starts empty for a new question.
            Evt::Clarify(_) => {
                self.clarify_input
                    .update(cx, |s, cx| s.set_value("", window, cx));
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
                    let id = self.active_thread.session.clone().unwrap_or_default();
                    crate::system::notify::post(
                        &id,
                        &self.current_title(),
                        &bench_ui::clip(first, 140),
                    );
                }
                self.speed.finish();
                // The answer is over: its speed is the thread's to keep (Bench measured it, the
                // agent stores it where the thread lives).
                let stat = self.speed.stat();
                if !stat.is_empty() {
                    self.cmd(Cmd::SaveSpeed {
                        session: session.clone(),
                        speed: stat.to_json(),
                    });
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

    /// One of the person's own chats (not Telegram, cron or API) went from working to done in a
    /// sessions refresh, and it isn't the one on screen: a notification. The open chat is told
    /// by its own `Complete` (with the answer's first line).
    pub(crate) fn notify_finished(&self, rows: &[crate::app::model::SessionRow]) {
        for old in self.sessions.iter().filter(|s| s.running) {
            let done = rows
                .iter()
                .find(|r| r.id == old.id)
                .is_some_and(|r| !r.running);
            let own = crate::app::model::source_label(&old.source) == "Bench";
            if done && own && self.active_thread.session.as_deref() != Some(old.id.as_str()) {
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
