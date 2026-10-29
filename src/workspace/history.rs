//! Back and forward over what the center showed — ‹ › in the title bar, ⌘[ ⌘] — as Zed's pane
//! keeps its navigation history and Finder its own. A place is a thread or a document; it is
//! noted when the center comes to show it (`note_place`, from the window's render), so every way
//! of opening one (the sidebar, ⌘K, a link, a tab) is remembered without being told. A new
//! thread or a settings page is no place: back from one returns to the last place shown.

use gpui_kit::*;
use protocol::ThreadId;

use crate::app::BenchApp;
use crate::explorer::pane::doc::DocKey;
use crate::workspace::Center;

/// How many places back are kept.
const KEPT: usize = 50;

/// Somewhere the center can be brought back to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Thread { host: String, id: ThreadId },
    Doc(DocKey),
}

/// The places before and after the current one.
#[derive(Default)]
pub struct NavHistory {
    back: Vec<Place>,
    forward: Vec<Place>,
    current: Option<Place>,
}

impl NavHistory {
    /// The center shows `shown`: a place other than the current one becomes current, the one
    /// before it goes back, and what was ahead is dropped, as in a browser.
    fn note(&mut self, shown: Option<Place>) {
        let Some(place) = shown else { return };
        if self.current.as_ref() == Some(&place) {
            return;
        }
        if let Some(prev) = self.current.take() {
            self.back.push(prev);
            if self.back.len() > KEPT {
                self.back.remove(0);
            }
        }
        self.current = Some(place);
        self.forward.clear();
    }

    /// Away from the current place (a new thread, settings): back is the current place itself.
    fn away(&self, shown: &Option<Place>) -> bool {
        self.current.is_some() && *shown != self.current
    }

    fn can_back(&self, shown: &Option<Place>) -> bool {
        self.away(shown) || !self.back.is_empty()
    }

    fn can_forward(&self, shown: &Option<Place>) -> bool {
        !self.away(shown) && !self.forward.is_empty()
    }

    fn back(&mut self, shown: &Option<Place>) -> Option<Place> {
        if self.away(shown) {
            return self.current.clone();
        }
        let to = self.back.pop()?;
        self.forward.extend(self.current.replace(to.clone()));
        Some(to)
    }

    fn forward(&mut self, shown: &Option<Place>) -> Option<Place> {
        if self.away(shown) {
            return None;
        }
        let to = self.forward.pop()?;
        self.back.extend(self.current.replace(to.clone()));
        Some(to)
    }

    /// A place that is gone (a deleted thread) leaves the history.
    fn forget(&mut self, gone: impl Fn(&Place) -> bool) {
        self.back.retain(|p| !gone(p));
        self.forward.retain(|p| !gone(p));
        if self.current.as_ref().is_some_and(&gone) {
            self.current = self.back.pop();
        }
    }
}

impl BenchApp {
    /// What the center shows now, as a place (none for a new thread or a settings page).
    fn shown_place(&self, cx: &App) -> Option<Place> {
        match self.center {
            Center::Chat => {
                let at = self.thread_ref(cx);
                at.id.map(|id| Place::Thread { host: at.host, id })
            }
            Center::Doc => self.explorer.active.clone().map(Place::Doc),
            Center::Settings | Center::NewProject => None,
        }
    }

    /// Called from the window's render: whatever opened it, the place shown is remembered.
    pub(crate) fn note_place(&mut self, cx: &App) {
        let shown = self.shown_place(cx);
        self.nav.note(shown);
    }

    pub(crate) fn can_go_back(&self, cx: &App) -> bool {
        self.nav.can_back(&self.shown_place(cx))
    }

    pub(crate) fn can_go_forward(&self, cx: &App) -> bool {
        self.nav.can_forward(&self.shown_place(cx))
    }

    /// ‹ and ⌘[: the place shown before this one.
    pub(crate) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.forget_gone();
        let shown = self.shown_place(cx);
        if let Some(to) = self.nav.back(&shown) {
            self.show_place(to, window, cx);
        }
    }

    /// › and ⌘]: the place left by going back.
    pub(crate) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.forget_gone();
        let shown = self.shown_place(cx);
        if let Some(to) = self.nav.forward(&shown) {
            self.show_place(to, window, cx);
        }
    }

    /// Threads deleted since they were shown are skipped.
    fn forget_gone(&mut self) {
        let list = &self.threads.list;
        self.nav.forget(|p| match p {
            Place::Thread { id, .. } => !list.iter().any(|s| &s.id == id),
            Place::Doc(_) => false,
        });
    }

    fn show_place(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        match place {
            Place::Thread { host, id } => self.open_thread(host, id, cx),
            Place::Doc(key) => self.open_doc(key, window, cx),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{NavHistory, Place};
    use protocol::ThreadId;

    fn t(id: &str) -> Option<Place> {
        Some(Place::Thread {
            host: "local".into(),
            id: ThreadId::from(id.to_string()),
        })
    }

    #[test]
    fn back_and_forward_walk_the_places_shown() {
        let mut h = NavHistory::default();
        h.note(t("a"));
        h.note(t("b"));
        h.note(t("c"));
        assert_eq!(h.back(&t("c")), t("b"));
        assert_eq!(h.back(&t("b")), t("a"));
        assert!(!h.can_back(&t("a")));
        assert_eq!(h.forward(&t("a")), t("b"));
        // Opening another place drops what was ahead.
        h.note(t("d"));
        assert!(!h.can_forward(&t("d")));
        assert_eq!(h.back(&t("d")), t("b"));
    }

    #[test]
    fn back_from_a_new_thread_returns_to_the_last_place() {
        let mut h = NavHistory::default();
        h.note(t("a"));
        h.note(t("b"));
        h.note(None);
        assert!(h.can_back(&None));
        assert!(!h.can_forward(&None));
        assert_eq!(h.back(&None), t("b"));
        assert_eq!(h.back(&t("b")), t("a"));
    }

    #[test]
    fn a_deleted_thread_is_skipped() {
        let mut h = NavHistory::default();
        h.note(t("a"));
        h.note(t("b"));
        h.note(t("c"));
        h.forget(|p| Some(p) == t("b").as_ref());
        assert_eq!(h.back(&t("c")), t("a"));
    }
}
