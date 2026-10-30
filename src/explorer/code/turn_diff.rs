//! What one turn of a thread changed in a file, in a tab of its own (`DocKey::TurnDiff`), read
//! only: the file as the repository's trees had it at the turn's start and end
//! (`agent/src/runtime/changes.ts`, `GET /api/git/show`), as a unified diff. The thread shows the
//! same diff inline under the turn (`thread/rows/changed_files.rs`); this is its "Open in Code".

use bench_ui::icons::Ico;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rest;
use crate::explorer::pane::doc::Turn;
use crate::explorer::pane::item::Item;
use super::git::{DiffBody, diff_editor, diff_pane};
use crate::explorer::{Doc, DocKey, FileRef};

pub struct TurnDiffItem {
    pub file: FileRef,
    pub turn: Turn,
}

impl TurnDiffItem {
    fn key(&self) -> DocKey {
        DocKey::TurnDiff {
            file: self.file.clone(),
            turn: self.turn.clone(),
        }
    }
}

impl Item for TurnDiffItem {
    fn tab(&self, _app: &BenchApp, cx: &App) -> (String, (Ico, Hsla)) {
        (
            format!("{} (turn)", self.file.name()),
            (Ico::FileDiff, cx.theme().muted_foreground),
        )
    }

    fn open(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        let key = self.key();
        if app.explorer.docs.contains_key(&key) {
            return true;
        }
        app.explorer.docs.insert(key.clone(), Doc::new(diff_editor(window, cx)));
        let Some((conn, _)) = app.env_target(&self.file.project) else {
            return true;
        };
        let (f, t) = (self.file.clone(), self.turn.clone());
        app.background(
            window,
            cx,
            move || turn_diff(&conn, &f, &t),
            move |this, r, window, cx| {
                let Some(d) = this.explorer.docs.get_mut(&key) else {
                    return;
                };
                let text = r.unwrap_or_else(|e| format!("Could not read the turn's trees: {e:#}\n"));
                d.editor.update(cx, |s, cx| s.set_value(text, window, cx));
                d.loading = false;
                cx.notify();
            },
        );
        true
    }

    fn render(&self, app: &mut BenchApp, _window: &mut Window, cx: &mut Context<BenchApp>) -> AnyElement {
        let Some(d) = app.explorer.docs.get(&self.key()) else {
            return div().into_any_element();
        };
        let body = if d.loading { DiffBody::Note("Reading…") } else { DiffBody::Diff(d.editor.clone()) };
        diff_pane(
            "turn-diff-open",
            format!("{} · changed in this turn", self.file.name()),
            &self.file,
            body,
            cx,
        )
    }
}

/// The unified diff of `f` between the turn's trees (a file made or removed in the turn is
/// compared with nothing).
pub fn turn_diff(conn: &crate::client::connection::Connection, f: &FileRef, t: &Turn) -> anyhow::Result<String> {
    let machine = f.machine.as_deref();
    let before = rest::git_show(conn, machine, &t.root, &t.base, &f.path)?.unwrap_or_default();
    let after = rest::git_show(conn, machine, &t.root, &t.head, &f.path)?.unwrap_or_default();
    let rel = f.path.strip_prefix(&format!("{}/", t.root.trim_end_matches('/'))).unwrap_or(&f.path);
    let d = super::git::unified(rel, &before, &after);
    Ok(if d.is_empty() { "No changes in text\n".into() } else { d })
}
