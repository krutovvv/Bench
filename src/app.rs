//! Application state, gateway events and user actions. Rendering lives in `ui/`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::IndexPath;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{EditorState, InputEvent, TextareaState};
use gpui_kit::component::message_scroller::MessageScrollerState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::*;

use crate::audio::{Playback, Recorder};
use crate::connection::{self, Connection};
use crate::gateway::{Cmd, Evt, Gateway};
use crate::history;
use crate::model::{EFFORTS, Profile, SessionRow, SkillRow, Step, Target, Turn, Usage};
use crate::rest;

pub type Choice = SelectState<Vec<SharedString>>;

#[derive(Clone, Copy, PartialEq)]
pub enum Panel {
    Chat,
    /// Settings tabs.
    Skills,
    Mcp,
    Telegram,
}

impl Panel {
    pub fn is_settings(self) -> bool {
        self != Panel::Chat
    }
}

pub struct Live {
    pub started: Instant,
    pub label: SharedString,
}

pub struct HermesApp {
    pub targets: Vec<Target>,
    pub target: Target,
    pub target_choice: Entity<Choice>,
    gateway: Option<Gateway>,
    pub conn: Option<Arc<Connection>>,
    evt_tx: async_channel::Sender<Evt>,
    pub connecting: bool,

    pub profiles: Vec<Profile>,
    pub profile: Option<String>,
    pub profile_choice: Entity<Choice>,
    pub sessions: Vec<SessionRow>,
    /// Stored id of the open session (what the session list shows).
    pub session: Option<String>,
    /// The id the gateway uses for prompts and events of the open session.
    runtime: Option<String>,
    pub confirm_delete: Option<String>,
    /// Stored id -> runtime id for sessions this client has opened (to close before delete).
    opened: HashMap<String, String>,
    /// Telegram: session id -> owner user id; known people; whose sessions to show (None = all).
    tg_owner: HashMap<String, String>,
    pub tg_people: Vec<(String, String)>,
    pub tg_visible: Option<HashSet<String>>,

    pub turns: Vec<Turn>,
    pub live: Option<Live>,
    queued: Option<String>,
    pub usage: Option<Usage>,
    pub effort: usize,
    effort_pending: bool,

    pub panel: Panel,
    pub skills: Vec<SkillRow>,
    pub skill_open: Option<(String, String)>,
    pub mcp_editor: Entity<EditorState>,
    pub mcp_status: SharedString,
    pub mcp_error: Option<SharedString>,

    pub composer: Entity<TextareaState>,
    pub scroller: Entity<MessageScrollerState>,
    pub recorder: Option<Recorder>,
    pub transcribing: bool,
    pub playing: Option<(usize, Playback)>,
    pub speaking: Option<usize>,

    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

impl HermesApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (evt_tx, evt_rx) = async_channel::unbounded();

