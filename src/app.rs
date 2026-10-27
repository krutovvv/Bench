//! Application state, gateway events and user actions. Rendering lives with each area of the window
//! (`workspace/`, `titlebar/`, `sidebar/`, `thread/`, `settings/`, `statusbar/`).

use std::collections::{HashMap, HashSet};

use web_time::Instant;

use gpui_kit::component::IndexPath;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::select::SelectState;
use gpui_kit::*;

pub(crate) mod agents;
mod backend;
mod connect;
mod events;
mod keys;
pub mod menu;
pub(crate) mod model;
pub(crate) mod places;
pub(crate) mod projects;
pub(crate) mod shortcuts;
mod subscriptions;

use crate::env::ProjectEnv;
use crate::settings::new_project::ProjectForm;
use crate::thread::{ActiveThread, ThreadView};
use crate::workspace::{Center, Dock, DockPosition, Panel};

use crate::app::model::{EFFORTS, Profile, SessionRow, SkillRow, Target};
use crate::client::connection::{self};
use crate::client::rest;
use crate::client::rpc::Evt;
use crate::media::audio::{Playback, Recorder};
use crate::media::{self, Loaded};

pub type Choice = SelectState<Vec<SharedString>>;

/// This machine's own project (the agent's general environment): installing, building, looking
/// after the machine. A server's is a project named after it ("Dev"); Host is first in the sidebar.
pub const LOCAL_HOST: &str = "Host";

/// The open thread's run, as the status says it: what the agent is doing.
pub struct Live {
    pub label: SharedString,
}

pub struct BenchApp {
    /// The open thread, an entity of its own (`thread/view.rs`): its state, its composer, drawn
    /// by itself in the center (or the right dock, beside a document).
    pub active_thread: Entity<ThreadView>,
    /// A second thread, beside the open one (the right dock's `Panel::Thread` when the center
    /// shows a thread): its own view, its events routed to it (`app/events.rs`).
    pub beside_thread: Option<Entity<ThreadView>>,
    pub targets: Vec<Target>,
    pub target: Target,
    /// The gateway list as a Select (kept for its subscription; the status bar has its own menu).
    pub(crate) _target_choice: Entity<Choice>,
    /// The subject of the chat about to be created ("tasks:today"): it goes with session.create.
    pub new_subject: String,
    /// Each thread's inserts (by session id; "draft" for a new chat): `app::inserts`.
    pub inserts: HashMap<String, Vec<crate::app::model::Insert>>,
    /// A new chat's session is being made (an insert came before any message).
    pub creating: bool,
    /// Of the inserts whose state is app-wide (tasks, tools), the live one per kind.
    pub insert_live: HashMap<String, String>,
    /// The person's projects: the sidebar.
    pub projects: Vec<crate::app::model::ProjectEntry>,
    /// The machine's own agent (Host) has the watchable server browser.
    pub host_browser: bool,
    /// The new project's form, in the chat area while open (the sidebar's "+").
    pub project_form: Option<ProjectForm>,
    /// Projects' environments, by project id (`ProjectEnv::id`), each read once something shows
    /// it: the Explorer (`explorer.project`), a settings or a part's tab (`DocKey`).
    pub envs: HashMap<String, ProjectEnv>,
    /// The project whose prompts `prompts` holds (they follow the open thread's project).
    pub prompts_for: Option<String>,
    /// Every agent, by the host it runs on ("" = this Mac's): `app/agents.rs`.
    pub agents: HashMap<String, agents::AgentLink>,
    /// The agent whose models and voice the gear shows ("" = this Mac's).
    pub gear_agent: String,
    /// The person's presets (this Mac's agent's) and the kind open to edit in the gear.
    pub presets: crate::settings::presets::Presets,
    pub preset_editor: Option<crate::settings::presets::PresetEditor>,
    /// The reasoning levels offered (the presets'; `EFFORTS` until they are read).
    pub efforts: Vec<String>,
    /// The gear's tab: agents, presets, look.
    pub gear_tab: usize,
    /// The gear's "+ Agent on a server" form, while open (`agent_form.rs`).
    pub agent_form: Option<crate::settings::agent_form::AgentForm>,
    /// A thread or project being renamed in the sidebar (`sidebar_items.rs`).
    pub renaming: Option<crate::sidebar::items::Renaming>,
    /// The sidebar's keys (↑ ↓ ⌫) while it has the focus, after a click on a thread.
    pub sidebar_focus: FocusHandle,
    /// The sources of `projects` (`places.rs`): this Mac's agent's projects, the servers' agents
    /// this Mac knows, and each server's own projects as its agent last listed them.
    pub local_projects: Vec<crate::app::model::ProjectEntry>,
    pub server_agents: Vec<places::ServerAgent>,
    pub server_projects: HashMap<String, Vec<crate::app::model::ProjectEntry>>,
    /// BENCH_SUBJECT was opened (once).
    pub subject_opened: bool,
    /// Sites on the server's browser: each turn's tab, live or as its last frame.
    pub sites: crate::inserts::browser::state::Sites,
    pub sites_error: Option<String>,
    /// A step's tab being reopened: (thread, turn, url, the tabs before).
    /// A click in a thread landed in something that types (a terminal, an editor) or acts (a
    /// fold, a button): it keeps the focus. The window's pointer, so the app's.
    pub click_claimed: bool,
    /// Where the left button went down in the thread (a drag selects text: no focus move).
    pub click_at: Option<Point<Pixels>>,
    pub(crate) evt_tx: async_channel::Sender<Evt>,
    pub connecting: bool,

