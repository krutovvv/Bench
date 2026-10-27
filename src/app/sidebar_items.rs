//! The sidebar's items — a thread or a project — and what the person does to them, the same way
//! for both: pin and color (`marks.rs`), delete (asked once: ⌫ ⌫ for a thread, Remove · Cancel
//! for a project), and the keyboard. A thread is also renamed in place (a project in its
//! environment). The order the sidebar draws (`sidebar_groups`) is the order ↑ ↓ walk.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;

use super::BenchApp;
use crate::gateway::Cmd;
use crate::model::ProjectEntry;

/// A row of the sidebar that can be pinned, colored, renamed and deleted.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Thread(String),
    Project(String),
}

impl Item {
    /// Its key among the marks (a project's are `p:<id>`).
    pub fn mark_key(&self) -> String {
        match self {
            Item::Thread(id) => id.clone(),
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
        let visible = self.visible_sessions();
        let mut loose: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|&i| {
                let s = &self.sessions[i];
                s.host.is_empty()
                    && (s.project.is_empty() || !self.projects.iter().any(|p| p.id == s.project))
            })
            .collect();
        // Pinned first in the Host's group too: a new thread lands under them, never above.
        loose.sort_by_key(|&i| !self.mark_of(&self.sessions[i].id).pinned);
        let mut projects = self.projects.clone();
        projects.sort_by_key(|p| !self.mark_of(&format!("p:{}", p.id)).pinned);
        let groups = projects
            .into_iter()
            .map(|p| {
                // A project's threads: of its agent, in it (a server's are placed by `agents.rs`).
                let host = p.agent.clone();
                let mut rows: Vec<usize> = visible
                    .iter()
                    .copied()
                    .filter(|&i| {
                        let s = &self.sessions[i];
                        s.host == host && s.project == p.id
                    })
                    .collect();
                rows.sort_by_key(|&i| !self.mark_of(&self.sessions[i].id).pinned);
                (p, rows)
            })
            .collect();
        (loose, groups)
    }

    /// The threads on screen in the sidebar, top to bottom (folded groups hide theirs).
    pub fn drawn_threads(&self) -> Vec<String> {
        let (loose, groups) = self.sidebar_groups();
        let mut out = vec![];
        if self.ssh.collapsed.contains("open:loose") {
            out.extend(loose.iter().map(|&i| self.sessions[i].id.clone()));
        }
        for (p, rows) in groups {
            if !self.ssh.collapsed.contains(&format!("p:{}", p.id)) {
                out.extend(rows.iter().map(|&i| self.sessions[i].id.clone()));
            }
        }
        out
    }

    /// The field an item is being renamed in, if it is.
    pub fn renaming_field(&self, item: &Item) -> Option<Entity<InputState>> {
        self.renaming
            .as_ref()
            .filter(|r| &r.item == item)
            .map(|r| r.field.clone())
    }

    pub fn item_name(&self, item: &Item) -> String {
        match item {
            Item::Thread(id) => self
                .sessions
                .iter()
                .find(|s| &s.id == id)
                .map(|s| s.title.clone())
                .unwrap_or_default(),
            Item::Project(id) => self
                .projects
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
        self.renaming = Some(Renaming {
            item,
            field,
            _events: events,
        });
        cx.notify();
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            cx.notify();
        }
    }

    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.renaming.take() else {
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
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                    s.title = name.clone();
                }
                self.cmd(Cmd::Rename {
                    session: id,
                    title: name,
                });
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
            .session
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
                    self.open_session(order[n].clone(), cx);
                }
            }
            "backspace" | "delete" => {
                let Some(k) = at else {
                    return;
                };
                // The first ⌫ asks (the row shows Delete · Cancel), the second deletes: a stray
                // key never takes a thread.
                if self.confirm_delete.as_deref() != Some(order[k].as_str()) {
                    self.arm_delete(order[k].clone(), cx);
                    return;
                }
                // The one after it opens (the one before, at the end): deleting goes on with ⌫.
                let next = order
                    .get(k + 1)
                    .or(k.checked_sub(1).and_then(|p| order.get(p)));
                let next = next.cloned();
                self.delete_session(order[k].clone(), window, cx);
                if let Some(n) = next {
                    self.open_session(n, cx);
                }
                window.focus(&self.sidebar_focus, cx);
            }
            "escape" => self.cancel_delete(cx),
            "enter" => self.composer.update(cx, |s, cx| s.focus(window, cx)),
            _ => {}
        }
    }
}
