//! Application state, gateway events and user actions. Rendering lives in `ui/`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use web_time::Instant;

use gpui_kit::component::IndexPath;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::*;

mod asks;
mod attachments;
pub(crate) mod inserts;
mod keys;
pub(crate) mod local_files;
pub mod marks;
mod processes;
pub(crate) mod project_env;
pub(crate) mod project_list;
pub(crate) mod projects;
pub(crate) mod remotes;
pub use project_env::ProjectEnv;
pub use project_list::ProjectForm;
mod prompts;
pub(crate) mod providers;
pub(crate) mod remote_files;
pub(crate) mod servers;
pub(crate) mod sites;
mod slash;
pub(crate) mod subjects;
pub(crate) mod tasks;
pub(crate) mod tools;

use crate::attach;
use crate::audio::{Playback, Recorder};
use crate::connection::{self, Connection};
use crate::gateway::{Cmd, Evt, Gateway};
use crate::history;
use crate::media::{self, Loaded};
use crate::model::{EFFORTS, Profile, SessionRow, SkillRow, Step, Target, Turn, Usage};
use crate::rest;

pub type Choice = SelectState<Vec<SharedString>>;

/// This machine's own project (the agent's general environment): installing, building, looking
/// after the machine. A server's is a project named after it ("Dev"); Host is first in the sidebar.
pub const LOCAL_HOST: &str = "Host";

#[derive(Clone, Copy, PartialEq)]
pub enum Panel {
    Chat,
    /// Model providers, fallback, voice, service keys.
    Providers,
    /// A new project: a page of its own, not a thread (a project is not a conversation).
    NewProject,
}

impl Panel {
    pub fn is_settings(self) -> bool {
        self != Panel::Chat
    }
}

/// The instant a turn began, from its Unix start time (now when unknown or in the future).
fn live_since(started: Option<f64>) -> Instant {
    started
        .map(|t| (now_secs() - t).max(0.))
        .and_then(|ago| Instant::now().checked_sub(Duration::from_secs_f64(ago)))
        .unwrap_or_else(Instant::now)
}

pub struct Live {
    pub started: Instant,
    pub label: SharedString,
}

pub struct BenchApp {
    pub targets: Vec<Target>,
    pub target: Target,
    /// The gateway list as a Select (kept for its subscription; the status bar has its own menu).
    _target_choice: Entity<Choice>,
    gateway: Option<Gateway>,
    pub conn: Option<Arc<Connection>>,
    /// The subject of the chat about to be created ("tasks:today"): it goes with session.create.
    pub new_subject: String,
    /// Each thread's inserts (by session id; "draft" for a new chat): `app::inserts`.
    pub inserts: HashMap<String, Vec<crate::model::Insert>>,
    /// A new chat's session is being made (an insert came before any message).
    pub creating: bool,
    /// Inserts put while a message still waited in the composer: they belong after that message,
    /// whose place in the thread is not there yet. Their ids; `send` fixes the place as it sends.
    pub insert_draft: Vec<String>,
    /// The insert a form was opened from (a new server, a new host): it shows under it.
    pub form_after: Option<String>,
    /// Of the inserts whose state is app-wide (tasks, tools, files), the live one per kind.
    pub insert_live: HashMap<String, String>,
    /// The open thread's messages waiting for the agent: (id, mode, text).
    pub queue: Vec<(String, String, String)>,
    /// Which thread the next opening of a subject goes to: unset = its latest, `Some(None)` = a
    /// new one (the row's "+"), `Some(Some(id))` = that thread (a row under the subject).
    pub subject_target: Option<Option<String>>,
    /// The project the open thread belongs to (a new one is created in it); "" = none.
    pub thread_project: String,
    /// The person's projects: the sidebar.
    pub projects: Vec<crate::model::ProjectEntry>,
    /// The machine's own agent (Host) has the watchable server browser.
    pub host_browser: bool,
    /// The new project's form, in the chat area while open (the sidebar's "+").
    pub project_form: Option<ProjectForm>,
    /// The open environment thread's project environment (its block).
    pub project_env: Option<ProjectEnv>,
    /// ~/.ssh/config servers that belong to a project: alias → project id (the rest are shared).
    pub ssh_projects: HashMap<String, String>,
    /// The project whose prompts `prompts` holds (they follow the open thread's project).
    pub prompts_for: Option<String>,
    /// The agents of remote projects, by server.
    pub remotes: HashMap<String, remotes::RemoteAgent>,
    /// BENCH_SUBJECT was opened (once).
    pub subject_opened: bool,
    /// Sites on the server's browser: each turn's tab, live or as its last frame.
    pub sites: sites::Sites,
    pub sites_error: Option<String>,
    /// A step's tab being reopened: (thread, turn, url, the tabs before).
    /// A click in the thread landed in something that types (a terminal, an editor): it keeps the focus.
    pub click_claimed: bool,
    /// Where the left button went down in the thread (a drag selects text: no focus move).
    pub click_at: Option<Point<Pixels>>,
    /// The composer shows a new chat (no placeholder) — what render last set.
    pub composer_fresh: bool,
    evt_tx: async_channel::Sender<Evt>,
    pub connecting: bool,

    pub profiles: Vec<Profile>,
    pub profile: Option<String>,
    pub profile_choice: Entity<Choice>,
    pub sessions: Vec<SessionRow>,
    /// Stored id of the open session (what the session list shows).
    pub session: Option<String>,
    /// The id the gateway uses for prompts and events of the open session.
    pub runtime: Option<String>,
    pub confirm_delete: Option<String>,
    /// Stored id -> runtime id for sessions this client has opened (to close before delete).
    opened: HashMap<String, String>,
    /// Telegram: session id -> owner user id; known people; whose sessions to show (None = all).
    tg_owner: HashMap<String, String>,
    pub tg_people: Vec<(String, String)>,
    pub tg_visible: Option<HashSet<String>>,

