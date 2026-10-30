//! The sidebar's items — a thread or a project — and what the person does to them, the same way
//! for both: pin and color (`marks.rs`), delete (asked once: ⌫ ⌫ for a thread, Remove · Cancel
//! for a project), and the keyboard. A thread is also renamed in place (a project in its
//! environment). The order the sidebar draws (`sidebar_groups`) is the order ↑ ↓ walk.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use protocol::ThreadId;

use crate::app::BenchApp;
use crate::app::model::ProjectEntry;

/// A row of the sidebar that can be pinned, colored, renamed and deleted.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Thread(ThreadId),
    Project(String),
}

impl Item {
    /// Its key among the marks (a project's are `p:<id>`).
    pub fn mark_key(&self) -> String {
        match self {
            Item::Thread(id) => id.to_string(),
            Item::Project(id) => format!("p:{id}"),
        }
    }
}

/// An item being renamed: the field in place of its name.
pub struct Renaming {
    pub item: Item,
    pub field: Entity<InputState>,
    _events: Subscription,
}

impl BenchApp {
    /// The threads as the sidebar groups them: the Host's (no project), then each project's,
    /// pinned projects and pinned threads first.
    pub fn sidebar_groups(&self) -> (Vec<usize>, Vec<(ProjectEntry, Vec<usize>)>) {
        let visible = self.visible_threads();
        let mut loose: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|&i| {
                let s = &self.threads.list[i];
                s.host.is_empty()
                    && (s.project.is_empty() || !self.places.projects.iter().any(|p| p.id == s.project))
            })
            .collect();
        // Pinned first in the Host's group too: a new thread lands under them, never above.
        loose.sort_by_key(|&i| !self.mark_of(self.threads.list[i].id.as_str()).pinned);
        let loose = self.nest_forks(loose);
        let mut projects = self.places.projects.clone();
        // The order the person put them in by dragging (`order.rs`); those never arranged keep
        // their source's order, after them, pinned ones first (`marks.rs`).
        projects.sort_by_key(|p| {
            (
                self.project_rank(&p.id),
                !self.mark_of(&format!("p:{}", p.id)).pinned,
            )
        });
        let groups = projects
            .into_iter()
            .map(|p| {
                // A project's threads: of its agent, in it (a server's are placed by `agents.rs`).
                let host = p.agent.clone();
                let mut rows: Vec<usize> = visible
                    .iter()
                    .copied()
                    .filter(|&i| {
                        let s = &self.threads.list[i];
                        s.host == host && s.project == p.id
                    })
                    .collect();
                rows.sort_by_key(|&i| !self.mark_of(self.threads.list[i].id.as_str()).pinned);
                (p, self.nest_forks(rows))
            })
            .collect();
        (loose, groups)
    }

    /// The thread a row sits under: the one it was forked from, when that one is in the same
    /// group (a parent deleted or elsewhere leaves the fork a row of its own).
    pub(crate) fn fork_parent(&self, i: usize) -> Option<usize> {
        let s = &self.threads.list[i];
        let parent = s.parent.as_ref()?;
        self.threads
            .list
            .iter()
            .position(|t| &t.id == parent && t.host == s.host && t.project == s.project)
    }

    /// A group's rows with every fork right under the thread it came from, as Claude's sidebar
    /// nests a session's children; the rest keep their order.
    fn nest_forks(&self, rows: Vec<usize>) -> Vec<usize> {
        let parent = |i: usize| self.fork_parent(i).filter(|p| rows.contains(p));
        let mut out = Vec::with_capacity(rows.len());
        fn place(i: usize, rows: &[usize], parent: &dyn Fn(usize) -> Option<usize>, out: &mut Vec<usize>) {
            out.push(i);
            for &c in rows.iter().filter(|&&c| parent(c) == Some(i)) {
                place(c, rows, parent, out);
            }
        }
        for &i in rows.iter().filter(|&&i| parent(i).is_none()) {
            place(i, &rows, &parent, &mut out);
        }
        // A loop of parents (not made by forks, but kept safe): its rows at the end.
        for &i in &rows {
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out
    }

    /// The threads on screen in the sidebar, top to bottom (folded groups hide theirs).
    pub fn drawn_threads(&self) -> Vec<ThreadId> {
        let (loose, groups) = self.sidebar_groups();
        let mut out = vec![];
        if self.sidebar.collapsed.contains("open:loose") {
            out.extend(loose.iter().map(|&i| self.threads.list[i].id.clone()));
        }
        for (p, rows) in groups {
            if !self.sidebar.collapsed.contains(&format!("p:{}", p.id)) {
                out.extend(rows.iter().map(|&i| self.threads.list[i].id.clone()));
            }
        }
        out
    }

    /// The field an item is being renamed in, if it is.
    pub fn renaming_field(&self, item: &Item) -> Option<Entity<InputState>> {
        self.sidebar.renaming
            .as_ref()
            .filter(|r| &r.item == item)
            .map(|r| r.field.clone())
    }

    pub fn item_name(&self, item: &Item) -> String {
        match item {
            Item::Thread(id) => self
                .threads.list
                .iter()
                .find(|s| &s.id == id)
                .map(|s| s.title.clone())
                .unwrap_or_default(),
            Item::Project(id) => self
                .places.projects
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_default(),
        }
    }

    /// Its name becomes a field in place: Enter keeps the new one, Esc or a click away does not.
    pub fn start_rename(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.item_name(&item);
        let field = cx.new(|cx| InputState::new(window, cx));
        field.update(cx, |s, cx| {
            s.set_value(name, window, cx);
            s.focus(window, cx);
        });
        let events = cx.subscribe_in(
            &field,
            window,
            |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => this.commit_rename(window, cx),
                InputEvent::Blur => this.cancel_rename(cx),
                _ => {}
            },
        );
        self.sidebar.renaming = Some(Renaming {
            item,
            field,
            _events: events,
        });
        cx.notify();
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if self.sidebar.renaming.take().is_some() {
            cx.notify();
        }
    }

    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.sidebar.renaming.take() else {
            return;
        };
        let name = r.field.read(cx).value().trim().to_string();
        cx.notify();
        if name.is_empty() || name == self.item_name(&r.item) {
            return;
        }
        match r.item {
            // The thread's agent keeps its title; the sidebar shows it at once.
            Item::Thread(id) => {
                let host = self.host_of_thread(&id);
                if let Some(s) = self.threads.list.iter_mut().find(|s| s.host == host && s.id == id) {
                    s.title = name.clone();
                }
                self.send(&host, protocol::RenameThread { thread: id, title: name }, cx);
            }
            Item::Project(id) => self.rename_project(id, name, window, cx),
        }
    }

    /// Keys the sidebar takes while it has the focus (after a click on a thread): ↑ ↓ open the
    /// thread above or below, ⌫ and ⌦ delete the open one and open the next — as a file in
    /// Finder. Enter or typing goes on to write in the thread.
    pub fn sidebar_key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        if ks.modifiers.modified() {
            return;
        }
        let order = self.drawn_threads();
        let at = self
            .thread(cx)
            .id
            .as_ref()
            .and_then(|id| order.iter().position(|o| o == id));
        match ks.key.as_str() {
            "up" | "down" => {
                let next = match (ks.key.as_str(), at) {
                    ("up", Some(k)) => k.checked_sub(1),
                    ("down", Some(k)) => Some(k + 1).filter(|&n| n < order.len()),
                    (_, None) => (!order.is_empty()).then_some(0),
                    _ => None,
                };
                if let Some(n) = next {
                    self.open_listed(order[n].clone(), cx);
                }
            }
            "backspace" | "delete" => {
                let Some(k) = at else {
                    return;
                };
                // The first ⌫ asks (the row shows Delete · Cancel), the second deletes: a stray
                // key never takes a thread.
                if !self.armed.is(&crate::app::armed::thread_key(order[k].as_str())) {
                    self.arm_delete(order[k].to_string(), cx);
                    return;
                }
                // The one after it opens (the one before, at the end): deleting goes on with ⌫.
                let next = order
                    .get(k + 1)
                    .or(k.checked_sub(1).and_then(|p| order.get(p)));
                let next = next.cloned();
                self.delete_listed(order[k].clone(), cx);
                if let Some(n) = next {
                    self.open_listed(n, cx);
                }
                window.focus(&self.sidebar.focus, cx);
            }
            "escape" => self.cancel_delete(cx),
            "enter" => self.focus_composer(window, cx),
            _ => {}
        }
    }
}
