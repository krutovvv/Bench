//! A project's settings: its `FORMS` as groups under headings — name and where
//! it lives, how it runs, its Telegram bot, its removal — fields in rows, scrolling as a page. A
//! document of the Explorer (`DocKey::Settings`), the last row of its tree.

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use super::FORMS;
use super::section::SettingsForm;
use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// `project`'s settings, filling the document.
    pub(crate) fn render_project_settings(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let inset = px(bench_ui::CHAT_LEFT);
        let id = id.to_string();
        if !self.envs.contains_key(&id) {
            self.load_env(&id, window, cx);
        }
        let Some(e) = self.envs.get(&id) else {
            return div()
                .p(inset)
                .text_color(theme.muted_foreground)
                .child("Loading…")
                .into_any_element();
        };
        let error = e.error.clone();
        v_flex()
            .id("env-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.env_scroll)
            .px(inset)
            .pb(inset)
            .children(error.map(|err| {
                div()
                    .pt(inset)
                    .text_color(theme.danger)
                    .child(err)
                    .into_any_element()
            }))
            .children(self.render_env_form(&id, cx))
            .into_any_element()
    }

    /// Each group's heading, then its sections' fields.
    fn render_env_form(&self, id: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let Some(e) = self.envs.get(id) else {
            return vec![];
        };
        let general = crate::app::model::ProjectKey::parse(&e.id).is_host();
        let mut groups: Vec<(&'static str, Vec<&'static dyn SettingsForm>)> = vec![];
        for s in FORMS.iter().copied().filter(|s| s.shown(self, e)) {
            let title = s.block(general);
            match groups.last_mut() {
                Some((t, group)) if *t == title => group.push(s),
                _ => groups.push((title, vec![s])),
            }
        }
        let mut out = vec![];
        for (title, group) in groups {
            out.push(
                bench_ui::settings_heading(title, vec![], cx)
                    .flex_none()
                    .into_any_element(),
            );
            let fields: Vec<AnyElement> = group
                .iter()
                .filter_map(|s| s.form(self, e, cx))
                .flatten()
                .collect();
            out.push(
                v_flex()
                    .flex_none()
                    .w_full()
                    .border_t_1()
                    .border_color(theme.border)
                    .children(fields)
                    .into_any_element(),
            );
        }
        out
    }
}

/// A project's settings as a tab of the Explorer (`DocKey::Settings`). Each group has its own
/// Save; its typed fields stay in the environment until saved or reverted.
pub struct SettingsItem(pub String);

impl crate::explorer::item::Item for SettingsItem {
    fn tab(&self, app: &BenchApp, cx: &App) -> (String, (Ico, Hsla)) {
        (
            format!("{} · Settings", app.project_name(&self.0)),
            (Ico::Settings, cx.theme().muted_foreground),
        )
    }

    /// Its fields are the environment's: read once, kept while typed in.
    fn open(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        if !app.envs.contains_key(&self.0) {
            app.load_env(&self.0, window, cx);
        }
        true
    }

    fn render(
        &self,
        app: &mut BenchApp,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> AnyElement {
        app.settings_pane(&self.0, window, cx)
    }
}

impl BenchApp {
    /// A project's settings as a document: its fields, the page `env/page.rs` draws.
    fn settings_pane(
        &mut self,
        project: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = self.project_name(project);
        v_flex()
            .size_full()
            .child(bench_ui::ide_head(
                format!("{name} · Settings"),
                None,
                vec![],
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_project_settings(project, window, cx)),
            )
            .into_any_element()
    }
}