    pub turns: Vec<Turn>,
    /// Each thread opened in this run, as last shown: switching back shows it at once while
    /// its history comes again.
    pub thread_cache: HashMap<String, Vec<Turn>>,
    /// Frames left to keep a thread that is opening invisible, and how many in a row its layout
    /// stayed the same; it shows once settled (`ui::chat`), so it appears whole, not in jumps.
    pub settle: u8,
    pub settle_stable: u8,
    /// The opening thread has content to lay out (cached or its history arrived).
    pub settle_ready: bool,
    /// The open thread's history could not be read: why (shown with Retry instead of "Loading").
    pub history_error: Option<String>,
    /// `BENCH_DEMO_DRAFT` was put into the composer.
    pub demo_draft_done: bool,
    pub live: Option<Live>,
    queued: Option<String>,
    /// Send was pressed while dictating: send once the speech is recognized.
    pub send_after_transcribe: bool,
    /// The "Changed files" list under the last turn is unfolded (folded by default).
    pub files_open: bool,
    /// The open session's background processes (`app/processes.rs`), the card's fold, and
    /// the process whose output is shown.
    pub processes: Vec<crate::model::BgProcess>,
    pub processes_open: bool,
    pub process_open: Option<String>,
    /// Files waiting in the composer to go with the next message.
    pub attachments: Vec<std::path::PathBuf>,
    /// The sidebar is folded away (⌘\, its top strip's toggle): one thread, the whole window.
    pub sidebar_hidden: bool,
    pub usage: Option<Usage>,
    pub effort: usize,
    /// The open session's effort when it is none of EFFORTS (set elsewhere, e.g. `high`; "" is the
    /// provider's default): shown as it is, nothing checked.
    pub effort_other: Option<String>,
    /// The profile's `agent.reasoning_effort`: where a new chat starts.
    profile_effort: usize,
    /// The profile's primary model and its fallbacks, as (model, provider).
    pub profile_models: Vec<rest::ModelChoice>,
    /// Settings → Providers and Messages.
    pub pv: providers::Providers,
    /// The gateway's state for the status bar's backend item (refreshed every 10 s):
    /// None before the first answer, Err when the gateway didn't answer.
    pub gw_status: Option<Result<rest::GatewayStatus, String>>,
    /// The last lines of the agent's log, from where it runs (its own status item shows them).
    pub gw_log: Vec<String>,
    /// Slash commands of the profile; the popup's highlighted row; Esc closed it for this text.
    pub catalog: Vec<crate::model::SlashCommand>,
    pub slash_sel: usize,
    pub slash_closed: bool,
    pub slash_scroll: ScrollHandle,
    /// The agent waits for an answer (approval mode manual/smart).
    pub approval: Option<crate::model::ApprovalReq>,
    /// A question the agent asks, the choices ticked so far (multi-select), the free answer.
    pub clarify: Option<crate::model::ClarifyReq>,
    pub clarify_picked: Vec<usize>,
    pub clarify_input: Entity<InputState>,
    /// The agent's plan for this chat, and whether its list is unfolded.
    pub todos: Vec<crate::model::Todo>,
    /// When the plan above began (unix seconds): the thread's row of the plan sits there
    /// instead of trailing the newest message. None = no plan shown.
    pub plan_at: Option<f64>,
    pub plan_open: bool,
    /// The profile's `approvals.mode`, once read.
    pub approval_mode: Option<String>,
    /// The profile's own approvals, effort and model have arrived: until then the status bar
    /// shows none of them (defaults there would only mislead).
    pub profile_ready: bool,
    /// A made-up session for screenshots (`BENCH_DEMO=1`, src/demo.rs): nothing is connected.
    pub demo: bool,
    /// The demo's threads by session id (opening one shows its turns; nothing is fetched).
    pub demo_threads: HashMap<String, Vec<Turn>>,
    /// The gateway refused a restart over running work: the next press (within a minute)
    /// restarts anyway.
    pub restart_armed: Option<Instant>,
    /// `model.context_length` of the profile (0 when not set).
    pub profile_context: u64,
    /// Colors and pins of sessions (this Mac).
    pub marks: HashMap<String, marks::Mark>,
    /// This window was opened for a given profile/session (else: where the app was last).
    start_place: Option<(Option<String>, Option<String>)>,
    /// Session folders/repos as the agent recorded them, and repos inferred from edited files.
    pub places: HashMap<String, crate::model::Place>,
    pub inferred: HashMap<String, String>,
    /// The open session's project (None for chats that touch no repo).
    pub project: Option<crate::model::Project>,
    /// The agent's default working folder (sessions started there have no folder of their own).
    pub default_cwd: Option<String>,
    /// Reopen this session once the list arrives (where the app was last closed).
    resume_session: Option<String>,
    /// For the next new chat: work in this folder.
    pub new_project: Option<String>,
    /// The chosen folder is a git repo (so "in a new branch" applies).
    pub new_project_repo: bool,
    /// Typing a folder path (SSH hosts have no native folder dialog here).
    pub path_entry: bool,
    pub project_path: Entity<InputState>,
    /// New chat: a group to put it in (`~/Research/<name>`) when it has no project.
    pub group_name: Entity<InputState>,
    pub appearance: crate::theme::Appearance,
    pub speed: crate::speed::Meter,
    /// Sidebar search: the field, what was typed, and the gateway's hits (None while searching).
    pub search: Entity<InputState>,
    pub search_query: String,
    pub search_hits: Option<Vec<(String, String)>>,
    search_seq: u64,
    effort_pending: bool,

    pub panel: Panel,
    pub skills: Vec<SkillRow>,
    pub skill_open: Option<String>,
    /// Settings → Prompts.
    pub prompts: Vec<crate::model::Prompt>,
    pub prompt_open: Option<usize>,
    pub effort_choice: Entity<Choice>,