    pub profiles: Vec<Profile>,
    pub profile: Option<String>,
    pub profile_choice: Entity<Choice>,
    pub sessions: Vec<SessionRow>,
    pub confirm_delete: Option<String>,
    /// Stored id -> runtime id for sessions this client has opened (to close before delete).
    pub(crate) opened: HashMap<String, String>,
    /// Telegram: session id -> owner user id; known people; whose sessions to show (None = all).
    pub(crate) tg_owner: HashMap<String, String>,
    pub tg_people: Vec<(String, String)>,
    pub tg_visible: Option<HashSet<String>>,

    /// Each thread opened in this run, as last shown: switching back shows it at once.
    pub thread_cache: HashMap<String, ::thread::Thread>,
    /// Send was pressed while dictating: send once the speech is recognized.
    pub send_after_transcribe: bool,
    /// The project's code sent to a thread from the Explorer, waiting with its message, by
    /// `thread_key` (each thread its own, as its draft).
    pub references: HashMap<String, Vec<crate::thread::attachments::Reference>>,
    /// The profile's `agent.reasoning_effort`: where a new chat starts.
    pub(crate) profile_effort: usize,
    /// The profile's primary model and its fallbacks, as (model, provider).
    pub profile_models: Vec<rest::ModelChoice>,
    /// Settings → Providers and Messages.
    pub pv: crate::settings::providers::Providers,
    /// The gateway's state for the status bar's backend item (refreshed every 10 s):
    /// None before the first answer, Err when the gateway didn't answer.
    pub gw_status: Option<Result<rest::GatewayStatus, String>>,
    /// The last lines of the agent's log, from where it runs (its own status item shows them).
    pub gw_log: Vec<String>,
    /// Slash commands of the profile.
    pub catalog: Vec<crate::app::model::SlashCommand>,
    /// The profile's `approvals.mode`, once read.
    pub approval_mode: Option<String>,
    /// The profile's own approvals, effort and model have arrived: until then the status bar
    /// shows none of them (defaults there would only mislead).
    pub profile_ready: bool,
    /// A made-up session for screenshots (`BENCH_DEMO=1`, src/demo/mod.rs): nothing is connected.
    pub demo: bool,
    /// The demo's threads by session id (opening one shows its turns; nothing is fetched).
    pub demo_threads: HashMap<String, ::thread::Thread>,
    /// The gateway refused a restart over running work: the next press (within a minute)
    /// restarts anyway.
    pub restart_armed: Option<Instant>,
    /// `model.context_length` of the profile (0 when not set).
    pub profile_context: u64,
    /// Colors and pins of sessions (this Mac).
    pub marks: HashMap<String, crate::sidebar::marks::Mark>,
    /// This window was opened for a given profile/session (else: where the app was last).
    pub(crate) start_place: Option<(Option<String>, Option<String>)>,
    /// Session folders/repos as the agent recorded them, and repos inferred from edited files.
    pub places: HashMap<String, crate::app::model::Place>,
    pub inferred: HashMap<String, String>,
    /// The agent's default working folder (sessions started there have no folder of their own).
    pub default_cwd: Option<String>,
    /// Reopen this session once the list arrives (where the app was last closed).
    pub(crate) resume_session: Option<String>,
    /// For the next new chat: work in this folder.
    pub new_project: Option<String>,
    /// The chosen folder is a git repo (so "in a new branch" applies).
    pub new_project_repo: bool,
    /// Typing a folder path (SSH hosts have no native folder dialog here).
    pub path_entry: bool,
    pub project_path: Entity<InputState>,
    /// New chat: a group to put it in (`~/Research/<name>`) when it has no project.
    pub group_name: Entity<InputState>,
    pub appearance: bench_ui::theme::Appearance,
    pub speed: crate::statusbar::speed::Meter,
    /// Sidebar search: the field, what was typed, and the gateway's hits (None while searching).
    pub search: Entity<InputState>,
    pub search_query: String,
    pub search_hits: Option<Vec<(String, String)>>,
    pub(crate) search_seq: u64,

