//! What the agents say unasked (`protocol::Event`), applied to the window: a thread changed — to
//! the views showing it, and what a run's end means for the window (a notification, stale copies
//! of files) —, an agent's list of threads changed, the tasks changed.

use gpui_kit::*;
use protocol::{Event, ThreadEvent, ThreadId};

use super::*;
use crate::app::model::ThreadRow;
use crate::thread::ThreadView;

impl BenchApp {
    pub(super) fn on_agent_event(
        &mut self,
        host: &str,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            Event::ThreadsChanged => self.list_threads(host, cx),
            Event::TasksChanged => self.load_tasks(),
            // A project's environment shown here is read again.
            Event::EnvChanged { project } => {
                let key = crate::app::model::ProjectKey::new(host, &project).to_string();
                if self.envs.contains_key(&key) {
                    self.load_env(&key, window, cx);
                }
            }
            Event::Thread { thread, event } => {
                self.on_thread_event(host, &thread, event, window, cx)
            }
        }
    }

    /// A change of the thread `thread` of the agent `host`: each view showing it takes it; what it
    /// means beyond the thread follows.
    fn on_thread_event(
        &mut self,
        host: &str,
        thread: &ThreadId,
        event: ThreadEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let views = self.views_of(host, thread, cx);
        for view in &views {
            view.update(cx, |v, cx| v.on_thread_event(&event, window, cx));
        }
        if let Some(view) = views.first() {
            match &event {
                // The inserts' kinds read the list from the app: it follows the journal.
                ThreadEvent::Inserts(_) | ThreadEvent::Append(_) => self.inserts_from(view, cx),
                ThreadEvent::Run(false) => self.run_ended(view, window, cx),
                _ => {}
            }
        }
        // A hold or a run stills or stirs the thread's row in the sidebar at once.
        if let ThreadEvent::Holds(_) | ThreadEvent::Run(_) = &event {
            let (running, paused) = match views.first() {
                Some(v) => {
                    let j = &v.read(cx).thread.journal;
                    (j.running(), j.paused())
                }
                None => (matches!(event, ThreadEvent::Run(true)), false),
            };
            if let Some(row) = self.row_mut(host, thread) {
                row.running = running;
                row.paused = paused;
            }
        }
        cx.notify();
    }

    /// A run of a thread on screen ended: the files its agent's machine has may have changed, and
    /// the person, when Bench is behind other windows, is told.
    fn run_ended(&mut self, view: &Entity<ThreadView>, window: &mut Window, cx: &mut Context<Self>) {
        let at = view.read(cx).thread_ref();
        // Every copy we cached of that agent's machine is stale now; the next look brings the
        // current one. It may also have committed in any repo the windows show: the chips re-read.
        let label = self.thread_target(&at).label();
        self.media.retain(|k, _| !k.starts_with(&format!("{label}\0")));
        self.git_info.clear();
        // Its speed is the thread's to keep: Bench measured it, the agent stores it.
        let speed = view.read(cx).thread.speed.stat();
        if let Some(id) = at.id.clone()
            && !speed.is_empty()
        {
            self.send_at(&at, protocol::SaveSpeed { thread: id, speed }, cx);
        }
        // Bench in the background: say the thread is done (in front, it is on screen).
        if !window.is_window_active() {
            let v = view.read(cx);
            let answer = v.thread.journal.last_answer().unwrap_or_default();
            let first = answer
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("Done");
            let id = at.key();
            let title = self.title_of(&at);
            crate::system::notify::post(&id, &title, &bench_ui::clip(first, 140));
        }
    }

    /// The views on screen showing the thread `thread` of the agent `host`.
    pub(crate) fn views_of(
        &self,
        host: &str,
        thread: &ThreadId,
        cx: &App,
    ) -> Vec<Entity<ThreadView>> {
        self.thread_views()
            .into_iter()
            .filter(|v| {
                let t = &v.read(cx).thread;
                t.host == host && t.id.as_ref() == Some(thread)
            })
            .collect()
    }

    /// The sidebar's row of a thread.
    pub(crate) fn row_mut(&mut self, host: &str, thread: &ThreadId) -> Option<&mut ThreadRow> {
        self.threads
            .iter_mut()
            .find(|r| r.host == host && &r.id == thread)
    }

    /// The agent `host` listed its threads: its rows are these. One of the person's own threads
    /// that was running and is done now, and is not on screen, is said in a notification; the
    /// thread the app was closed on opens again once its row is known.
    pub(super) fn threads_listed(
        &mut self,
        host: &str,
        rows: Vec<ThreadRow>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for old in self.threads.iter().filter(|s| s.host == host && s.running) {
            let done = rows.iter().any(|r| r.id == old.id && !r.running);
            let own = crate::app::model::source_label(&old.source) == "Bench";
            let on_screen = !self.views_of(host, &old.id, cx).is_empty();
            if done && own && !on_screen {
                let title = if old.title.is_empty() {
                    "Bench"
                } else {
                    &old.title
                };
                crate::system::notify::post(old.id.as_str(), title, "The agent finished");
            }
        }
        self.threads.retain(|s| s.host != host);
        self.threads.extend(rows);
        if let Some((h, id)) = self.reopen.clone()
            && h == host
            && self.threads.iter().any(|s| s.host == h && s.id == id)
        {
            self.reopen = None;
            if self.thread(cx).id.is_none() {
                self.open_thread(h, id, cx);
            }
        }
        cx.notify();
    }
}
