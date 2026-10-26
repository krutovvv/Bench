//! Gateway events: what the agent says, applied to the window's state.

use gpui_kit::component::IndexPath;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::gateway::{Cmd, Evt};

use super::*;

impl BenchApp {
    pub(crate) fn on_event(&mut self, evt: Evt, window: &mut Window, cx: &mut Context<Self>) {
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
                self.load_remote_hosts(window, cx);
                self.load_projects(window, cx);
            }
            Evt::Error(e) => {
                self.connecting = false;
                // A send that failed: no run started, and the message never reached the journal.
                if self.live.is_some() && !self.thread.running() {
                    self.live = None;
                    self.thread.clear_sent();
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
                        if self.session.is_none() && self.sessions.iter().any(|s| s.id == id) {
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
                    self.open_panel(Panel::Providers, window, cx);
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
                if self.session.as_deref() == Some(&session) {
                    let running = open.running;
                    // A thread already on screen keeps what it has and only catches up.
                    if self.runtime.as_deref() == Some(runtime.as_str()) || !self.thread.is_empty()
                    {
                        self.thread.catch_up(open);
                    } else {
                        self.thread = ::thread::Thread::open(open);
                    }
                    self.runtime = Some(runtime);
                    self.history_error = None;
                    self.live = running.then(|| Live {
                        label: "Working…".into(),
                    });
                    self.resolve_project(window, cx);
                }
            }
            Evt::Thread(sid, e) if self.is_current(&sid) => {
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
                self.thread.apply(e);
            }
            Evt::Fallback(sid, note) if self.is_current(&sid) => {
                self.fallback_on = note;
                cx.notify();
            }
            Evt::Effort(sid, value) if self.is_current(&sid) => {
                // The session's own level replaces whatever the bar showed (the profile's default
                // or the previous chat's).
                self.effort_pending = false;
                match self.efforts.iter().position(|e| *e == value) {
                    Some(i) => {
                        self.effort = i;
                        self.effort_other = None;
                        self.effort_choice.update(cx, |s, cx| {
                            s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                        });
                    }
                    None if value.is_empty() => self.effort_other = Some("default".into()),
                    None => self.effort_other = Some(value),
                }
            }
            Evt::Created { session, runtime } => {
                // In the list at once with its subject: the first message's block state (and the
                // block itself) depend on it, and the sessions list comes back only later.
                if !self.sessions.iter().any(|s| s.id == session) {
                    self.sessions.insert(
                        0,
                        crate::model::SessionRow {
                            id: session.clone(),
                            title: "Untitled".into(),
                            messages: 0,
                            running: true,
                            source: "desktop".into(),
                            subject: self.new_subject.clone(),
                            project: self.thread_project.clone(),
                            host: self.thread_host(),
                        },
                    );
                }
                self.opened.insert(session.clone(), runtime.clone());
                self.inserts_created(&session);
                self.session = Some(session);
                self.save_place();
                self.runtime = Some(runtime.clone());
                if let (Some(text), Some(profile)) = (self.queued.take(), self.profile.clone()) {
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
                if self.session.as_deref() == Some(sid.as_str()) {
                    self.history_error = Some(why);
                }
            }
            Evt::InsertFailed(sid, e) => {
                self.toast(
                    NotificationType::Error,
                    format!("The insert didn't change: {e}"),
                    window,
                    cx,
                );
                // The agent's list is the truth: read it again.
                if self.session.as_deref() == Some(&sid) {
                    self.open_session(sid, cx);
                }
            }
            Evt::Started(sid) if self.is_current(&sid) => {
                if self.live.is_none() {
                    self.live = Some(Live {
                        label: "Thinking…".into(),
                    });
                }
            }
            Evt::Delta(sid, text) if self.is_current(&sid) => {
                self.speed.token(text.chars().count());
                if let Some(live) = self.live.as_mut() {
                    live.label = "Writing…".into();
                }
            }
            Evt::Reasoning(sid, text) if self.is_current(&sid) => {
                self.speed.token(text.chars().count());
                if let Some(live) = self.live.as_mut() {
                    live.label = "Thinking…".into();
                }
            }
            Evt::ToolStart {
                session,
                name,
                detail,
            } if self.is_current(&session) => {
                self.speed.tool_call();
                if let Some(live) = self.live.as_mut() {
                    live.label = tool_label(&name, &detail).into();
                }
            }
            Evt::ToolDone { session } if self.is_current(&session) => {
                self.speed.request();
                // The agent may have changed the list this thread is about.
                let subject = self.current_subject();
                if subject.starts_with("tasks:") {
                    self.load_tasks();
                } else if let Some(id) = subject.strip_prefix("env:") {
                    self.load_env(id, window, cx);
                }
                if self.project.is_none() {
                    self.resolve_project(window, cx);
                }
            }
            Evt::Catalog(list) => self.catalog = list,
            Evt::Processes(sid, list) if self.is_current(&sid) => self.on_processes(list, cx),
            Evt::Dispatched {
                session,
                output,
                send,
            } if self.is_current(&session) => match send {
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
                    self.live = None;
                    if let Some(out) = output.filter(|o| !o.trim().is_empty()) {
                        self.toast(
                            NotificationType::Info,
                            crate::ui::clip(&out, 300),
                            window,
                            cx,
                        );
                    }
                }
            },
            Evt::Approval(req) if self.is_current(&req.session) => {
                if let Some(live) = self.live.as_mut() {
                    live.label = "Waiting for your approval…".into();
                }
                self.approval = Some(req);
            }
            Evt::ApprovalDone(sid) => {
                if self.approval.as_ref().is_some_and(|a| a.session == sid) {
                    self.approval = None;
                }
            }
            Evt::Clarify(req) if self.is_current(&req.session) => {
                if let Some(live) = self.live.as_mut() {
                    live.label = "Waiting for your answer…".into();
                }
                self.clarify = Some(req);
                self.clarify_picked.clear();
                self.clarify_input
                    .update(cx, |s, cx| s.set_value("", window, cx));
            }
            Evt::ClarifyDone(sid) => {
                if self.clarify.as_ref().is_some_and(|c| c.session == sid) {
                    self.clarify = None;
                }
            }
            Evt::Todos(sid, todos, _at) if self.is_current(&sid) => {
                // The plan block shows work still ahead: once no step is pending or in progress
                // (the agent finished the plan, or dropped it as wrong), the block goes by itself
                // — a plan that is over tells nothing the thread does not already say.
                let live = todos
                    .iter()
                    .any(|t| t.status == "pending" || t.status == "in_progress");
                if live {
                    self.todos = todos;
                } else {
                    self.todos.clear();
                }
            }
            Evt::Usage(sid, usage) if self.is_current(&sid) => {
                self.speed.usage(&usage);
                self.usage = Some(usage);
            }
            Evt::Complete { session, text } if self.is_current(&session) => {
                // Bench in the background: say the open chat is done (in front, it's on screen).
                if !window.is_window_active() {
                    let first = text
                        .lines()
                        .map(str::trim)
                        .find(|l| !l.is_empty())
                        .unwrap_or("Done");
                    let id = self.session.clone().unwrap_or_default();
                    crate::notify::post(&id, &self.current_title(), &crate::ui::clip(first, 140));
                }
                self.live = None;
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
            Evt::Speed(session, stat) if self.is_current(&session) => {
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
    pub(crate) fn notify_finished(&self, rows: &[crate::model::SessionRow]) {
        for old in self.sessions.iter().filter(|s| s.running) {
            let done = rows
                .iter()
                .find(|r| r.id == old.id)
                .is_some_and(|r| !r.running);
            let own = crate::model::source_label(&old.source) == "Bench";
            if done && own && self.session.as_deref() != Some(old.id.as_str()) {
                let title = if old.title.is_empty() {
                    "Bench"
                } else {
                    &old.title
                };
                crate::notify::post(&old.id, title, "The agent finished");
            }
        }
    }
}