    /// What the middle of the window shows (`workspace.rs`).
    pub center: Center,
    /// The window's docks, as Zed's workspace has them (`workspace/dock.rs`): the left one (the
    /// threads or the Explorer's tree; ⌘\ folds it), the right one (the thread beside a
    /// document), the bottom one (under the center).
    pub left_dock: Dock,
    pub right_dock: Dock,
    pub bottom_dock: Dock,
    /// The Explorer's state (`src/explorer/`).
    pub explorer: crate::explorer::Explorer,
    pub skills: Vec<SkillRow>,
    pub skill_open: Option<String>,
    /// Settings → Prompts.
    pub prompts: Vec<crate::app::model::Prompt>,
    pub prompt_open: Option<usize>,
    pub effort_choice: Entity<Choice>,

    /// Unsent text per chat, kept on disk: switching chats or restarting keeps it. Each view's
    /// composer shows its thread's (`thread/drafts.rs`).
    pub(crate) drafts: std::collections::HashMap<String, String>,
    /// Keeps the `/`-list keystroke interceptor alive (main.rs).
    pub _slash_keys: Option<gpui_kit::Subscription>,
    pub recorder: Option<Recorder>,
    /// Repaints the waveform while recording.
    pub(crate) wave_ticker: Option<Task<()>>,
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
    pub video: Option<crate::media::video::Player>,
    pub video_loading: Option<String>,
    pub video_task: Option<Task<()>>,
    /// Markdown files shown in full rather than their first lines.
    pub media_open: HashSet<String>,

    /// /terminal: the servers of ~/.ssh/config and the terminals' shells.
    pub ssh: crate::inserts::terminal::servers::Ssh,
    /// /tools: the utilities of `devtools`.
    pub tools: crate::inserts::tools::state::Tools,
    /// The tasks the /tasks insert shows, and its card.
    pub tasks: crate::inserts::tasks::state::TasksMode,
    /// The docks' dividers (`workspace/layout.rs`): across the window, and over the bottom dock.
    pub layout_state: Entity<gpui_kit::component::resizable::ResizableState>,
    pub bottom_state: Entity<gpui_kit::component::resizable::ResizableState>,
    /// Which docks were drawn when the dividers last took their sizes; another set takes them
    /// again.
    pub layout_synced: Option<(bool, bool, bool)>,

    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) _tasks: Vec<Task<()>>,
}

