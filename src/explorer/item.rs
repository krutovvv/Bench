//! A document of the middle, as Zed's `Item`: what its tab shows, whether it has unsaved changes,
//! what opening it reads, how it is drawn, saved and closed. `DocKey` is a tab's identity (kept,
//! compared, saved between starts); `DocKey::item` gives its `Item` — the one place that tells
//! the kinds apart. A new kind of document: a `DocKey` variant, its `Item` beside its own code,
//! a line in `item()` and in `persistence.rs`.

use gpui_kit::*;

use super::DocKey;
use crate::app::BenchApp;
use bench_ui::icons::Ico;

pub trait Item {
    /// Its tab: the name and the icon in its color.
    fn tab(&self, app: &BenchApp, cx: &App) -> (String, (Ico, Hsla));
    /// Changes not saved: its tab's dot, and a close asked once.
    fn dirty(&self, _app: &BenchApp, _cx: &App) -> bool {
        false
    }
    /// It is opened (again): read what it shows. False: it may not open now (it said why).
    fn open(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool;
    /// Its pane in the middle: its head and its body.
    fn render(
        &self,
        app: &mut BenchApp,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> AnyElement;
    /// ⌘S (nothing for what is only read, or saves by its own groups).
    fn save(&self, _app: &mut BenchApp, _window: &mut Window, _cx: &mut Context<BenchApp>) {}
    /// Its tab was closed.
    fn closed(&self, _app: &mut BenchApp, _cx: &mut Context<BenchApp>) {}
}

impl DocKey {
    /// The document this tab holds.
    pub fn item(&self) -> Box<dyn Item> {
        match self.clone() {
            DocKey::File(f) => Box::new(super::file::FileItem(f)),
            DocKey::Diff(f) => Box::new(super::git::DiffItem(f)),
            DocKey::Settings { project } => Box::new(crate::env::page::SettingsItem(project)),
            DocKey::Part {
                project,
                kind,
                name,
            } => Box::new(super::parts::PartItem {
                project,
                kind,
                name,
            }),
            DocKey::Problems { project } => Box::new(super::problems::ProblemsItem(project)),
        }
    }
}
