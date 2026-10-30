//! Application state, the agents' events and user actions. Rendering lives with each area of the window
//! (`workspace/`, `titlebar/`, `sidebar/`, `thread/`, `settings/`).

use std::collections::HashMap;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

pub(crate) mod agents;
pub(crate) mod armed;
mod backend;
mod defaults;
mod events;
mod keys;
pub mod menu;
pub(crate) mod model;
pub(crate) mod places;
pub(crate) mod repo;
pub(crate) mod shortcuts;
mod subscriptions;

use crate::env::ProjectEnv;
use crate::thread::{ActiveThread, ThreadView};
use crate::workspace::{Center, Dock, DockPosition, Panel};

/// This machine's own project (the agent's general environment): installing, building, looking
/// after the machine. A server's is a project named after it ("Dev"); Host is first in the sidebar.
pub const LOCAL_HOST: &str = "Host";

/// The open thread's run, as the status says it: what the agent is doing.
pub struct Live {
    pub label: SharedString,
}

pub struct BenchApp {
    /// What this Mac's agent says before a thread says its own (`defaults.rs`).
    pub defaults: defaults::AgentDefaults,
    /// Where the sidebar's projects come from, and the projects (`places.rs`).
    pub places: places::Places,
    /// Every agent's threads, the journals seen, the drafts (`thread/store.rs`).
    pub threads: crate::thread::store::ThreadStore,
    /// The inserts of each thread and each kind's state (`inserts/state.rs`).
    pub inserts: crate::inserts::state::InsertsState,
    /// The answers' media: files, diagrams, what plays, dictation (`media/state.rs`).
    pub media: crate::media::state::MediaState,
    /// The threads panel: search, folds, marks, renaming (`sidebar/state.rs`).
    pub sidebar: crate::sidebar::state::SidebarState,
    /// The gear while open (`settings/state.rs`).
    pub gear: crate::settings::state::Gear,
    /// The new project's page (`settings/new_project.rs`).
    pub new_project: crate::settings::new_project::NewProject,
    /// The Code status bar's corner for the open file: caret, indent, language (`explorer/pane/cursor.rs`).
    pub cursor_item: Entity<crate::explorer::pane::cursor::CursorItem>,
    /// The docks' dividers (`workspace/layout.rs`).
    pub layout: crate::workspace::layout::LayoutState,
    /// The repo chip's git facts (`repo.rs`).
    pub git: repo::GitState,
    /// The open thread, an entity of its own (`thread/view.rs`): its state, its composer, drawn
    /// by itself in the center (or the right dock, beside a document).
    pub active_thread: Entity<ThreadView>,
    /// A second thread, beside the open one (the right dock's `Panel::Thread` when the center
    /// shows a thread): its own view, its events routed to it (`app/events.rs`).
    pub beside_thread: Option<Entity<ThreadView>>,
    /// The window this app is: a request's answer comes back to it (`agents.rs`).
    pub window: AnyWindowHandle,
    /// Projects' environments, by project id (`ProjectEnv::id`), each read once something shows
    /// it: the Explorer (`explorer.project`), a settings or a part's tab (`DocKey`).
    pub envs: HashMap<String, ProjectEnv>,
    /// Every agent, by the host it runs on ("" = this Mac's): `app/agents.rs`.
    pub agents: HashMap<String, agents::AgentLink>,
    /// What waits for a second press (a delete, closing an unsaved tab): `armed.rs`.
    pub armed: armed::Armed,

    pub appearance: bench_ui::theme::Appearance,

    /// What the middle of the window shows (`workspace.rs`).
    pub center: Center,
    /// The window's docks, as Zed's workspace has them (`workspace/dock.rs`): the left one (the
    /// threads or the Explorer's tree; ⌘\ folds it), the right one (the thread beside a
    /// document), the bottom one (under the center).
    pub left_dock: Dock,
    pub right_dock: Dock,
    pub bottom_dock: Dock,
    /// Back and forward over the threads and documents the center showed (`workspace/history.rs`).
    pub nav: crate::workspace::history::NavHistory,
    /// The Explorer's state (`src/explorer/`).
    pub explorer: crate::explorer::Explorer,

    /// Keeps the `/`-list keystroke interceptor alive (main.rs).
    pub _slash_keys: Option<gpui_kit::Subscription>,

    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) _tasks: Vec<Task<()>>,
}

impl BenchApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // "The agent finished" notifications: macOS asks once.
        crate::system::notify::request();
        // A window opened from another one opens the thread it was asked for; else the one the
        // app was last closed on.
        let reopen = crate::sidebar::marks::NEXT_WINDOW
            .lock()
            .unwrap()
            .take()
            .or_else(Self::last_place);

        let app = cx.weak_entity();
        let active_thread = cx.new(|cx| ThreadView::new(app, window, cx));
        let sidebar = crate::sidebar::state::SidebarState::new(window, cx);
        let new_project = crate::settings::new_project::NewProject::new(window, cx);
        let subs = Self::subscriptions(&sidebar.search, &new_project.path, window, cx);
        let tasks = Self::timers(window, cx);

        let mut app = Self {
            defaults: Default::default(),
            places: places::Places {
                ssh_servers: crate::system::sshconfig::load(),
                ..Default::default()
            },
            threads: crate::thread::store::ThreadStore::new(reopen),
            inserts: crate::inserts::state::InsertsState::new(window, cx),
            media: Default::default(),
            sidebar,
            gear: Default::default(),
            new_project,
            cursor_item: cx.new(|_| crate::explorer::pane::cursor::CursorItem::default()),
            layout: crate::workspace::layout::LayoutState::new(cx),
            git: Default::default(),
            window: window.window_handle(),
            envs: HashMap::new(),
            agents: HashMap::new(),
            armed: Default::default(),
            appearance: crate::settings::appearance::load(),
            center: Center::Chat,
            left_dock: Dock::new(DockPosition::Left, &[Panel::Threads, Panel::Explorer], true),
            right_dock: Dock::new(DockPosition::Right, &[Panel::Thread], false),
            bottom_dock: Dock::new(DockPosition::Bottom, &[Panel::Terminal], false),
            nav: Default::default(),
            active_thread,
            beside_thread: None,
            explorer: Default::default(),
            _slash_keys: None,
            _subscriptions: subs,
            _tasks: tasks,
        };
        app.connect_agent(String::new(), window, cx);
        app
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

    /// Files carried in from the machine go to the thread the person is working in — the one
    /// whose composer has the focus, else the open one — and wait there with the next message
    /// (`thread/composer/attachments.rs`). That thread comes on screen if it is not: a file never
    /// waits where it cannot be seen. A folder is not a file to attach, and it is said.
    pub fn drop_paths(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focused = self
            .thread_views()
            .into_iter()
            .find(|v| {
                v.read(cx)
                    .composer
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            });
        let view = focused.unwrap_or_else(|| self.active_thread.clone());
        if !paths.iter().any(|p| p.is_file()) {
            self.toast(NotificationType::Info, "Only files attach, not folders", window, cx);
            return;
        }
        match self.center {
            Center::Chat => {}
            Center::Doc => self.right_dock.activate(Panel::Thread),
            Center::Settings | Center::NewProject => self.center = Center::Chat,
        }
        view.update(cx, |v, cx| v.add_attachments(paths, cx));
        cx.notify();
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
