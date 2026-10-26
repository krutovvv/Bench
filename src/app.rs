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
mod backend;
mod events;
mod keys;
pub mod marks;
pub(crate) mod mcp_status;
mod processes;
pub(crate) mod project_list;
pub(crate) mod projects;
pub(crate) mod remotes;
mod send;
mod sessions;
mod telegram;
mod voice;
pub use crate::env::ProjectEnv;
pub use crate::env::project_form::ProjectForm;
mod prompts;
pub(crate) mod providers;
mod slash;
pub(crate) mod subjects;

use crate::audio::{Playback, Recorder};
use crate::connection::{self, Connection};
use crate::gateway::{Cmd, Evt, Gateway};
use crate::media::{self, Loaded};
use crate::model::{EFFORTS, Profile, SessionRow, SkillRow, Target, Usage};
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

/// The open thread's run, as the status says it: what the agent is doing.
pub struct Live {
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
    /// The waiting message whose words the person is fixing, and the field they fix them in.
    pub queue_edit: Option<(String, Entity<TextareaState>)>,
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
    pub sites: crate::inserts::browser::state::Sites,
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

    /// The open thread: its journal, what is being written, its queue (`crates/thread`).
    pub thread: ::thread::Thread,
    /// Each thread opened in this run, as last shown: switching back shows it at once.
    pub thread_cache: HashMap<String, ::thread::Thread>,
    /// The open thread on screen: its list and rows (`ui::thread`).
    pub tv: crate::ui::thread::View,
    /// Opened pieces of work and steps, by the work row's key.
    pub open_work: HashSet<String>,
    /// The open thread's texts, parsed once (`ui::media::Texts`).
    pub texts: crate::ui::media::Texts,
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
    /// The fallback model is answering the open thread: (its model, why the main one failed).
    pub fallback_on: Option<(String, String)>,
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
    pub plan_open: bool,
    /// The profile's `approvals.mode`, once read.
    pub approval_mode: Option<String>,
    /// The profile's own approvals, effort and model have arrived: until then the status bar
    /// shows none of them (defaults there would only mislead).
    pub profile_ready: bool,
    /// A made-up session for screenshots (`BENCH_DEMO=1`, src/demo.rs): nothing is connected.
    pub demo: bool,
    /// The demo's threads by session id (opening one shows its turns; nothing is fetched).
    pub demo_threads: HashMap<String, ::thread::Thread>,
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
    /// The next change of the thread brings the composer into view (a message sent, an insert put).
    pub follow: bool,
    /// Keeps the `/`-list keystroke interceptor alive (main.rs).
    pub _slash_keys: Option<gpui_kit::Subscription>,
    /// Where the composer is in the window, and the window's height (last frame): the `/` list
    /// opens above the line when there is no room under it.
    pub composer_y: Pixels,
    /// The composer's measured height, from the same canvas as `composer_y`: how much of the
    /// window the thread's last row has to share with it (`past_end`).
    pub composer_h: f32,
    pub viewport_h: Pixels,
    /// Unfolded steps (work key, step) and the source host whose pages are listed (work key, host).
    pub open_steps: HashSet<(String, usize)>,
    pub open_source: Option<(String, String)>,
    pub recorder: Option<Recorder>,
    /// Repaints the waveform while recording.
    wave_ticker: Option<Task<()>>,
    pub transcribing: bool,
    /// The answer read aloud (its row key), and the one whose audio is being made.
    pub playing: Option<(String, Playback)>,
    pub speaking: Option<String>,
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
    pub ssh: crate::inserts::terminal::servers::Ssh,
    /// Tools mode (⌘3): the utilities of `devtools`.
    pub tools: crate::inserts::tools::state::Tools,
    /// The tasks the /tasks insert shows, and its card.
    pub tasks: crate::inserts::tasks::state::TasksMode,
    /// The files of an FTP / SFTP host, shown in its thread.
    pub files_remote: crate::inserts::ftp::state::RemoteFiles,
    /// The project's own folder in a thread (`/files`).
    pub local_files: crate::inserts::files::state::LocalFiles,
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
        let ssh = crate::inserts::terminal::servers::Ssh::new(window, cx);
        let tools = crate::inserts::tools::state::Tools::new(window, cx);
        let tasks_mode = crate::inserts::tasks::state::TasksMode::new(window, cx);
        let files_remote = crate::inserts::ftp::state::RemoteFiles::new(window, cx);
        let local_files = crate::inserts::files::state::LocalFiles::new(window, cx);
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
            queue_edit: None,
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
            thread: Default::default(),
            thread_cache: HashMap::new(),
            tv: Default::default(),
            open_work: HashSet::new(),
            texts: Default::default(),
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
            fallback_on: None,
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
            appearance: crate::theme::load(),
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
            drafts: send::load_drafts(),
            draft_key: None,
            follow: false,
            _slash_keys: None,
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
    pub(crate) fn cmd(&self, cmd: Cmd) {
        self.route(cmd);
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

    // ---- chat ------------------------------------------------------------

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

    // ---- right panel: changed files, tree, viewer --------------------------

    pub fn set_appearance(
        &mut self,
        a: crate::theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.appearance = a;
        crate::theme::save(&a);
        crate::theme::apply(a, Some(window), cx);
        self.repaint_terminals(cx);
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
}

pub fn now_secs() -> f64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
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

impl BenchApp {}

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