    pub composer: Entity<TextareaState>,
    /// Unsent text per chat ("" = the new chat), kept on disk: switching chats or restarting
    /// keeps it. `draft_key` is the chat whose draft the composer shows.
    drafts: std::collections::HashMap<String, String>,
    draft_key: Option<String>,
    /// The chat column scrolls as one tall list (no virtualization: real heights, real bottom).
    pub chat_scroll: ScrollHandle,
    /// Keep the view pinned to the newest content until the user scrolls up.
    pub follow: bool,
    pub last_max: Pixels,
    /// Where the composer is in the window, and the window's height (last frame): the `/` list
    /// opens above the line when there is no room under it.
    pub composer_y: Pixels,
    /// The composer's measured height, from the same canvas as `composer_y`: how much of the
    /// window the thread's last row has to share with it (`past_end`).
    pub composer_h: f32,
    pub viewport_h: Pixels,
    /// Unfolded steps (turn, step) and the source host whose pages are listed (turn, host).
    pub open_steps: HashSet<(usize, usize)>,
    pub open_source: Option<(usize, String)>,
    pub recorder: Option<Recorder>,
    /// Repaints the waveform while recording.
    wave_ticker: Option<Task<()>>,
    pub transcribing: bool,
    pub playing: Option<(usize, Playback)>,
    pub speaking: Option<usize>,
    /// Files answers delivered, by target and path; diagrams, by source and dark theme.
    pub media: HashMap<String, Loaded<media::Fetched>>,
    pub diagrams: HashMap<(String, bool), Loaded<media::Diagram>>,
    /// The delivered audio file playing, by its media key, and since when.
    pub media_playing: Option<(String, Playback)>,
    pub media_since: Instant,
    /// The video playing in the chat, the one starting, and the loop that shows its frames.
    pub video: Option<crate::video::Player>,
    pub video_loading: Option<String>,
    pub video_task: Option<Task<()>>,
    /// Markdown files shown in full rather than their first lines.
    pub media_open: HashSet<String>,

    /// SSH mode: servers of ~/.ssh/config and their terminals.
    pub ssh: servers::Ssh,
    /// Tools mode (⌘3): the utilities of `crate::devtools`.
    pub tools: tools::Tools,
    /// Tasks mode (⌘4): Apple Reminders as a task list.
    pub tasks: tasks::TasksMode,
    /// The files of an FTP / SFTP host, shown in its thread.
    pub files_remote: remote_files::RemoteFiles,
    /// The project's own folder in a thread (`/files`).
    pub local_files: local_files::LocalFiles,
    /// The left column (sessions, servers, or the maximized IDE's tree) is one column: one
    /// width, kept by the app across the two layouts that show it (`ui/mod.rs`).
    pub sidebar_w: Pixels,
    pub layout_state: Entity<gpui_kit::component::resizable::ResizableState>,
    /// The layout shape (maximized IDE, IDE on the right) the left width was last applied to;
    /// a new shape gets it again.
    pub layout_synced: Option<(bool, bool, bool)>,

    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

impl BenchApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // "The agent finished" notifications: macOS asks once.
        crate::notify::request();
        let (evt_tx, evt_rx) = async_channel::unbounded();