impl BenchApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // "The agent finished" notifications: macOS asks once.
        crate::system::notify::request();
        let (evt_tx, evt_rx) = async_channel::unbounded();

        let mut targets = vec![Target::Local];
        targets.extend(connection::ssh_hosts().into_iter().map(Target::Ssh));
        let labels: Vec<SharedString> = targets.iter().map(|t| t.label().into()).collect();
        // A window opened from another one starts where it was asked to; else where we were last.
        let next = crate::sidebar::marks::NEXT_WINDOW.lock().unwrap().take();
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

        let app = cx.weak_entity();
        let active_thread = cx.new(|cx| ThreadView::new(app, window, cx));
        if let Ok(v) = std::env::var("BENCH_PREFILL") {
            let composer = active_thread.read(cx).composer.clone();
            composer.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search sessions"));
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
        let subs = Self::subscriptions(
            &search,
            &project_path,
            &effort_choice,
            &target_choice,
            &profile_choice,
            window,
            cx,
        );

        let mut tasks = Self::timers(evt_rx, window, cx);

        // The gateway list keeps the SSH hosts that can run the agent (asked again when the menu opens).
        let ssh = crate::inserts::terminal::servers::Ssh::new();
        let tools = crate::inserts::tools::state::Tools::new(window, cx);
        let tasks_mode = crate::inserts::tasks::state::TasksMode::new(window, cx);
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
            tasks: tasks_mode,
            layout_state: cx.new(|_| Default::default()),
            bottom_state: cx.new(|_| Default::default()),
            layout_synced: None,
            targets,
            target: initial.clone(),
            _target_choice: target_choice,
            new_subject: String::new(),
            inserts: HashMap::new(),
            creating: false,
            insert_live: HashMap::new(),
            projects: vec![],
            host_browser: false,
            project_form: None,
            envs: HashMap::new(),
            prompts_for: None,
            agents: HashMap::new(),
            gear_agent: String::new(),
            renaming: None,
            agent_form: None,
            presets: Default::default(),
            preset_editor: None,
            efforts: EFFORTS.iter().map(|e| e.to_string()).collect(),
            gear_tab: 0,
            sidebar_focus: cx.focus_handle(),
            local_projects: vec![],
            server_agents: vec![],
            server_projects: HashMap::new(),
            subject_opened: false,
            sites: Default::default(),
            sites_error: None,
            click_claimed: false,
            click_at: None,
            evt_tx,
            connecting: false,
            profiles: vec![],
            profile: None,
            profile_choice,
            sessions: vec![],
            confirm_delete: None,
            opened: HashMap::new(),
            tg_owner: HashMap::new(),
            tg_people: vec![],
            tg_visible: None,
            thread_cache: HashMap::new(),
            references: HashMap::new(),
            send_after_transcribe: false,
            profile_effort: EFFORTS.len() - 1,
            profile_models: vec![],
            pv: Default::default(),
            gw_status: None,
            gw_log: Vec::new(),
            catalog: vec![],
            approval_mode: None,
            profile_context: 0,
            marks: crate::sidebar::marks::load(),
            start_place: next.as_ref().map(|(_, p, s)| (p.clone(), s.clone())),
            places: HashMap::new(),
            inferred: HashMap::new(),
            default_cwd: None,
            resume_session: None,
            new_project: None,
            new_project_repo: true,
            path_entry: false,
            project_path,
            group_name,
            appearance: crate::settings::appearance::load(),
            speed: crate::statusbar::speed::Meter::fresh(),
            search,
            search_query: String::new(),
            search_hits: None,
            search_seq: 0,
            center: Center::Chat,
            left_dock: Dock::new(DockPosition::Left, &[Panel::Threads, Panel::Explorer], true),
            right_dock: Dock::new(DockPosition::Right, &[Panel::Thread], false),
            bottom_dock: Dock::new(DockPosition::Bottom, &[Panel::Terminal], false),
            active_thread,
            beside_thread: None,
            explorer: Default::default(),
            skills: vec![],
            skill_open: None,
            prompts: vec![],
            prompt_open: None,
            effort_choice,
            drafts: crate::thread::drafts::load_drafts(),
            _slash_keys: None,
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
        self.local_conn().is_some() || self.demo
    }

    /// The open thread's state (its view's). Never while that view draws or handles an event:
    /// what a view asks of the app is told which thread it is (`ThreadRef`, `thread/view.rs`).
    pub fn thread<'a>(&self, cx: &'a App) -> &'a ActiveThread {
        &self.active_thread.read(cx).thread
    }

    /// The views of the threads on screen: the open one, and the one beside it.
    pub fn thread_views(&self) -> Vec<Entity<ThreadView>> {
        std::iter::once(self.active_thread.clone())
            .chain(self.beside_thread.clone())
            .collect()
    }

    /// The open thread's composer takes the focus.
    pub fn focus_composer(&self, window: &mut Window, cx: &mut App) {
        let composer = self.active_thread.read(cx).composer.clone();
        composer.update(cx, |s, cx| s.focus(window, cx));
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
}

pub fn now_secs() -> f64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}
