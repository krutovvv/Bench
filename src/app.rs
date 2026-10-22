//! Application state, gateway events and user actions. Rendering lives in `ui/`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::IndexPath;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::diff::{DiffFile, DiffState};
use gpui_kit::component::input::{EditorState, InputEvent, InputState, TextareaState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::*;

mod cron;
mod gateways;
pub(crate) mod projects;
mod prompts;
mod slash;
pub use projects::GitArm;

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
    Config,
    Cron,
    Telegram,
    Prompts,
    Gateways,
    Appearance,
}

impl Panel {
    pub fn is_settings(self) -> bool {
        self != Panel::Chat
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum RightTab {
    Changes,
    Files,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FileMode {
    Diff,
    Before,
    After,
}

/// The file open in the right panel. `diffs` are this session's tool diffs for it, in order.
pub struct FileView {
    pub path: String,
    pub mode: FileMode,
    pub diffs: Vec<String>,
    pub loaded: bool,
    pub truncated: bool,
    /// The file is gone from disk (an old session): only the recorded edits can be shown.
    pub missing: bool,
    /// Diff view: the whole file with the changes in place, else just the tool diffs.
    pub diff_view: Option<Entity<DiffState>>,
    /// The original text, when it could be reconstructed from the edits.
    pub before_view: Option<Entity<DiffState>>,
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
    /// The profile's primary model and its fallbacks, as (model, provider).
    pub profile_models: Vec<(String, String)>,
    /// Slash commands of the profile; the popup's highlighted row; Esc closed it for this text.
    pub catalog: Vec<crate::model::SlashCommand>,
    pub slash_sel: usize,
    pub slash_closed: bool,
    pub slash_scroll: ScrollHandle,
    /// The agent waits for an answer (approval mode manual/smart).
    pub approval: Option<crate::model::ApprovalReq>,
    /// The profile's `approvals.mode`, once read.
    pub approval_mode: Option<String>,
    /// `model.context_length` of the profile (0 when not set).
    pub profile_context: u64,
    /// Session folders/repos as Hermes recorded them, and repos inferred from edited files.
    pub places: HashMap<String, crate::model::Place>,
    pub inferred: HashMap<String, String>,
    /// The open session's project (None for chats that touch no repo).
    pub project: Option<crate::model::Project>,
    /// Hermes' default working folder (sessions started there have no folder of their own).
    pub default_cwd: Option<String>,
    /// Reopen this session once the list arrives (where the app was last closed).
    resume_session: Option<String>,
    /// For the next new chat: work in this repo, and in a branch of its own.
    pub new_project: Option<String>,
    pub new_branch: bool,
    /// The chosen folder is a git repo (so "in a new branch" applies).
    pub new_project_repo: bool,
    /// Typing a folder path (SSH hosts have no native folder dialog here).
    pub path_entry: bool,
    pub project_path: Entity<InputState>,
    pub git_armed: Option<GitArm>,
    /// Settings → Gateways: each machine's install, by target label.
    pub installs: HashMap<String, crate::model::Install>,
    pub appearance: crate::theme::Appearance,
    pub speed: crate::speed::Meter,
    /// Sidebar search: the field, what was typed, and the gateway's hits (None while searching).
    pub search: Entity<InputState>,
    pub search_query: String,
    pub search_hits: Option<Vec<(String, String)>>,
    search_seq: u64,
    /// The model picked in the status bar for this session (None = profile default).
    pub model_pick: Option<usize>,
    model_pending: bool,
    effort_pending: bool,

    pub panel: Panel,
    pub skills: Vec<SkillRow>,
    pub skill_open: Option<String>,
    pub skill_editor: Entity<EditorState>,
    pub skill_delete_armed: bool,
    pub config_editor: Entity<EditorState>,
    pub config_error: Option<SharedString>,
    pub cron: Vec<rest::CronJob>,
    pub cron_open: Option<String>,
    /// Settings → Prompts.
    pub prompts: Vec<crate::model::Prompt>,
    pub prompt_open: Option<usize>,
    pub prompt_title: Entity<InputState>,
    pub prompt_editor: Entity<EditorState>,
    pub prompt_delete_armed: bool,
    pub cron_editor: Entity<EditorState>,
    pub cron_schedule: Entity<InputState>,
    pub effort_choice: Entity<Choice>,
    pub mcp_editor: Entity<EditorState>,
    pub mcp_status: SharedString,
    pub mcp_error: Option<SharedString>,

    pub composer: Entity<TextareaState>,
    /// The chat column scrolls as one tall list (no virtualization: real heights, real bottom).
    pub chat_scroll: ScrollHandle,
    /// Keep the view pinned to the newest content until the user scrolls up.
    pub follow: bool,
    pub last_max: Pixels,
    /// Unfolded steps (turn, step) and the source host whose pages are listed (turn, host).
    pub open_steps: HashSet<(usize, usize)>,
    pub open_source: Option<(usize, String)>,
    /// Right panel: changed files / file tree, and the file being viewed.
    pub right: Option<RightTab>,
    pub file: Option<FileView>,
    pub file_editor: Entity<EditorState>,
    /// The file list beside the viewer (remembered; hidden by default).
    pub show_list: bool,
    pub tree_root: Option<String>,
    pub tree: HashMap<String, Vec<(String, String, bool)>>,
    pub tree_open: HashSet<String>,
    pub recorder: Option<Recorder>,
    /// Repaints the waveform while recording.
    wave_ticker: Option<Task<()>>,
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
                .placeholder("Message Hermes…")
        });
        if let Ok(v) = std::env::var("HERMES_GPUI_PREFILL") {
            composer.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search sessions"));
        let project_path =
            cx.new(|cx| InputState::new(window, cx).placeholder("/home/you/project — Enter"));
        let mcp_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
        });
        let skill_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(true)
        });
        let prompt_title = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let prompt_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(true)
        });
        let cron_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(true)
        });
        let cron_schedule = cx.new(|cx| {
            InputState::new(window, cx).placeholder("30 10 * * 1-5  ·  every 2h  ·  daily at 9:00")
        });
        let config_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("yaml")
                .line_number(true)
        });
        let effort_labels: Vec<SharedString> =
            EFFORTS.iter().map(|e| SharedString::from(*e)).collect();
        let effort_choice = cx.new(|cx| {
            SelectState::new(
                effort_labels,
                Some(IndexPath::new(EFFORTS.len() - 1)),
                window,
                cx,
            )
        });
        let file_editor = cx.new(|cx| EditorState::new(window, cx).line_number(true));

        let subs = vec![
            // "System" follows macOS light/dark as it changes.
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance.mode == crate::theme::Mode::System {
                    crate::theme::apply(this.appearance, Some(window), cx);
                }
            }),
            cx.subscribe_in(&composer, window, |this, _, ev: &InputEvent, window, cx| {
                match ev {
                    InputEvent::PressEnter { shift: false, .. } => this.send(window, cx),
                    // New text: the command list starts over at its first row.
                    InputEvent::Change => {
                        this.slash_sel = 0;
                        // Esc hides the list for the current word; a new word may open it again.
                        let text = this.composer.read(cx).value();
                        if text.is_empty() || text.ends_with(char::is_whitespace) {
                            this.slash_closed = false;
                        }
                        cx.notify();
                    }
                    _ => {}
                }
            }),
            cx.subscribe_in(
                &search,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        let q = state.read(cx).value().trim().to_string();
                        this.search_sessions(q, window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &project_path,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let path = state.read(cx).value().trim().to_string();
                        if !path.is_empty() {
                            this.path_entry = false;
                            this.set_new_project(path, window, cx);
                        }
                    }
                },
            ),
            // An error shown after a failed save goes away once the text is edited.
            cx.subscribe_in(&mcp_editor, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) && this.mcp_error.is_some() {
                    this.mcp_error = None;
                    cx.notify();
                }
            }),
            cx.subscribe_in(
                &effort_choice,
                window,
                |this, _, ev: &SelectEvent<Vec<SharedString>>, _, cx| {
                    let SelectEvent::Confirm(Some(label)) = ev else {
                        return;
                    };
                    if let Some(i) = EFFORTS.iter().position(|e| *e == label.as_ref()) {
                        this.set_effort(i, cx);
                    }
                },
            ),
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
                    if this.live.is_some() {
                        this.speed.tick();
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
                    .background_spawn(async move { connection::host_check(&h) })
                    .await;
                // Only hide hosts that answered without Hermes; unreachable ones stay listed.
                if ok == connection::HostCheck::NoHermes {
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
            profile_models: vec![],
            catalog: vec![],
            slash_sel: 0,
            slash_closed: false,
            slash_scroll: ScrollHandle::new(),
            approval: None,
            approval_mode: None,
            profile_context: 0,
            places: HashMap::new(),
            inferred: HashMap::new(),
            project: None,
            default_cwd: None,
            resume_session: None,
            new_project: None,
            new_branch: false,
            new_project_repo: true,
            path_entry: false,
            project_path,
            git_armed: None,
            installs: HashMap::new(),
            appearance: crate::theme::Appearance::load(),
            speed: crate::speed::Meter::fresh(),
            show_list: std::fs::read_to_string(file_list_pref()).is_ok_and(|s| s.trim() == "1"),
            search,
            search_query: String::new(),
            search_hits: None,
            search_seq: 0,
            model_pick: None,
            model_pending: false,
            effort_pending: false,
            panel: Panel::Chat,
            skills: vec![],
            skill_open: None,
            cron_open: None,
            prompts: vec![],
            prompt_open: None,
            prompt_title,
            prompt_editor,
            prompt_delete_armed: false,
            cron_editor,
            cron_schedule,
            skill_editor,
            skill_delete_armed: false,
            config_editor,
            config_error: None,
            cron: vec![],
            effort_choice,
            mcp_editor,
            mcp_status: "".into(),
            mcp_error: None,
            composer,
            chat_scroll: ScrollHandle::new(),
            follow: true,
            last_max: px(0.),
            open_steps: HashSet::new(),
            open_source: None,
            right: None,
            file: None,
            file_editor,
            tree_root: None,
            tree: HashMap::new(),
            tree_open: HashSet::new(),
            wave_ticker: None,
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

    pub(crate) fn refresh_sessions(&self) {
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
                // Back where the app was closed: that profile, then that session.
                let (last_profile, last_session) = self.last_place();
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
                if self.profile.as_deref() == Some(&p) {
                    let unknown_tg = rows
                        .iter()
                        .any(|s| s.source == "telegram" && !self.tg_owner.contains_key(&s.id));
                    self.sessions = rows;
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
            }
            Evt::SessionsChanged => self.refresh_sessions(),
            Evt::Deleted => self.refresh_sessions(),
            Evt::DeleteFailed(e) => {
                let msg = if e.contains("active") {
                    "The session is open in another client (Hermes.app, Telegram) — close it there"
                        .to_string()
                } else {
                    format!("Could not delete: {e}")
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
                    self.restore_reviews();
                    self.live = running.then(|| Live {
                        started: Instant::now(),
                        label: "Working…".into(),
                    });
                    self.reset_scroller(cx);
                    self.resolve_project(window, cx);
                }
            }
            Evt::Created { session, runtime } => {
                self.opened.insert(session.clone(), runtime.clone());
                self.session = Some(session);
                self.save_place();
                self.runtime = Some(runtime.clone());
                if let (Some(text), Some(profile)) = (self.queued.take(), self.profile.clone()) {
                    self.apply_model_pick(&profile, &runtime);
                    let effort = self.take_effort();
                    match self.parse_command(&text) {
                        Some((name, arg)) => self.cmd(Cmd::Dispatch {
                            profile,
                            session: runtime,
                            name,
                            arg,
                        }),
                        None => self.cmd(Cmd::Send {
                            profile,
                            session: runtime,
                            text,
                            effort,
                        }),
                    }
                }
                // Show the new session in the list right away.
                self.refresh_sessions();
            }
            Evt::Started(sid) if self.is_current(&sid) => {
                if self.live.is_none() {
                    self.live = Some(Live {
                        started: Instant::now(),
                        label: "Thinking…".into(),
                    });
                }
            }
            Evt::Delta(sid, text) if self.is_current(&sid) => {
                self.speed.token(text.chars().count());
                if let Some(turn) = self.turns.last_mut() {
                    turn.answer.push_str(&text);
                }
                if let Some(live) = self.live.as_mut() {
                    live.label = "Writing…".into();
                }
                self.remeasure_last(cx);
            }
            Evt::Reasoning(sid, text) if self.is_current(&sid) => {
                self.speed.token(text.chars().count());
                if let Some(turn) = self.turns.last_mut() {
                    match turn.steps.last_mut() {
                        Some(Step::Thought(t)) => t.push_str(&text),
                        _ => turn.steps.push(Step::Thought(text)),
                    }
                }
                if let Some(live) = self.live.as_mut() {
                    live.label = "Thinking…".into();
                }
                self.remeasure_last(cx);
            }
            Evt::ToolStart {
                session,
                name,
                detail,
                urls,
                call_id,
                args,
            } if self.is_current(&session) => {
                self.speed.tool_call();
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
                        call_id,
                        args,
                        output: String::new(),
                    });
                }
                self.remeasure_last(cx);
            }
            Evt::ToolDone {
                session,
                failed,
                call_id,
                output,
                edit,
            } if self.is_current(&session) => {
                self.speed.request();
                if let Some(turn) = self.turns.last_mut() {
                    let step = turn.steps.iter_mut().rev().find(|s| match s {
                        Step::Tool { call_id: id, .. } => call_id.is_empty() || *id == call_id,
                        _ => false,
                    });
                    if let Some(Step::Tool {
                        failed: f,
                        output: o,
                        ..
                    }) = step
                    {
                        *f = failed;
                        *o = output;
                    }
                    if let Some(edit) = edit {
                        turn.files.push(edit);
                        if self.project.is_none() {
                            self.resolve_project(window, cx);
                        }
                    }
                }
                cx.notify();
            }
            Evt::Catalog(list) => self.catalog = list,
            Evt::Review(sid, text) if self.is_current(&sid) => {
                if let Some(i) = self.turns.len().checked_sub(1) {
                    self.turns[i].reviews.push(text.clone());
                    self.remember_review(i, text);
                }
                self.remeasure_last(cx);
            }
            Evt::Dispatched {
                session,
                output,
                send,
            } if self.is_current(&session) => match send {
                // The command expanded into a prompt (skills, /goal, …): run it as a turn.
                Some(message) => {
                    if let Some(profile) = self.profile.clone() {
                        self.speed.request();
                        self.cmd(Cmd::Send {
                            profile,
                            session,
                            text: message,
                            effort: None,
                        });
                    }
                }
                None => {
                    self.live = None;
                    if let Some(turn) = self.turns.last_mut() {
                        turn.answer = output.unwrap_or_else(|| "Done.".into());
                        turn.finished = Some(now_secs());
                    }
                    self.remeasure_last(cx);
                }
            },
            Evt::Approval(req) if self.is_current(&req.session) => {
                if let Some(live) = self.live.as_mut() {
                    live.label = "Waiting for your approval…".into();
                }
                self.approval = Some(req);
                self.remeasure_last(cx);
            }
            Evt::ApprovalDone(sid) => {
                if self.approval.as_ref().is_some_and(|a| a.session == sid) {
                    self.approval = None;
                }
            }
            Evt::Subagent {
                session,
                id,
                kind,
                text,
                status,
            } if self.is_current(&session) => {
                if let Some(turn) = self.turns.last_mut() {
                    let pos = turn
                        .steps
                        .iter()
                        .position(|s| matches!(s, Step::Subagent { id: x, .. } if *x == id));
                    let k = pos.unwrap_or_else(|| {
                        turn.steps.push(Step::Subagent {
                            id: id.clone(),
                            goal: String::new(),
                            status: "running".into(),
                            lines: vec![],
                            summary: String::new(),
                        });
                        turn.steps.len() - 1
                    });
                    if let Step::Subagent {
                        goal,
                        status: st,
                        lines,
                        summary,
                        ..
                    } = &mut turn.steps[k]
                    {
                        match kind {
                            "start" => *goal = clip_line(&text),
                            "tool" => lines.push(text),
                            "text" if !text.trim().is_empty() => lines.push(clip_line(&text)),
                            "complete" => {
                                *st = if status.is_empty() {
                                    "completed".into()
                                } else {
                                    status
                                };
                                *summary = text;
                            }
                            _ => {}
                        }
                        if goal.is_empty() {
                            *goal = "task".into();
                        }
                    }
                    if let Some(live) = self.live.as_mut() {
                        live.label = "Subagent working…".into();
                    }
                }
                self.remeasure_last(cx);
            }
            Evt::Usage(sid, usage) if self.is_current(&sid) => {
                self.speed.usage(&usage);
                self.usage = Some(usage);
            }
            Evt::Complete { session, text } if self.is_current(&session) => {
                self.live = None;
                self.speed.finish();
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
        self.tree_root = None;
        self.tree.clear();
        self.session = None;
        self.runtime = None;
        self.turns.clear();
        self.live = None;
        self.queued = None;
        self.usage = None;
        self.playing = None;
        self.model_pick = None;
        self.model_pending = false;
        // Speeds describe the model and server, not the chat: keep them, restart the counters.
        self.speed.restart();
        self.project = None;
        self.new_project = None;
        self.new_branch = false;
        self.git_armed = None;
        self.reset_scroller(cx);
    }

    fn reset_scroller(&mut self, cx: &mut Context<Self>) {
        self.follow = true;
        self.open_steps.clear();
        self.open_source = None;
        cx.notify();
    }

    fn remeasure_last(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.follow = true;
        self.chat_scroll.scroll_to_bottom();
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
        self.save_place();
        self.cmd(Cmd::Open {
            profile,
            session: id,
        });
        cx.notify();
    }

    pub fn new_session(&mut self, cx: &mut Context<Self>) {
        self.panel = Panel::Chat;
        self.reset_chat(cx);
        self.save_place();
        cx.notify();
    }

    /// First click arms, second click deletes.
    pub fn delete_session(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
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
        let rid = id.clone();
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

    fn rest_branch(
        &mut self,
        conn: Arc<Connection>,
        profile: String,
        root: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p = profile.clone();
        let target = self.target.clone();
        self.background(
            window,
            cx,
            move || {
                let _ = connection::exclude_worktrees(&target, &root);
                rest::worktree_add(&conn, &p, &root, &name)
            },
            move |this, r, window, cx| match r {
                Ok((folder, _branch)) => this.cmd(Cmd::Create {
                    profile,
                    cwd: Some(folder),
                }),
                Err(e) => {
                    this.live = None;
                    this.queued = None;
                    this.toast(
                        NotificationType::Error,
                        format!("New branch: {e:#}"),
                        window,
                        cx,
                    );
                }
            },
        );
    }

    /// Pick the model for a new session (primary or a fallback); applied once it is created.
    pub fn set_model(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.session.is_some() {
            return;
        }
        self.model_pick = Some(i);
        self.model_pending = true;
        cx.notify();
    }

    /// Debounced: only the last query typed within 300 ms goes to the gateway.
    fn search_sessions(&mut self, q: String, window: &mut Window, cx: &mut Context<Self>) {
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

    fn apply_model_pick(&mut self, profile: &str, session: &str) {
        if !std::mem::take(&mut self.model_pending) {
            return;
        }
        let Some((model, provider)) = self
            .model_pick
            .and_then(|i| self.profile_models.get(i))
            .cloned()
        else {
            return;
        };
        if let Some(u) = self.usage.as_mut() {
            u.model = model.clone();
        }
        self.cmd(Cmd::SetModel {
            profile: profile.to_string(),
            session: session.to_string(),
            model,
            provider,
        });
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
        // Enter in the command list picks the command instead of sending.
        if self.complete_slash(window, cx) {
            return;
        }
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.turns.push(Turn {
            user: text.clone(),
            started: Some(now_secs()),
            ..Default::default()
        });
        self.follow = true;
        self.speed.begin_answer();
        self.speed.request();
        self.live = Some(Live {
            started: Instant::now(),
            label: "Sending…".into(),
        });
        match self.runtime.clone() {
            Some(session) => match self.parse_command(&text) {
                Some((name, arg)) => self.cmd(Cmd::Dispatch {
                    profile,
                    session,
                    name,
                    arg,
                }),
                None => {
                    let effort = self.take_effort();
                    self.cmd(Cmd::Send {
                        profile,
                        session,
                        text,
                        effort,
                    });
                }
            },
            None => {
                // A new session is created on the first message; the effort goes with it.
                self.effort_pending = true;
                self.queued = Some(text.clone());
                match (self.new_project.clone(), self.new_branch) {
                    (Some(root), true) => {
                        // Its own branch: a fresh folder named after the request.
                        let Some(conn) = self.conn.clone() else {
                            return;
                        };
                        let name = branch_name(&text);
                        self.rest_branch(conn, profile, root, name, window, cx);
                    }
                    (root, _) => self.cmd(Cmd::Create { profile, cwd: root }),
                }
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
            label: "Regenerating…".into(),
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
            self.toast(NotificationType::Success, "Copied", window, cx);
        }
    }

    /// `hermes gateway restart` on the machine we are connected to, then reconnect.
    pub fn restart_gateway(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        self.toast(
            NotificationType::Info,
            "Restarting the gateway…",
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let t = target.clone();
            let result = cx
                .background_spawn(async move { connection::restart_gateway(&t) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(()) => {
                    this.toast(NotificationType::Success, "Gateway restarted", window, cx);
                    this.connect(target, window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Restart: {e:#}"),
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
            cx.notify();
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
        self.rest(window, cx, rest::profile_defaults, |this, r, window, cx| {
            let Ok((effort, model, models, approval, context)) = r else {
                return;
            };
            this.approval_mode = Some(approval);
            this.profile_context = context;
            this.profile_models = models;
            this.model_pick = None;
            if let Some(i) = effort.and_then(|v| EFFORTS.iter().position(|e| *e == v)) {
                this.effort = i;
                this.effort_pending = false;
                this.effort_choice.update(cx, |s, cx| {
                    s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                });
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

    // ---- right panel: changed files, tree, viewer --------------------------

    /// Files this session edited, in first-touch order, with their diffs in order.
    pub fn session_files(&self) -> Vec<(String, Vec<String>)> {
        let mut out: Vec<(String, Vec<String>)> = vec![];
        for edit in self.turns.iter().flat_map(|t| &t.files) {
            let slot = match out.iter_mut().position(|(p, _)| *p == edit.path) {
                Some(i) => &mut out[i].1,
                None => {
                    out.push((edit.path.clone(), vec![]));
                    &mut out.last_mut().unwrap().1
                }
            };
            if let Some(d) = &edit.diff {
                slot.push(d.clone());
            }
        }
        out
    }

    /// Switch the right panel's tab (opening it if closed).
    pub fn toggle_right_tab(&mut self, tab: RightTab, window: &mut Window, cx: &mut Context<Self>) {
        self.right = Some(tab);
        if tab == RightTab::Files && self.tree_root.is_none() {
            self.load_tree_root(window, cx);
        }
        cx.notify();
    }

    pub fn toggle_right(&mut self, tab: RightTab, window: &mut Window, cx: &mut Context<Self>) {
        self.right = if self.right == Some(tab) {
            None
        } else {
            Some(tab)
        };
        if self.right == Some(RightTab::Files) && self.tree_root.is_none() {
            self.load_tree_root(window, cx);
        }
        cx.notify();
    }

    /// Tree root: the chat's project, else the common folder of the changed files, else the
    /// profile's working dir.
    pub(super) fn load_tree_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let own = self
            .project
            .as_ref()
            .map(|p| p.dir.clone())
            .or_else(|| self.session.as_ref().and_then(|s| self.folder_of(s)));
        if let Some(dir) = own {
            self.set_tree_root(dir, window, cx);
            return;
        }
        let files: Vec<String> = self.session_files().into_iter().map(|(p, _)| p).collect();
        if let Some(root) = common_dir(&files) {
            self.set_tree_root(root, window, cx);
            return;
        }
        self.rest(window, cx, rest::default_cwd, |this, r, window, cx| {
            if let Ok(cwd) = r {
                if !cwd.is_empty() {
                    this.set_tree_root(cwd, window, cx);
                }
            }
        });
    }

    pub fn set_tree_root(&mut self, root: String, window: &mut Window, cx: &mut Context<Self>) {
        self.tree_root = Some(root.clone());
        self.tree.clear();
        self.tree_open.clear();
        self.open_dir(root, window, cx);
    }

    pub fn open_dir(&mut self, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_open.remove(&dir) && self.tree_root.as_deref() != Some(&dir) {
            cx.notify();
            return;
        }
        self.tree_open.insert(dir.clone());
        if self.tree.contains_key(&dir) {
            cx.notify();
            return;
        }
        let d = dir.clone();
        self.rest(
            window,
            cx,
            move |c, p| rest::list_dir(c, p, &d),
            move |this, r, window, cx| match r {
                Ok(entries) => {
                    this.tree.insert(dir, entries);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Folder: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Open a file in the viewer; `diffs` empty means a plain file from the tree.
    pub fn open_file(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let diffs = self
            .session_files()
            .into_iter()
            .find(|(p, _)| *p == path)
            .map(|(_, d)| d)
            .unwrap_or_default();
        let mode = if diffs.is_empty() {
            FileMode::After
        } else {
            FileMode::Diff
        };
        self.file = Some(FileView {
            path: path.clone(),
            mode,
            diffs,
            loaded: false,
            truncated: false,
            missing: false,
            diff_view: None,
            before_view: None,
        });
        if let Some(view) = self.file.as_mut() {
            if !view.diffs.is_empty() {
                let files = DiffFile::parse(&crate::diff::joined(&view.diffs)).unwrap_or_default();
                view.diff_view = Some(diff_state(files, cx));
            }
        }
        if self.right.is_none() {
            self.right = Some(RightTab::Changes);
        }
        let language = language_for(&path);
        self.file_editor.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_value("", window, cx);
        });
        let p = path.clone();
        self.rest(
            window,
            cx,
            move |c, prof| rest::read_text(c, prof, &p),
            move |this, r, window, cx| {
                let Some(view) = this.file.as_mut().filter(|f| f.path == path) else {
                    return;
                };
                match r {
                    Ok(file) if file.binary => view.missing = true,
                    Ok(file) => {
                        let before = (!view.diffs.is_empty() && !file.truncated)
                            .then(|| crate::diff::before(&file.text, &view.diffs))
                            .flatten();
                        if let Some(before) = before {
                            let full = crate::diff::full(&view.path, &before, &file.text);
                            if let Ok(files) = DiffFile::parse(&full) {
                                if !files.is_empty() {
                                    view.diff_view = Some(diff_state(files, cx));
                                }
                            }
                            let original = DiffFile::unchanged(view.path.clone(), &before);
                            view.before_view = Some(diff_state(vec![original], cx));
                        }
                        view.truncated = file.truncated;
                        view.loaded = true;
                        this.file_editor
                            .update(cx, |s, cx| s.set_value(file.text, window, cx));
                    }
                    // Deleted or moved since (old sessions): no error, just the recorded edits.
                    Err(_) => {
                        view.missing = true;
                        if view.diff_view.is_none() {
                            view.mode = FileMode::After;
                        }
                    }
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    pub fn set_appearance(
        &mut self,
        a: crate::theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.appearance = a;
        a.save();
        crate::theme::apply(a, Some(window), cx);
        cx.notify();
    }

    pub fn answer_approval(&mut self, choice: String, cx: &mut Context<Self>) {
        let (Some(req), Some(profile)) = (self.approval.take(), self.profile.clone()) else {
            return;
        };
        if let Some(live) = self.live.as_mut() {
            live.label = "Working…".into();
        }
        self.cmd(Cmd::Approve {
            profile,
            req,
            choice,
        });
        cx.notify();
    }

    pub fn set_approval_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.approval_mode = Some(mode.to_string());
        self.cmd(Cmd::SetApproval {
            profile,
            mode: mode.to_string(),
        });
        cx.notify();
    }

    /// Hermes sends review notes only live; keep them per chat (turn index → notes) to show again.
    fn reviews_path(&self) -> std::path::PathBuf {
        connection::state_dir().join("reviews.json")
    }

    fn remember_review(&self, turn: usize, text: String) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let path = self.reviews_path();
        let mut all: HashMap<String, HashMap<usize, Vec<String>>> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        all.entry(session)
            .or_default()
            .entry(turn)
            .or_default()
            .push(text);
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path, serde_json::to_vec(&all).unwrap_or_default());
    }

    fn restore_reviews(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let all: HashMap<String, HashMap<usize, Vec<String>>> = std::fs::read(self.reviews_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for (i, notes) in all.get(&session).cloned().unwrap_or_default() {
            if let Some(t) = self.turns.get_mut(i) {
                t.reviews = notes;
            }
        }
    }

    /// Where this target was last: (profile, open session).
    fn last_place(&self) -> (Option<String>, Option<String>) {
        let all: HashMap<String, (Option<String>, Option<String>)> =
            std::fs::read(connection::state_dir().join("last-place.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
        all.get(&self.target.label()).cloned().unwrap_or_default()
    }

    /// Remember the profile and session for this target (called whenever either changes).
    fn save_place(&self) {
        let path = connection::state_dir().join("last-place.json");
        let mut all: HashMap<String, (Option<String>, Option<String>)> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        all.insert(
            self.target.label(),
            (self.profile.clone(), self.session.clone()),
        );
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path, serde_json::to_vec(&all).unwrap_or_default());
    }

    pub fn toggle_file_list(&mut self, cx: &mut Context<Self>) {
        self.show_list = !self.show_list;
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(file_list_pref(), if self.show_list { "1" } else { "0" });
        cx.notify();
    }

    pub fn set_file_mode(&mut self, mode: FileMode, cx: &mut Context<Self>) {
        if let Some(f) = self.file.as_mut() {
            f.mode = mode;
        }
        cx.notify();
    }

    pub fn save_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.file.as_ref() else {
            return;
        };
        if !view.loaded || view.truncated {
            self.toast(
                NotificationType::Warning,
                "This file was loaded partially — saving is disabled",
                window,
                cx,
            );
            return;
        }
        let path = view.path.clone();
        let text = self.file_editor.read(cx).value().to_string();
        self.rest(
            window,
            cx,
            move |c, p| rest::write_text(c, p, &path, &text),
            |this, r, window, cx| match r {
                Ok(()) => this.toast(NotificationType::Success, "Saved", window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Not saved: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Open the file in Zed: locally by path, over SSH as ssh://host/path.
    /// The open file in Zed, inside its project when there is one (so Zed opens the whole repo).
    pub fn open_in_zed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.file.as_ref().map(|f| f.path.clone()) else {
            return;
        };
        let mut paths: Vec<String> = self
            .project
            .as_ref()
            .filter(|p| path.starts_with(&format!("{}/", p.dir)))
            .map(|p| vec![p.dir.clone()])
            .unwrap_or_default();
        paths.push(path);
        self.zed(paths, window, cx);
    }

    pub fn open_project_in_zed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) {
            self.zed(vec![dir], window, cx);
        }
    }

    fn zed(&mut self, paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let targets: Vec<String> = paths
            .into_iter()
            .map(|p| match &self.target {
                Target::Local => p,
                Target::Ssh(host) => format!("ssh://{host}{p}"),
            })
            .collect();
        let zed = ["/usr/local/bin/zed", "/opt/homebrew/bin/zed"]
            .into_iter()
            .find(|p| std::path::Path::new(p).exists());
        let result = match (zed, &self.target) {
            (Some(cli), _) => std::process::Command::new(cli)
                .args(&targets)
                .spawn()
                .map(|_| ()),
            (None, Target::Local) => std::process::Command::new("open")
                .args(["-a", "Zed"])
                .args(&targets)
                .spawn()
                .map(|_| ()),
            (None, Target::Ssh(_)) => Err(std::io::Error::other(
                "the `zed` CLI is not installed (Zed → Install CLI)",
            )),
        };
        if let Err(e) = result {
            self.toast(NotificationType::Error, format!("Zed: {e}"), window, cx);
        }
    }

    pub fn toggle_step(&mut self, turn: usize, step: usize, cx: &mut Context<Self>) {
        if !self.open_steps.remove(&(turn, step)) {
            self.open_steps.insert((turn, step));
        }
        cx.notify();
    }

    pub fn toggle_source(&mut self, turn: usize, host: String, cx: &mut Context<Self>) {
        self.open_source = match &self.open_source {
            Some((t, h)) if *t == turn && *h == host => None,
            _ => Some((turn, host)),
        };
        cx.notify();
    }

    // ---- panels ----------------------------------------------------------

    pub fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = panel;
        match panel {
            Panel::Chat | Panel::Telegram | Panel::Appearance => {}
            Panel::Prompts => self.load_prompts(window, cx),
            Panel::Gateways => self.check_installs(window, cx),
            Panel::Config => {
                self.config_error = None;
                self.rest(
                    window,
                    cx,
                    rest::config_raw,
                    |this, r, window, cx| match r {
                        Ok(text) => this
                            .config_editor
                            .update(cx, |s, cx| s.set_value(text, window, cx)),
                        Err(e) => this.toast(
                            NotificationType::Error,
                            format!("Config: {e:#}"),
                            window,
                            cx,
                        ),
                    },
                )
            }
            Panel::Cron => self.load_cron(window, cx),
            Panel::Skills => self.rest(window, cx, rest::skills, |this, r, window, cx| match r {
                Ok(list) => this.skills = list,
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Skills: {e:#}"),
                    window,
                    cx,
                ),
            }),
            Panel::Mcp => {
                self.mcp_status = "Loading…".into();
                self.rest(window, cx, rest::mcp_json, |this, r, window, cx| match r {
                    Ok(text) => {
                        this.mcp_editor
                            .update(cx, |s, cx| s.set_value(text, window, cx));
                        this.mcp_status = "".into();
                        this.mcp_error = None;
                    }
                    Err(e) => this.mcp_status = format!("Could not load: {e:#}").into(),
                })
            }
        }
        cx.notify();
    }

    pub fn open_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.skill_open = Some(name.clone());
        self.skill_delete_armed = false;
        self.skill_editor
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.rest(
            window,
            cx,
            move |c, p| rest::skill_content(c, p, &name),
            |this, r, window, cx| match r {
                Ok(text) => this
                    .skill_editor
                    .update(cx, |s, cx| s.set_value(text, window, cx)),
                Err(e) => this.toast(NotificationType::Error, format!("Skill: {e:#}"), window, cx),
            },
        );
        cx.notify();
    }

    /// First press arms, second deletes the skill's folder.
    pub fn delete_skill(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.skill_open.clone() else {
            return;
        };
        if !self.skill_delete_armed {
            self.skill_delete_armed = true;
            cx.notify();
            return;
        }
        self.skill_delete_armed = false;
        let n = name.clone();
        self.rest(
            window,
            cx,
            move |c, p| rest::delete_skill(c, p, &n),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    this.skills.retain(|s| s.name != name);
                    this.skill_open = None;
                    this.toast(
                        NotificationType::Success,
                        format!("Skill {name} deleted"),
                        window,
                        cx,
                    );
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Skill not deleted: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    pub fn save_skill(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.skill_open.clone() else {
            return;
        };
        let text = self.skill_editor.read(cx).value().to_string();
        self.rest(
            window,
            cx,
            move |c, p| rest::save_skill(c, p, &name, &text),
            |this, r, window, cx| match r {
                Ok(()) => this.toast(NotificationType::Success, "Skill saved", window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Skill not saved: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// YAML is parsed before saving; a broken document is not sent.
    pub fn save_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.config_editor.read(cx).value().to_string();
        self.config_error = match serde_yaml::from_str::<serde_yaml::Value>(&text) {
            Ok(serde_yaml::Value::Mapping(_)) => None,
            Ok(_) => Some("config.yaml must be a YAML mapping".into()),
            Err(e) => Some(
                match e.location() {
                    Some(l) => format!("Line {}, column {}: {e}", l.line(), l.column()),
                    None => e.to_string(),
                }
                .into(),
            ),
        };
        cx.notify();
        if self.config_error.is_some() {
            return;
        }
        self.rest(
            window,
            cx,
            move |c, p| rest::save_config_raw(c, p, &text),
            |this, r, window, cx| match r {
                Ok(()) => this.toast(NotificationType::Success, "config.yaml saved", window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Config not saved: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// JSON check run on save; an invalid document is not sent.
    fn validate_mcp(&mut self, cx: &mut Context<Self>) {
        let text = self.mcp_editor.read(cx).value().to_string();
        self.mcp_error = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) if v.is_object() => None,
            Ok(_) => Some("Expected an object: { \"server-name\": { … } }".into()),
            Err(e) => Some(format!("Line {}, column {}: {e}", e.line(), e.column()).into()),
        };
        cx.notify();
    }

    pub fn save_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.validate_mcp(cx);
        if self.mcp_error.is_some() {
            return;
        }
        let text = self.mcp_editor.read(cx).value().to_string();
        self.mcp_status = "Saving…".into();
        self.rest(
            window,
            cx,
            move |c, p| rest::save_mcp_json(c, p, &text),
            |this, r, window, cx| {
                this.mcp_status = "".into();
                match r {
                    Ok(()) => this.toast(
                        NotificationType::Success,
                        "MCP servers saved. New sessions pick them up.",
                        window,
                        cx,
                    ),
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("MCP not saved: {e:#}"),
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
                                    "Nothing recognized",
                                    window,
                                    cx,
                                ),
                                Err(e) => this.toast(
                                    NotificationType::Error,
                                    format!("Transcription: {e:#}"),
                                    window,
                                    cx,
                                ),
                            }
                        },
                    );
                }
                Err(e) => self.toast(
                    NotificationType::Warning,
                    format!("Recording: {e:#}"),
                    window,
                    cx,
                ),
            }
        } else {
            match Recorder::start() {
                Ok(r) => {
                    self.recorder = Some(r);
                    self.wave_ticker = Some(cx.spawn(async move |this, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(33))
                                .await;
                            let alive = this.update(cx, |this, cx| {
                                cx.notify();
                                this.recorder.is_some()
                            });
                            if !matches!(alive, Ok(true)) {
                                break;
                            }
                        }
                    }));
                }
                Err(e) => self.toast(
                    NotificationType::Error,
                    format!("Microphone: {e:#}"),
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
                        format!("Text-to-speech: {e:#}"),
                        window,
                        cx,
                    ),
                }
            },
        );
        cx.notify();
    }
}

/// The deepest folder containing every path.
fn common_dir(paths: &[String]) -> Option<String> {
    let first = std::path::Path::new(paths.first()?).parent()?.to_path_buf();
    let mut root = first;
    for p in &paths[1..] {
        while !std::path::Path::new(p).starts_with(&root) {
            root = root.parent()?.to_path_buf();
        }
    }
    Some(root.to_string_lossy().to_string())
}

/// Tree-sitter language for a file name (falls back to plain text).
pub fn language_for(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "sh" | "bash" | "zsh" => "bash",
        "go" => "go",
        "php" => "php",
        "rb" => "ruby",
        "swift" => "swift",
        "java" => "java",
        "kt" => "kotlin",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "lua" => "lua",
        _ => "text",
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

/// A diff view state: three lines of context around each change, the rest folded (expandable).
fn diff_state(files: Vec<DiffFile>, cx: &mut App) -> Entity<DiffState> {
    cx.new(|cx| DiffState::new(files, cx).with_context_lines(Some(3)))
}

fn file_list_pref() -> std::path::PathBuf {
    connection::state_dir().join("file-list")
}

/// One line of at most 160 characters.
fn clip_line(s: &str) -> String {
    let line = s
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .trim();
    if line.chars().count() > 160 {
        format!("{}…", line.chars().take(159).collect::<String>())
    } else {
        line.to_string()
    }
}

/// A branch/folder name from the request: its first Latin words ("fix login bug"), else a short
/// unique "chat-1a2b" (Hermes keeps only ASCII in the slug).
fn branch_name(text: &str) -> String {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|w| w.len() > 1)
        .take(5)
        .collect();
    if words.is_empty() {
        format!("chat-{}", &uuid::Uuid::new_v4().simple().to_string()[..4])
    } else {
        words.join(" ").to_lowercase()
    }
}