        let mut targets = vec![Target::Local];
        targets.extend(connection::ssh_hosts().into_iter().map(Target::Ssh));
        let labels: Vec<SharedString> = targets.iter().map(|t| t.label().into()).collect();
        // A window opened from another one starts where it was asked to; else where we were last.
        let next = marks::NEXT_WINDOW.lock().unwrap().take();
        let last = match &next {
            Some((target, _, _)) => target.clone(),
            None => std::fs::read_to_string(connection::state_dir().join("last-target"))
                .unwrap_or_default(),
        };
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
                .auto_grow(1, 100_000)
                .submit_on_enter(false)
        });
        if let Ok(v) = std::env::var("BENCH_PREFILL") {
            composer.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search sessions"));
        let clarify_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Your own answer — Enter to send"));
        let project_path =
            cx.new(|cx| InputState::new(window, cx).placeholder("/home/you/project — Enter"));
        let group_name = cx.new(|cx| InputState::new(window, cx).placeholder("Name a group"));
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
        let subs = vec![
            // "System" follows macOS light/dark as it changes.
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance.mode == crate::theme::Mode::System {
                    crate::theme::apply(this.appearance, Some(window), cx);
                    this.repaint_terminals(cx);
                }
            }),
            // Enter in the question card's field sends that text as the answer.
            cx.subscribe_in(
                &clarify_input,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let text = state.read(cx).value().trim().to_string();
                        if !text.is_empty() {
                            this.answer_clarify(text, window, cx);
                        }
                    }
                },
            ),
            cx.subscribe_in(&composer, window, |this, _, ev: &InputEvent, window, cx| {
                match ev {
                    // Enter is a new line; ⌘Enter sends.
                    InputEvent::PressEnter {
                        secondary: true, ..
                    } => this.send(window, cx),
                    // New text: the command list starts over at its first row.
                    InputEvent::Change => {
                        this.note_draft(cx);
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
                    if this
                        .media_playing
                        .as_ref()
                        .is_some_and(|(_, p)| p.is_done())
                    {
                        this.media_playing = None;
                    }
                    if this.media_playing.is_some() {
                        cx.notify(); // the elapsed time
                    }
                    if this.live.is_some() {
                        this.speed.tick();
                    }
                    if this.live.is_some() || this.recorder.is_some() {
                        cx.notify();
                    }
                    this.tick_tasks(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        // The open session's background processes, every 3 s (the gateway has no event for them).
        let processes = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                if this.update(cx, |this, _| this.poll_processes()).is_err() {
                    break;
                }
            }
        });
        // The gateway item in the status bar: its state and the last lines of its log, every 10 s
        // while connected. Both come from where this project's agent runs, so a project on a
        // server shows that server's gateway.
        let backend = cx.spawn(async move |this, cx| {
            loop {
                let Ok(job) = this.update(cx, |this, _| {
                    this.conn
                        .clone()
                        .map(|c| (c, this.current_profile(), this.target.clone()))
                }) else {
                    break;
                };
                if let Some((conn, profile, target)) = job {
                    let r = cx
                        .background_spawn(async move {
                            (
                                rest::gateway_status(&conn, &profile),
                                connection::gateway_log(&target, 12),
                            )
                        })
                        .await;
                    if this
                        .update(cx, |this, cx| {
                            this.gw_status = Some(r.0.map_err(|e| format!("{e:#}")));
                            this.gw_log = r.1;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_secs(10))
                    .await;
            }
        });
        let mut tasks = vec![events, ticker, processes, backend];

        // The gateway list keeps the SSH hosts that can run the agent (asked again when the menu opens).
        let ssh = servers::Ssh::new(window, cx);
        let tools = tools::Tools::new(window, cx);
        let tasks_mode = tasks::TasksMode::new(window, cx);
        let files_remote = remote_files::RemoteFiles::new(window, cx);
        let local_files = local_files::LocalFiles::new(window, cx);
        for host in targets.iter().filter_map(|t| match t {
            Target::Ssh(h) => Some(h.clone()),
            Target::Local => None,
        }) {
            tasks.push(cx.spawn_in(window, async move |this, cx| {
                let h = host.clone();
                let ok = cx
                    .background_spawn(async move { connection::host_check(&h) })
                    .await;
                let _ = this.update(cx, |this, cx| this.host_checked(host, ok, cx));
            }));
        }

        let mut app = Self {
            ssh,
            tools,
            files_remote,
            local_files,
            tasks: tasks_mode,
            sidebar_w: px(280.),
            layout_state: cx.new(|_| Default::default()),
            layout_synced: None,
            targets,
            target: initial.clone(),
            _target_choice: target_choice,
            gateway: None,
            conn: None,
            new_subject: String::new(),
            inserts: HashMap::new(),
            creating: false,
            insert_draft: Vec::new(),
            queue: Vec::new(),
            insert_live: HashMap::new(),
            form_after: None,
            subject_target: None,
            thread_project: String::new(),
            projects: vec![],
            host_browser: false,
            project_form: None,
            project_env: None,
            ssh_projects: HashMap::new(),
            prompts_for: None,
            remotes: HashMap::new(),
            subject_opened: false,
            sites: Default::default(),
            sites_error: None,
            click_claimed: false,
            click_at: None,
            composer_fresh: false,
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
            thread_cache: HashMap::new(),
            settle: 0,
            settle_stable: 0,
            settle_ready: false,
            history_error: None,
            demo_draft_done: false,
            live: None,
            queued: None,
            attachments: vec![],
            sidebar_hidden: false,
            files_open: false,
            processes: vec![],
            processes_open: false,
            process_open: None,
            send_after_transcribe: false,
            usage: None,
            effort: EFFORTS.len() - 1,
            effort_other: None,
            profile_effort: EFFORTS.len() - 1,
            profile_models: vec![],
            pv: Default::default(),
            gw_status: None,
            gw_log: Vec::new(),
            catalog: vec![],
            slash_sel: 0,
            slash_closed: false,
            slash_scroll: ScrollHandle::new(),
            approval: None,
            clarify: None,
            clarify_picked: vec![],
            clarify_input,
            todos: vec![],
            plan_at: None,
            plan_open: false,
            approval_mode: None,
            profile_context: 0,
            marks: marks::load(),
            start_place: next.as_ref().map(|(_, p, s)| (p.clone(), s.clone())),
            places: HashMap::new(),
            inferred: HashMap::new(),
            project: None,
            default_cwd: None,
            resume_session: None,
            new_project: None,
            new_project_repo: true,
            path_entry: false,
            project_path,
            group_name,
            appearance: crate::theme::Appearance::load(),
            speed: crate::speed::Meter::fresh(),
            search,
            search_query: String::new(),
            search_hits: None,
            search_seq: 0,
            effort_pending: false,
            panel: Panel::Chat,
            skills: vec![],
            skill_open: None,
            prompts: vec![],
            prompt_open: None,
            effort_choice,
            composer,
            drafts: load_drafts(),
            draft_key: None,
            chat_scroll: ScrollHandle::new(),
            follow: true,
            last_max: px(0.),
            composer_y: px(0.),
            composer_h: 0.,
            viewport_h: px(0.),
            open_steps: HashSet::new(),
            open_source: None,
            wave_ticker: None,
            recorder: None,
            transcribing: false,
            playing: None,
            speaking: None,
            profile_ready: false,
            demo: false,
            demo_threads: HashMap::new(),
            restart_armed: None,
            media: HashMap::new(),
            diagrams: HashMap::new(),
            media_playing: None,
            media_since: Instant::now(),
            video: None,
            video_loading: None,
            video_task: None,
            media_open: HashSet::new(),
            _subscriptions: subs,
            _tasks: tasks,
        };
        // The browser build is always the demo: there is nothing to connect to there.
        if cfg!(target_family = "wasm") || std::env::var("BENCH_DEMO").is_ok() {
            crate::demo::load(&mut app, cx);
        } else {
            app.connect(initial, window, cx);
        }
        // A new window opened from SSH mode starts in SSH mode.
        if let Some(alias) = servers::NEXT_SSH.lock().unwrap().take() {
            cx.spawn_in(window, async move |this, cx| {
                let _ = this.update_in(cx, |this, window, cx| {
                    // The server's thread (this machine's terminal when none was named).
                    let alias = if alias.is_empty() {
                        crate::term::LOCAL.to_string()
                    } else {
                        alias
                    };
                    this.open_server_thread(alias, window, cx);
                });
            })
            .detach();
        }
        app
    }

    /// Connected to a gateway — or showing the demo, which looks connected.
    pub fn online(&self) -> bool {
        self.conn.is_some() || self.demo
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

    pub fn connect(&mut self, target: Target, _window: &mut Window, cx: &mut Context<Self>) {
        self.gateway = None; // drops the old thread
        self.conn = None;
        self.target = target.clone();
        self.connecting = true;
        self.profile_ready = false;
        self.approval_mode = None;
        self.opened.clear();
        self.profiles.clear();
        self.profile = None;
        self.sessions.clear();
        self.skills.clear();
        self.reset_chat(cx);
        self.pv = Default::default();
        self.gw_status = None;
        self.gw_log.clear();
        let dir = connection::state_dir();
        let _ = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(dir.join("last-target"), target.label()));
        self.gateway = Some(Gateway::start(target, self.evt_tx.clone()));
        cx.notify();
    }

    /// To the agent the command concerns (`remotes.rs`): this Mac's, or a remote project's.
    fn cmd(&self, cmd: Cmd) {
        self.route(cmd);
    }

    /// A waiting message (the thread's queue): sent now, into the running work, or taken back.
    pub fn queued(&mut self, id: String, send_now: bool, cx: &mut Context<Self>) {
        let Some(session) = self.runtime.clone() else {
            return;
        };
        self.cmd(if send_now {
            Cmd::QueueSend { session, id }
        } else {
            Cmd::QueueWithdraw { session, id }
        });
        cx.notify();
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
                // Tasks live with the agent: another machine, its own tasks.
                self.restart_tasks(cx);
                self.reload_servers(cx);
                self.load_remote_hosts(window, cx);
                self.load_projects(window, cx);
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
                turns,
                running,
            } => {
                self.opened.insert(session.clone(), runtime.clone());
                self.thread_cache.insert(session.clone(), turns.clone());
                if self.session.as_deref() == Some(&session) {
                    self.runtime = Some(runtime);
                    self.turns = turns;
                    self.settle_ready = true;
                    self.settle_stable = 0;
                    self.restore_reviews();
                    // The clock runs from the user's message, not from when this chat was opened.
                    let since = live_since(self.turns.last().and_then(|t| t.started));
                    self.live = running.then(|| Live {
                        started: since,
                        label: "Working…".into(),
                    });
                    self.reset_scroller(cx);
                    self.resolve_project(window, cx);
                }
            }
            Evt::Effort(sid, value) if self.is_current(&sid) => {
                // The session's own level replaces whatever the bar showed (the profile's default
                // or the previous chat's).
                self.effort_pending = false;
                match EFFORTS.iter().position(|e| *e == value) {
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
                    self.settle = 0;
                }
            }
            Evt::Queue(sid, items) if self.is_current(&sid) => {
                self.queue = items;
                cx.notify();
            }
            // A waiting message the agent took up: the thread's next turn.
            Evt::UserPlaced(sid, text) if self.is_current(&sid) => {
                self.turns.push(Turn {
                    user: text,
                    started: Some(now_secs()),
                    ..Default::default()
                });
                self.follow = true;
                cx.notify();
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
                        started: live_since(self.turns.last().and_then(|t| t.started)),
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
                // The agent may have changed the list this thread is about.
                let subject = self.current_subject();
                if subject.starts_with("tasks:") {
                    self.load_tasks();
                } else if let Some(id) = subject.strip_prefix("env:") {
                    self.load_env(id, window, cx);
                }
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
            Evt::Processes(sid, list) if self.is_current(&sid) => self.on_processes(list, cx),
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
            Evt::Clarify(req) if self.is_current(&req.session) => {
                if let Some(live) = self.live.as_mut() {
                    live.label = "Waiting for your answer…".into();
                }
                self.clarify = Some(req);
                self.clarify_picked.clear();
                self.clarify_input
                    .update(cx, |s, cx| s.set_value("", window, cx));
                self.remeasure_last(cx);
            }
            Evt::ClarifyDone(sid) => {
                if self.clarify.as_ref().is_some_and(|c| c.session == sid) {
                    self.clarify = None;
                }
            }
            Evt::Todos(sid, todos, at) if self.is_current(&sid) => {
                // The plan block shows work still ahead: once no step is pending or in progress
                // (the agent finished the plan, or dropped it as wrong), the block goes by itself
                // — a plan that is over tells nothing the thread does not already say.
                let live = todos
                    .iter()
                    .any(|t| t.status == "pending" || t.status == "in_progress");
                if live {
                    // It stays where the plan began, not at the newest message: the same steps
                    // keep the moment they were first seen with, a new plan gets a new one.
                    self.plan_at = Some(if at > 0. {
                        at
                    } else if self.todos.is_empty() {
                        now_secs()
                    } else {
                        self.plan_at.unwrap_or_else(now_secs)
                    });
                    self.todos = todos;
                } else {
                    self.todos.clear();
                    self.plan_at = None;
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
                if let Some(turn) = self.turns.last_mut() {
                    turn.finished = Some(now_secs());
                    if !text.trim().is_empty() {
                        turn.answer = text;
                    }
                }
                // The answer is over: its speed is the thread's to keep (Bench measured it, the
                // agent stores it where the thread lives).
                let stat = self.speed.stat();
                if !stat.is_empty() {
                    self.cmd(Cmd::SaveSpeed {
                        session: session.clone(),
                        speed: stat.to_json(),
                    });
                }
                self.remeasure_last(cx);
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
    fn notify_finished(&self, rows: &[crate::model::SessionRow]) {
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

    // ---- chat ------------------------------------------------------------

    fn reset_chat(&mut self, cx: &mut Context<Self>) {
        self.processes.clear();
        self.process_open = None;
        self.session = None;
        self.runtime = None;
        self.inserts.remove("draft");
        self.creating = false;
        self.queue.clear();
        // A new chat starts on the profile's level.
        self.effort_other = None;
        self.effort = self.profile_effort;
        self.turns.clear();
        self.history_error = None;
        self.live = None;
        self.queued = None;
        self.usage = None;
        self.playing = None;
        self.media_playing = None;
        self.stop_video(None, cx);
        self.approval = None;
        self.clarify = None;
        self.todos.clear();
        self.plan_at = None;
        // Speeds describe the model and server, not the chat: keep them, restart the counters.
        self.speed.restart();
        self.project = None;
        self.new_project = None;
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
        // The next frame scrolls to the composer (render_chat follows the tail).
        self.follow = true;
        self.last_max = px(-1.);
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
        self.ssh.form = None;
        self.files_remote.form = false;
        self.project_form = None;
        if let Some(prev) = self.session.clone() {
            self.thread_cache.insert(prev, self.turns.clone());
        }
        self.reset_chat(cx);
        self.session = Some(id.clone());
        // Invisible until laid out at its bottom; a thread seen before shows its last state now.
        self.settle = 30;
        self.settle_stable = 0;
        self.settle_ready = false;
        if let Some(turns) = self.thread_cache.get(&id) {
            self.turns = turns.clone();
            self.settle_ready = true;
        }
        self.thread_project = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.project.clone())
            .unwrap_or_default();
        if self.demo {
            self.turns = self.demo_threads.get(&id).cloned().unwrap_or_default();
            self.settle_ready = true;
            self.processes = if id == "d-landing" {
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
        // A remote project's thread is its server agent's to delete.
        let host = self.host_of_session(&id);
        let remote = (!host.is_empty())
            .then(|| self.remotes.get(&host).and_then(|r| r.conn.clone()))
            .flatten();
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

    /// An open session takes the level at once (its next turn runs on it, the gateway confirms
    /// with session.info); a chat not created yet takes it with its first message.
    pub fn set_effort(&mut self, i: usize, cx: &mut Context<Self>) {
        self.effort = i;
        self.effort_other = None;
        match self.runtime.clone() {
            Some(session) => {
                self.effort_pending = false;
                self.cmd(Cmd::SetEffort {
                    session,
                    effort: EFFORTS[i].to_string(),
                });
            }
            None => self.effort_pending = true,
        }
        cx.notify();
    }

    fn take_effort(&mut self) -> Option<String> {
        std::mem::take(&mut self.effort_pending).then(|| EFFORTS[self.effort].to_string())
    }

    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // While dictating, Send stops the recording and sends what it recognizes.
        if self.recorder.is_some() || self.transcribing {
            self.send_after_transcribe = true;
            if self.recorder.is_some() {
                self.toggle_mic(window, cx);
            }
            cx.notify();
            return;
        }
        let typed = self.composer.read(cx).value().trim().to_string();
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let files = self.attachments.clone();
        if typed.is_empty() && files.is_empty() {
            return;
        }
        // Enter in the command list picks the command instead of sending.
        if files.is_empty() && self.complete_slash(window, cx) {
            return;
        }
        // "!command": run it in this thread's terminal (opened under the thread).
        if let Some(command) = typed.strip_prefix('!') {
            let command = command.trim().to_string();
            self.composer
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.run_bang(command, window, cx);
            return;
        }
        // While the agent works, the message waits in the thread's queue (Pi's inbox) until the
        // answer is ready — with its files too: they are staged first (copied aside, uploaded
        // over SSH), then the same queue. The old refusal ("files go with the next message")
        // was ours, not Pi's: the inbox takes a follow-up with attachments like any other.
        if self.live.is_some() {
            let Some(session) = self.runtime.clone() else {
                return;
            };
            let text = typed;
            self.composer
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.note_draft(cx);
            self.attachments.clear();
            let context = self.block_context(cx);
            // It becomes the thread's next turn: the inserts that waited with it go after it.
            let after = self.turns.len() + 1;
            if files.is_empty() {
                self.cmd(Cmd::Queue {
                    session,
                    text,
                    context,
                });
                self.anchor_draft_inserts(after, cx);
                cx.notify();
                return;
            }
            let Some(conn) = self.conn.clone() else {
                return;
            };
            let target = self.target.clone();
            let profile2 = profile.clone();
            let keep = files.clone();
            cx.spawn_in(window, async move |this, cx| {
                let prepared = cx
                    .background_spawn(
                        async move { attach::prepare(&conn, &target, &profile2, &files) },
                    )
                    .await;
                let _ = this.update_in(cx, |this, window, cx| match prepared {
                    Ok(prep) => {
                        // The thread he sent from, even if another is open by now.
                        let same = this.runtime.as_deref() == Some(session.as_str());
                        this.cmd(Cmd::Queue {
                            session,
                            text: with_attachments(&text, &prep.lines),
                            context,
                        });
                        if same {
                            this.anchor_draft_inserts(after, cx);
                        }
                    }
                    Err(e) => {
                        this.attachments = keep;
                        this.toast(
                            NotificationType::Error,
                            format!("Attach: {e:#}"),
                            window,
                            cx,
                        );
                    }
                });
            })
            .detach();
            return;
        }
        // Code from the IDE follows the words, each block naming its file and lines.
        let text = typed;
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.note_draft(cx);
        self.attachments.clear();
        let shown: Vec<String> = files
            .iter()
            .map(|f| format!("{}{}]", attach::PREFIX, f.display()))
            .collect();
        self.turns.push(Turn {
            user: with_attachments(&text, &shown),
            started: Some(now_secs()),
            ..Default::default()
        });
        // Inserts put while this message was in the composer sit after it, not before.
        let after = self.turns.len();
        self.anchor_draft_inserts(after, cx);
        self.follow = true;
        self.speed.begin_answer();
        self.speed.request();
        self.live = Some(Live {
            started: Instant::now(),
            label: if files.is_empty() {
                "Sending…"
            } else {
                "Uploading…"
            }
            .into(),
        });
        if files.is_empty() {
            self.submit(profile, text, window, cx);
            return;
        }
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let target = self.target.clone();
        let keep = files.clone();
        cx.spawn_in(window, async move |this, cx| {
            let p = profile.clone();
            let prepared = cx
                .background_spawn(async move { attach::prepare(&conn, &target, &p, &files) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match prepared {
                Ok(prep) => {
                    if let Some(live) = this.live.as_mut() {
                        live.label = "Sending…".into();
                    }
                    let full = with_attachments(&text, &prep.lines);
                    this.submit(profile, full, window, cx);
                }
                Err(e) => {
                    this.live = None;
                    // His files come back: the message stays his to send again.
                    this.attachments = keep;
                    this.toast(
                        NotificationType::Error,
                        format!("Attach: {e:#}"),
                        window,
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    /// The message (with its files named in the text) to the session, or to a new one.
    fn submit(
        &mut self,
        profile: String,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                    let context = self.block_context(cx);
                    self.cmd(Cmd::Send {
                        profile,
                        session,
                        text,
                        effort,
                        context,
                    });
                }
            },
            None => {
                // A new session is created on the first message; the effort goes with it.
                self.effort_pending = true;
                self.queued = Some(text.clone());
                let group = self.group_name.read(cx).value().trim().to_string();
                if self.new_project.is_none() && !group.is_empty() {
                    // A named group: its folder first (made where the gateway lives), then the chat.
                    self.group_name
                        .update(cx, |s, cx| s.set_value("", window, cx));
                    let target = self.target.clone();
                    self.background(
                        window,
                        cx,
                        move || connection::group_dir(&target, &group),
                        move |this, r, window, cx| match r {
                            Ok(dir) => this.cmd(this.create_cmd(profile, Some(dir))),
                            Err(e) => {
                                this.toast(
                                    NotificationType::Error,
                                    format!("Couldn't make the group's folder: {e}"),
                                    window,
                                    cx,
                                );
                                this.cmd(this.create_cmd(profile, None));
                            }
                        },
                    );
                    cx.notify();
                    return;
                }
                self.cmd(self.create_cmd(profile, self.new_project.clone()));
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

    /// Restart the agent on the machine we are connected to, then reconnect. Its runs are
    /// durable, so it never refuses (the `Busy` branch: a backend that refused while work ran,
    /// pressed again within a minute, restarted anyway).
    /// The gateway panel's notification line: refresh what macOS allows, and with `test` also
    /// send one — the way to see for oneself that Bench can say it is done from the background.
    pub fn notif(&mut self, test: bool, cx: &mut Context<Self>) {
        if test {
            crate::notify::post(
                "bench.test",
                "Bench",
                "This is a test: Bench says it is done.",
            );
        }
        crate::notify::check();
        cx.notify();
    }

    /// System Settings → Notifications: the choice is macOS's to keep, not ours.
    pub fn notif_settings(&mut self, cx: &mut Context<Self>) {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.notifications")
            .spawn();
        cx.notify();
    }

    pub fn restart_gateway(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        let interrupt = self
            .restart_armed
            .take()
            .is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        self.toast(
            NotificationType::Info,
            if interrupt {
                "Interrupting running work and restarting the agent…"
            } else {
                "Restarting the agent…"
            },
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let t = target.clone();
            let result = cx
                .background_spawn(async move { connection::restart_gateway(&t, interrupt) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(connection::Restart::Done) => {
                    this.toast(NotificationType::Success, "Agent restarted", window, cx);
                    this.connect(target, window, cx);
                }
                Ok(connection::Restart::Busy(what)) => {
                    this.restart_armed = Some(Instant::now());
                    this.toast(
                        NotificationType::Warning,
                        format!("Not restarted: {what}. Press restart again to interrupt it and restart."),
                        window,
                        cx,
                    );
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
        self.profile_ready = false;
        self.rest(window, cx, rest::profile_defaults, |this, r, window, cx| {
            this.profile_ready = true;
            let Ok((effort, model, models, approval, context)) = r else {
                return;
            };
            this.approval_mode = Some(approval);
            this.profile_context = context;
            this.profile_models = models;
            this.add_picker_models();
            let level = effort.and_then(|v| EFFORTS.iter().position(|e| *e == v));
            if let Some(i) = level {
                this.profile_effort = i;
            }
            // The profile's level is for new chats: an open session already has its own.
            if let Some(i) = level.filter(|_| this.runtime.is_none()) {
                this.effort = i;
                this.effort_other = None;
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

    /// The moment the agent's newest background process began, in unix seconds: where its
    /// BACKGROUND row hangs in the thread. The newest, not the first: the row follows the work
    /// the agent is really doing, but does not slide down as new turns arrive — only when it
    /// starts something new. None while the session has no processes at all.
    pub fn proc_moment(&self) -> Option<f64> {
        newest_start(&self.processes)
    }

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

    pub fn set_appearance(
        &mut self,
        a: crate::theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.appearance = a;
        a.save();
        crate::theme::apply(a, Some(window), cx);
        self.repaint_terminals(cx);
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

    /// The agent sends review notes only live; keep them per chat (turn index → notes) to show again.
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
            (self.profile.clone(), self.session.clone()),
        );
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path, serde_json::to_vec(&all).unwrap_or_default());
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
            Panel::Chat => {}
            Panel::Providers => self.load_providers(window, cx),
            Panel::NewProject => {}
        }
        cx.notify();
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
                            let send = std::mem::take(&mut this.send_after_transcribe);
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
                                    if send {
                                        this.send(window, cx);
                                    }
                                }
                                Ok(_) => this.toast(
                                    NotificationType::Warning,
                                    "Nothing recognized",
                                    window,
                                    cx,
                                ),
                                Err(e) => {
                                    let e = format!("{e:#}");
                                    let msg = if crate::pi_agent::ask_local_network(&e) {
                                        "Transcription: allow Bench on the local network (macOS asks now), then dictate again".to_string()
                                    } else {
                                        format!("Transcription: {e}")
                                    };
                                    this.toast(NotificationType::Error, msg, window, cx)
                                }
                            }
                        },
                    );
                }
                Err(e) => {
                    self.send_after_transcribe = false;
                    self.toast(
                        NotificationType::Warning,
                        format!("Recording: {e:#}"),
                        window,
                        cx,
                    );
                }
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
        self.media_playing = None;
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

pub fn now_secs() -> f64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
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

/// The message's text with the files' `[attached: …]` lines under it.
fn with_attachments(text: &str, lines: &[String]) -> String {
    std::iter::once(text.to_string())
        .filter(|t| !t.is_empty())
        .chain(lines.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n\n")
}

///  ("session search"; an MCP tool
/// `mcp__<server>__<tool>` as "<server> · <tool>"), then what it worked on.
pub fn tool_label(name: &str, detail: &str) -> String {
    let name = match name.strip_prefix("mcp__").and_then(|n| n.split_once("__")) {
        Some((server, tool)) => format!("{server} · {}", tool.replace('_', " ")),
        None => name.replace('_', " "),
    };
    // One line: a command with a heredoc, a patch, a quoted page — the running line is one row
    // tall whatever the tool was handed, or the thread jumps in height as the work changes.
    let detail = clip_line(detail);
    if detail.is_empty() {
        name
    } else {
        format!("{name}: {detail}")
    }
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

impl BenchApp {
    /// The composer's text is the open chat's draft (called on every change).
    pub(crate) fn note_draft(&mut self, cx: &mut Context<Self>) {
        // A new chat keeps no draft: every new chat starts empty.
        let Some(key) = self.draft_key.clone().filter(|k| !k.is_empty()) else {
            return;
        };
        let text = self.composer.read(cx).value().to_string();
        let changed = if text.trim().is_empty() {
            self.drafts.remove(&key).is_some()
        } else {
            self.drafts.insert(key, text.clone()).as_deref() != Some(text.as_str())
        };
        if changed {
            save_drafts(&self.drafts);
        }
    }

    /// Another chat is open (or the new one): the composer shows its draft.
    pub(crate) fn sync_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.session.clone().unwrap_or_default();
        if self.draft_key.as_ref() == Some(&key) {
            return;
        }
        // The new chat just became this thread (an insert, a message): what is typed stays.
        let became = self.draft_key.as_deref() == Some("")
            && !key.is_empty()
            && !self.drafts.contains_key(&key);
        let text = if became {
            self.composer.read(cx).value().to_string()
        } else if key.is_empty() {
            String::new()
        } else {
            // A lone "/word" is a command half picked, not a draft.
            self.drafts
                .get(&key)
                .cloned()
                .filter(|t| !(t.starts_with('/') && !t.trim().contains(' ')))
                .unwrap_or_default()
        };
        self.draft_key = Some(key);
        if became {
            self.note_draft(cx);
            return;
        }
        self.composer
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }
}

fn drafts_path() -> std::path::PathBuf {
    crate::connection::state_dir().join("drafts.json")
}

fn load_drafts() -> std::collections::HashMap<String, String> {
    std::fs::read(drafts_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_drafts(drafts: &std::collections::HashMap<String, String>) {
    let _ = std::fs::create_dir_all(crate::connection::state_dir());
    let _ = std::fs::write(
        drafts_path(),
        serde_json::to_vec(drafts).unwrap_or_default(),
    );
}

/// The newest start among the session's background processes (see `BenchApp::proc_moment`).
fn newest_start(ps: &[crate::model::BgProcess]) -> Option<f64> {
    ps.iter()
        .filter_map(|p| p.started)
        .fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.max(t))))
}

#[cfg(test)]
mod proc_moment_tests {
    use super::newest_start;
    use crate::model::BgProcess;

    fn proc(started: Option<f64>) -> BgProcess {
        BgProcess {
            id: "p".into(),
            command: "cargo build".into(),
            running: started.is_some(),
            exit_code: None,
            started,
            exited: None,
            reason: None,
            tail: String::new(),
        }
    }

    #[test]
    fn the_row_follows_the_newest_process() {
        assert_eq!(newest_start(&[]), None, "no processes: no row");
        assert_eq!(
            newest_start(&[proc(None)]),
            None,
            "a start the agent never gave"
        );
        assert_eq!(
            newest_start(&[proc(Some(100.)), proc(Some(300.))]),
            Some(300.)
        );
        assert_eq!(
            newest_start(&[proc(Some(300.)), proc(Some(100.))]),
            Some(300.),
            "the order the agent lists them in means nothing"
        );
    }
}

#[cfg(test)]
mod tool_label_tests {
    #[test]
    fn labels() {
        assert_eq!(super::tool_label("bash", "ls"), "bash: ls");
        assert_eq!(super::tool_label("session_search", ""), "session search");
        assert_eq!(
            super::tool_label("bash", "python3 - <<'PY'\nimport re\np='src/app.rs'"),
            "bash: python3 - <<'PY'",
            "a multiline command stays one line"
        );
        assert_eq!(super::tool_label("bash", "\n\n   \n"), "bash");
        assert_eq!(
            super::tool_label("mcp__reminders__list_items", "Today"),
            "reminders · list items: Today"
        );
    }
}