        let mut targets = vec![Target::Local];
        targets.extend(connection::ssh_hosts().into_iter().map(Target::Ssh));
        let labels: Vec<SharedString> = targets.iter().map(|t| t.label().into()).collect();
        // Reopen where we were last time.
        let last = std::fs::read_to_string(connection::state_dir().join("last-target"))
            .unwrap_or_default();
        let start = targets
            .iter()
            .position(|t| t.label() == last.trim())
            .unwrap_or(0);
        let initial = targets[start].clone();
        let target_choice =
            cx.new(|cx| SelectState::new(labels, Some(IndexPath::new(start)), window, cx));
        let profile_choice =
            cx.new(|cx| SelectState::new(Vec::<SharedString>::new(), None, window, cx));

        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 10)
                .submit_on_enter(true)
                .placeholder("Напиши Hermes…")
        });
        let mcp_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
        });
        let scroller = cx.new(|cx| MessageScrollerState::new(0, cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();

        let subs = vec![
            cx.subscribe_in(&composer, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { shift: false, .. } = ev {
                    this.send(window, cx);
                }
            }),
            // An error shown after a failed save goes away once the text is edited.
            cx.subscribe_in(&mcp_editor, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) && this.mcp_error.is_some() {
                    this.mcp_error = None;
                    cx.notify();
                }
            }),
            cx.subscribe_in(
                &target_choice,
                window,
                |this, _, ev: &SelectEvent<Vec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(label)) = ev else {
                        return;
                    };
                    if let Some(t) = this
                        .targets
                        .iter()
                        .find(|t| t.label() == label.as_ref())
                        .cloned()
                    {
                        if t != this.target || this.conn.is_none() {
                            this.connect(t, window, cx);
                        }
                    }
                },
            ),
            cx.subscribe_in(
                &profile_choice,
                window,
                |this, _, ev: &SelectEvent<Vec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(label)) = ev else {
                        return;
                    };
                    if let Some(p) = this.profiles.iter().find(|p| p.label == label.as_ref()) {
                        let name = p.name.clone();
                        this.select_profile(name, window, cx);
                    }
                },
            ),
        ];

        let events = cx.spawn_in(window, async move |this, cx| {
            while let Ok(evt) = evt_rx.recv().await {
                if this
                    .update_in(cx, |this, window, cx| this.on_event(evt, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        // Live clock + playback end detection.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    if this.playing.as_ref().is_some_and(|(_, p)| p.is_done()) {
                        this.playing = None;
                        cx.notify();
                    }
                    if this.live.is_some() || this.recorder.is_some() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let mut tasks = vec![events, ticker];

        // Hide SSH hosts that do not run Hermes.
        for host in targets.iter().filter_map(|t| match t {
            Target::Ssh(h) => Some(h.clone()),
            Target::Local => None,
        }) {
            tasks.push(cx.spawn_in(window, async move |this, cx| {
                let h = host.clone();
                let ok = cx
                    .background_spawn(async move { connection::has_hermes(&h) })
                    .await;
                if !ok {
                    let _ =
                        this.update_in(cx, |this, window, cx| this.drop_target(&host, window, cx));
                }
            }));
        }

        let mut app = Self {
            targets,
            target: initial.clone(),
            target_choice,
            gateway: None,
            conn: None,
            evt_tx,
            connecting: false,
            profiles: vec![],
            profile: None,
            profile_choice,
            sessions: vec![],
            session: None,
            runtime: None,
            confirm_delete: None,
            opened: HashMap::new(),
            tg_owner: HashMap::new(),
            tg_people: vec![],
            tg_visible: None,
            turns: vec![],
            live: None,
            queued: None,
            usage: None,
            effort: EFFORTS.len() - 1,
            effort_pending: false,
            panel: Panel::Chat,
            skills: vec![],
            skill_open: None,
            mcp_editor,
            mcp_status: "".into(),
            mcp_error: None,
            composer,
            scroller,
            recorder: None,
            transcribing: false,
            playing: None,
            speaking: None,
            _subscriptions: subs,
            _tasks: tasks,
        };
        app.connect(initial, window, cx);
        app
    }

    pub fn toast(
        &self,
        kind: NotificationType,
        msg: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.push_notification((kind, msg.into()), cx);
    }

    // ---- connection ------------------------------------------------------

    fn drop_target(&mut self, host: &str, window: &mut Window, cx: &mut Context<Self>) {
        let gone = Target::Ssh(host.to_string());
        if self.target == gone {
            return; // keep the one the user is on; its error explains itself
        }
        self.targets.retain(|t| *t != gone);
        let labels: Vec<SharedString> = self.targets.iter().map(|t| t.label().into()).collect();
        let current = self.targets.iter().position(|t| *t == self.target);
        self.target_choice.update(cx, |s, cx| {
            s.set_items(labels, window, cx);
            s.set_selected_index(current.map(IndexPath::new), window, cx);
        });
        cx.notify();
    }

    pub fn connect(&mut self, target: Target, _window: &mut Window, cx: &mut Context<Self>) {
        self.gateway = None; // drops the old thread
        self.conn = None;
        self.target = target.clone();
        self.connecting = true;
        self.opened.clear();
        self.profiles.clear();
        self.profile = None;
        self.sessions.clear();
        self.skills.clear();
        self.reset_chat(cx);
        let dir = connection::state_dir();
        let _ = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(dir.join("last-target"), target.label()));
        self.gateway = Some(Gateway::start(target, self.evt_tx.clone()));
        cx.notify();
    }

    fn cmd(&self, cmd: Cmd) {
        if let Some(g) = &self.gateway {
            g.send(cmd);
        }
    }

    pub fn current_profile(&self) -> String {
        self.profile.clone().unwrap_or_default()
    }

    /// Events carry the runtime id.
    fn is_current(&self, sid: &str) -> bool {
        self.runtime.as_deref() == Some(sid)
    }

    fn refresh_sessions(&self) {
        if let Some(p) = self.profile.clone() {
            self.cmd(Cmd::Sessions(p));
        }
    }

    // ---- events ----------------------------------------------------------

    fn on_event(&mut self, evt: Evt, window: &mut Window, cx: &mut Context<Self>) {
        match evt {
            Evt::Connected(conn) => {
                self.conn = Some(conn);
                self.connecting = false;
            }
            Evt::Error(e) => {
                self.connecting = false;
                if self.live.is_some() && self.turns.last().is_some_and(|t| t.answer.is_empty()) {
                    self.live = None;
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
                let pick = self
                    .profiles
                    .iter()
                    .find(|p| p.name == "deep-research")
                    .or(self.profiles.first())
                    .map(|p| p.name.clone());
                if let Some(p) = pick {
                    self.select_profile(p, window, cx);
                }
            }
            Evt::Sessions(p, rows) => {
                if self.profile.as_deref() == Some(&p) {
                    let unknown_tg = rows
                        .iter()
                        .any(|s| s.source == "telegram" && !self.tg_owner.contains_key(&s.id));
                    self.sessions = rows;
                    if unknown_tg {
                        self.load_telegram_owners(window, cx);
                    }
                }
            }
            Evt::SessionsChanged => self.refresh_sessions(),
            Evt::Deleted => self.refresh_sessions(),
            Evt::DeleteFailed(e) => {
                let msg = if e.contains("active") {
                    "Сессия сейчас открыта в другом клиенте (Hermes.app, Telegram) — там её и закройте".to_string()
                } else {
                    format!("Не удалось удалить: {e}")
                };
                self.toast(NotificationType::Error, msg, window, cx);
                self.refresh_sessions();
            }
            Evt::History {
                session,
                runtime,
                turns,
                running,
            } => {
                self.opened.insert(session.clone(), runtime.clone());
                if self.session.as_deref() == Some(&session) {
                    self.runtime = Some(runtime);
                    self.turns = turns;
                    self.live = running.then(|| Live {
                        started: Instant::now(),
                        label: "Работает…".into(),
                    });
                    self.reset_scroller(cx);
                }
            }
            Evt::Created { session, runtime } => {
                self.opened.insert(session.clone(), runtime.clone());
                self.session = Some(session);
                self.runtime = Some(runtime.clone());
                if let (Some(text), Some(profile)) = (self.queued.take(), self.profile.clone()) {
                    let effort = self.take_effort();
                    self.cmd(Cmd::Send {
                        profile,
                        session: runtime,
                        text,
                        effort,
                    });
                }
                // Show the new session in the list right away.
                self.refresh_sessions();
            }
            Evt::Started(sid) if self.is_current(&sid) => {
                if self.live.is_none() {
                    self.live = Some(Live {
                        started: Instant::now(),
                        label: "Думает…".into(),
                    });
                }
            }
            Evt::Delta(sid, text) if self.is_current(&sid) => {
                if let Some(turn) = self.turns.last_mut() {
                    turn.answer.push_str(&text);
                }
                if let Some(live) = self.live.as_mut() {
                    live.label = "Пишет ответ…".into();
                }
                self.remeasure_last(cx);
            }
            Evt::Reasoning(sid, text) if self.is_current(&sid) => {
                if let Some(turn) = self.turns.last_mut() {
                    match turn.steps.last_mut() {
                        Some(Step::Thought(t)) => t.push_str(&text),
                        _ => turn.steps.push(Step::Thought(text)),
                    }
                }
                if let Some(live) = self.live.as_mut() {
                    live.label = "Думает…".into();
                }
                self.remeasure_last(cx);
            }
            Evt::ToolStart {
                session,
                name,
                detail,
                urls,
            } if self.is_current(&session) => {
                if let Some(turn) = self.turns.last_mut() {
                    for url in urls {
                        if !turn.sources.contains(&url) {
                            turn.sources.push(url);
                        }
                    }
                    let narration = last_line(&turn.answer);
                    history::demote_answer(turn);
                    if let Some(live) = self.live.as_mut() {
                        live.label = if narration.is_empty() {
                            tool_label(&name, &detail).into()
                        } else {
                            narration.into()
                        };
                    }
                    turn.steps.push(Step::Tool {
                        name,
                        detail,
                        failed: false,
                    });
                }
                self.remeasure_last(cx);
            }
            Evt::ToolDone { session, failed } if self.is_current(&session) => {
                if let Some(Some(Step::Tool { failed: f, .. })) = self.turns.last_mut().map(|t| {
                    t.steps
                        .iter_mut()
                        .rev()
                        .find(|s| matches!(s, Step::Tool { .. }))
                }) {
                    *f = failed;
                }
            }
            Evt::Usage(sid, usage) if self.is_current(&sid) => self.usage = Some(usage),
            Evt::Complete { session, text } if self.is_current(&session) => {
                self.live = None;
                if let Some(turn) = self.turns.last_mut() {
                    turn.finished = Some(now_secs());
                    if !text.trim().is_empty() {
                        turn.answer = text;
                    }
                }
                self.remeasure_last(cx);
                self.refresh_sessions();
            }
            _ => {}
        }
        cx.notify();
    }

    // ---- chat ------------------------------------------------------------

    fn reset_chat(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        self.runtime = None;
        self.turns.clear();
        self.live = None;
        self.queued = None;
        self.usage = None;
        self.playing = None;
        self.reset_scroller(cx);
    }

    fn reset_scroller(&mut self, cx: &mut Context<Self>) {
        let n = self.turns.len();
        self.scroller.update(cx, |s, cx| {
            s.reset(n, cx);
            s.scroll_to_end(cx);
        });
    }

    pub fn remeasure(&mut self, i: usize, cx: &mut Context<Self>) {
        self.scroller.update(cx, |s, cx| {
            let _ = s.remeasure_items(i..i + 1, cx);
        });
    }

    fn remeasure_last(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.turns.len().checked_sub(1) {
            self.remeasure(i, cx);
        }
    }

    pub fn select_profile(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let idx = self.profiles.iter().position(|p| p.name == name);
        self.profile_choice.update(cx, |s, cx| {
            s.set_selected_index(idx.map(IndexPath::new), window, cx)
        });
        self.profile = Some(name.clone());
        self.load_profile_effort(window, cx);
        self.sessions.clear();
        self.tg_owner.clear();
        self.tg_people.clear();
        self.tg_visible = None;
        self.skills.clear();
        self.skill_open = None;
        self.reset_chat(cx);
        self.cmd(Cmd::Sessions(name));
        if self.panel != Panel::Chat {
            let panel = self.panel;
            self.open_panel(panel, window, cx);
        }
        cx.notify();
    }

    pub fn open_session(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.panel = Panel::Chat;
        self.confirm_delete = None;
        self.reset_chat(cx);
        self.session = Some(id.clone());
        self.cmd(Cmd::Open {
            profile,
            session: id,
        });
        cx.notify();
    }

    pub fn new_session(&mut self, cx: &mut Context<Self>) {
        self.panel = Panel::Chat;
        self.reset_chat(cx);
        cx.notify();
    }

    /// First click arms, second click deletes.
    pub fn delete_session(&mut self, id: String, cx: &mut Context<Self>) {
        if self.confirm_delete.as_deref() != Some(&id) {
            self.confirm_delete = Some(id);
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        let Some(profile) = self.profile.clone() else {
            return;
        };
        if self.session.as_deref() == Some(&id) {
            self.reset_chat(cx);
        }
        self.sessions.retain(|s| s.id != id);
        let runtime = self.opened.remove(&id);
        self.cmd(Cmd::Delete {
            profile,
            session: id,
            runtime,
        });
        cx.notify();
    }

    pub fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        cx.notify();
    }

    pub fn set_effort(&mut self, i: usize, cx: &mut Context<Self>) {
        self.effort = i;
        self.effort_pending = true;
        cx.notify();
    }

    fn take_effort(&mut self) -> Option<String> {
        std::mem::take(&mut self.effort_pending).then(|| EFFORTS[self.effort].to_string())
    }

    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value().trim().to_string();
        let Some(profile) = self.profile.clone() else {
            return;
        };
        if text.is_empty() || self.live.is_some() {
            return;
        }
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.turns.push(Turn {
            user: text.clone(),
            started: Some(now_secs()),
            ..Default::default()
        });
        self.scroller.update(cx, |s, cx| {
            let _ = s.append(1, cx);
            s.scroll_to_end(cx);
        });
        self.live = Some(Live {
            started: Instant::now(),
            label: "Отправляю…".into(),
        });
        match self.runtime.clone() {
            Some(session) => {
                let effort = self.take_effort();
                self.cmd(Cmd::Send {
                    profile,
                    session,
                    text,
                    effort,
                });
            }
            None => {
                // A new session is created on the first message; the effort goes with it.
                self.effort_pending = true;
                self.queued = Some(text);
                self.cmd(Cmd::Create { profile });
            }
        }
        cx.notify();
    }

    /// Ask the last question again: the gateway drops the last exchange, then the same text is sent.
    pub fn regenerate(&mut self, cx: &mut Context<Self>) {
        let (Some(profile), Some(session)) = (self.profile.clone(), self.runtime.clone()) else {
            return;
        };
        if self.live.is_some() {
            return;
        }
        let Some(turn) = self.turns.last_mut() else {
            return;
        };
        let text = turn.user.clone();
        *turn = Turn {
            user: text.clone(),
            started: Some(now_secs()),
            ..Default::default()
        };
        self.live = Some(Live {
            started: Instant::now(),
            label: "Повторяю…".into(),
        });
        self.remeasure_last(cx);
        self.cmd(Cmd::Regenerate {
            profile,
            session,
            text,
        });
        cx.notify();
    }

    pub fn copy_answer(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(t) = self.turns.get(i) {
            cx.write_to_clipboard(ClipboardItem::new_string(t.answer.clone()));
            self.toast(NotificationType::Success, "Скопировано", window, cx);
        }
    }

    /// `hermes gateway restart` on the machine we are connected to, then reconnect.
    pub fn restart_gateway(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        self.toast(NotificationType::Info, "Перезапускаю шлюз…", window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let t = target.clone();
            let result = cx
                .background_spawn(async move { connection::restart_gateway(&t) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(()) => {
                    this.toast(NotificationType::Success, "Шлюз перезапущен", window, cx);
                    this.connect(target, window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Перезапуск: {e:#}"),
                    window,
                    cx,
                ),
            });
        })
        .detach();
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let (Some(profile), Some(session)) = (self.profile.clone(), self.runtime.clone()) {
            self.cmd(Cmd::Interrupt { profile, session });
        }
        cx.notify();
    }

    pub fn toggle_turn(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(t) = self.turns.get_mut(i) {
            t.expanded = !t.expanded;
            self.remeasure(i, cx);
            cx.notify();
        }
    }

    // ---- background REST helpers -------------------------------------------

    /// Run `job` off the UI thread with the current connection, then `done` on it.
    fn rest<T: Send + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        job: impl FnOnce(&Connection, &str) -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let profile = self.current_profile();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { job(&conn, &profile) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                done(this, result, window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    // ---- profile defaults & telegram filter ---------------------------------

    /// Start each profile on its configured reasoning effort.
    fn load_profile_effort(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.effort_pending = false;
        self.rest(window, cx, rest::profile_defaults, |this, r, _, _| {
            let Ok((effort, model)) = r else { return };
            if let Some(i) = effort.and_then(|v| EFFORTS.iter().position(|e| *e == v)) {
                this.effort = i;
                this.effort_pending = false;
            }
            if let (Some(model), Some(p)) = (model, this.profile.clone()) {
                if let Some(profile) = this.profiles.iter_mut().find(|x| x.name == p) {
                    profile.model = model;
                }
            }
        });
    }

    fn telegram_prefs_path(&self) -> std::path::PathBuf {
        let key: String = format!("{}-{}", self.target.label(), self.current_profile())
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        connection::state_dir().join(format!("telegram-{key}.json"))
    }

    fn load_telegram_owners(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest(window, cx, rest::telegram_owners, |this, r, _, cx| {
            let Ok(rows) = r else { return };
            for (session, user, name) in rows {
                if !this.tg_people.iter().any(|(u, _)| *u == user) {
                    this.tg_people.push((user.clone(), name));
                }
                this.tg_owner.insert(session, user);
            }
            if this.tg_visible.is_none() {
                this.tg_visible = this.initial_telegram_visible();
            }
            cx.notify();
        });
    }

    /// Saved choice, else the account whose name matches this Mac's user, else everyone.
    fn initial_telegram_visible(&self) -> Option<HashSet<String>> {
        if let Ok(text) = std::fs::read_to_string(self.telegram_prefs_path()) {
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(&text) {
                return Some(ids.into_iter().collect());
            }
        }
        let me = std::process::Command::new("id")
            .arg("-F")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase())
            .unwrap_or_default();
        let first = me.split_whitespace().next().unwrap_or_default().to_string();
        let mine: HashSet<String> = self
            .tg_people
            .iter()
            .filter(|(_, name)| {
                !first.is_empty() && name.to_lowercase().split_whitespace().any(|w| w == first)
            })
            .map(|(u, _)| u.clone())
            .collect();
        (!mine.is_empty()).then_some(mine)
    }

    pub fn toggle_telegram_person(&mut self, user: String, cx: &mut Context<Self>) {
        let all: HashSet<String> = self.tg_people.iter().map(|(u, _)| u.clone()).collect();
        let mut visible = self.tg_visible.clone().unwrap_or(all);
        if !visible.remove(&user) {
            visible.insert(user);
        }
        let ids: Vec<&String> = visible.iter().collect();
        let _ = std::fs::write(
            self.telegram_prefs_path(),
            serde_json::to_string(&ids).unwrap_or_default(),
        );
        self.tg_visible = Some(visible);
        cx.notify();
    }

    /// Indices into `sessions` the list should show (other people's Telegram chats filtered out).
    pub fn visible_sessions(&self) -> Vec<usize> {
        (0..self.sessions.len())
            .filter(|&i| {
                let s = &self.sessions[i];
                match (&self.tg_visible, s.source.as_str()) {
                    (Some(vis), "telegram") => {
                        self.tg_owner.get(&s.id).is_none_or(|u| vis.contains(u))
                    }
                    _ => true,
                }
            })
            .collect()
    }

    // ---- panels ----------------------------------------------------------

    pub fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = panel;
        match panel {
            Panel::Chat | Panel::Telegram => {}
            Panel::Skills => self.rest(window, cx, rest::skills, |this, r, window, cx| match r {
                Ok(list) => this.skills = list,
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Скиллы: {e:#}"),
                    window,
                    cx,
                ),
            }),
            Panel::Mcp => {
                self.mcp_status = "Загружаю…".into();
                self.rest(window, cx, rest::mcp_json, |this, r, window, cx| match r {
                    Ok(text) => {
                        this.mcp_editor
                            .update(cx, |s, cx| s.set_value(text, window, cx));
                        this.mcp_status = "".into();
                        this.mcp_error = None;
                    }
                    Err(e) => this.mcp_status = format!("Не удалось загрузить: {e:#}").into(),
                })
            }
        }
        cx.notify();
    }

    pub fn open_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.skill_open = Some((name.clone(), "Загружаю…".into()));
        self.rest(
            window,
            cx,
            move |c, p| rest::skill_content(c, p, &name),
            |this, r, _, _| {
                if let Some((_, body)) = this.skill_open.as_mut() {
                    *body = r.unwrap_or_else(|e| format!("Не удалось открыть: {e:#}"));
                }
            },
        );
        cx.notify();
    }

    /// JSON check run on save; an invalid document is not sent.
    fn validate_mcp(&mut self, cx: &mut Context<Self>) {
        let text = self.mcp_editor.read(cx).value().to_string();
        self.mcp_error = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) if v.is_object() => None,
            Ok(_) => Some("Нужен объект: { \"имя-сервера\": { … } }".into()),
            Err(e) => Some(format!("Строка {}, столбец {}: {e}", e.line(), e.column()).into()),
        };
        cx.notify();
    }

    pub fn save_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.validate_mcp(cx);
        if self.mcp_error.is_some() {
            return;
        }
        let text = self.mcp_editor.read(cx).value().to_string();
        self.mcp_status = "Сохраняю…".into();
        self.rest(
            window,
            cx,
            move |c, p| rest::save_mcp_json(c, p, &text),
            |this, r, window, cx| {
                this.mcp_status = "".into();
                match r {
                    Ok(()) => this.toast(
                        NotificationType::Success,
                        "MCP сохранены. Подхватятся в новой сессии.",
                        window,
                        cx,
                    ),
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("MCP не сохранены: {e:#}"),
                        window,
                        cx,
                    ),
                }
            },
        );
    }

    // ---- voice -----------------------------------------------------------

    pub fn toggle_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rec) = self.recorder.take() {
            match rec.finish() {
                Ok(wav) => {
                    self.transcribing = true;
                    self.rest(
                        window,
                        cx,
                        move |c, p| rest::transcribe(c, p, &wav),
                        |this, r, window, cx| {
                            this.transcribing = false;
                            match r {
                                Ok(text) if !text.is_empty() => {
                                    let current = this.composer.read(cx).value().to_string();
                                    let joined = if current.trim().is_empty() {
                                        text
                                    } else {
                                        format!("{} {text}", current.trim_end())
                                    };
                                    this.composer
                                        .update(cx, |s, cx| s.set_value(joined, window, cx));
                                }
                                Ok(_) => this.toast(
                                    NotificationType::Warning,
                                    "Ничего не распознано",
                                    window,
                                    cx,
                                ),
                                Err(e) => this.toast(
                                    NotificationType::Error,
                                    format!("Распознавание: {e:#}"),
                                    window,
                                    cx,
                                ),
                            }
                        },
                    );
                }
                Err(e) => self.toast(
                    NotificationType::Warning,
                    format!("Запись: {e:#}"),
                    window,
                    cx,
                ),
            }
        } else {
            match Recorder::start() {
                Ok(r) => self.recorder = Some(r),
                Err(e) => self.toast(
                    NotificationType::Error,
                    format!("Микрофон: {e:#}"),
                    window,
                    cx,
                ),
            }
        }
        cx.notify();
    }

    pub fn toggle_speak(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.playing.as_ref().is_some_and(|(j, _)| *j == i) {
            self.playing = None;
            cx.notify();
            return;
        }
        self.playing = None;
        let Some(text) = self.turns.get(i).map(|t| t.answer.clone()) else {
            return;
        };
        self.speaking = Some(i);
        self.rest(
            window,
            cx,
            move |c, p| rest::speak(c, p, &text),
            move |this, r, window, cx| {
                this.speaking = None;
                match r {
                    Ok(bytes) => this.playing = Some((i, Playback::start(bytes))),
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("Озвучка: {e:#}"),
                        window,
                        cx,
                    ),
                }
            },
        );
        cx.notify();
    }
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

pub fn last_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .last()
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect()
}

pub fn tool_label(name: &str, detail: &str) -> String {
    let name = name.replace('_', " ");
    if detail.is_empty() {
        name
    } else {
        format!("{name}: {detail}")
    }
}
