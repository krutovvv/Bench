//! A second thread beside the open one: its own `ThreadView` in the right dock while the center
//! shows a thread (`BenchApp::beside_thread`), with its own composer, run, queue and events (the
//! app routes each thread's events to the views showing it, `app/events.rs`). Beside a document
//! the right dock shows the open thread itself instead (`panel.rs`).

use gpui_kit::*;

use super::ThreadView;
use crate::app::BenchApp;
use protocol::ThreadId;
use crate::workspace::{Center, Panel};

impl BenchApp {
    /// The sidebar's "Open beside": thread `id` in the right dock, beside what the center shows.
    /// Beside a document that is the open thread itself; beside a thread, a second view.
    pub fn open_beside(&mut self, id: ThreadId, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host_of_thread(&id);
        if self.center != Center::Chat {
            self.open_thread(host, id, cx);
            self.center = Center::Doc;
            self.right_dock.activate(Panel::Thread);
            cx.notify();
            return;
        }
        // The open thread is already on screen; the one beside, too.
        let shown = |v: &Entity<ThreadView>| {
            let t = &v.read(cx).thread;
            t.host == host && t.id.as_ref() == Some(&id)
        };
        if shown(&self.active_thread) || self.beside_thread.as_ref().is_some_and(shown) {
            self.right_dock.activate(Panel::Thread);
            cx.notify();
            return;
        }
        self.close_beside(cx);
        let app = cx.weak_entity();
        let view = cx.new(|cx| ThreadView::new(app, window, cx));
        // A thread seen before shows its last state now, and catches up when its journal comes.
        let journal = if self.demo {
            self.demo_threads.get(id.as_str()).cloned()
        } else {
            self.threads.cache.remove(id.as_str())
        };
        let project = self.project_of_thread(&host, &id);
        view.update(cx, |v, _| {
            v.thread.host = host.clone();
            v.thread.id = Some(id.clone());
            v.thread.journal = journal.unwrap_or_default();
            v.thread.project = project;
            v.thread.effort = self.defaults.effort;
        });
        let at = view.read(cx).thread_ref();
        self.beside_thread = Some(view);
        self.right_dock.activate(Panel::Thread);
        // Nothing to fetch in the demo: its thread is all there.
        if !self.demo {
            self.fetch_thread(at, cx);
        }
        cx.notify();
    }

    /// The thread beside goes; what it showed is kept for when it is opened again.
    pub fn close_beside(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.beside_thread.take() {
            let kept = view.update(cx, |v, _| {
                let journal = std::mem::take(&mut v.thread.journal);
                v.thread.id.clone().map(|s| (s, journal))
            });
            if let Some((id, journal)) = kept {
                self.threads.cache.insert(id.to_string(), journal);
            }
        }
        cx.notify();
    }

    /// The center opens thread `id`: if it was beside, it leaves the right dock (one thread, one
    /// view: its events reach the center's only).
    pub(crate) fn unbeside(&mut self, host: &str, id: &ThreadId, cx: &mut Context<Self>) {
        let beside = self.beside_thread.as_ref().is_some_and(|v| {
            let t = &v.read(cx).thread;
            t.host == host && t.id.as_ref() == Some(id)
        });
        if beside {
            self.close_beside(cx);
        }
    }
}
