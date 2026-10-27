//! A project's settings: its `Home::Settings` sections as groups under headings — name and where
//! it lives, how it runs, its Telegram bot, its removal — fields in rows, scrolling as a page. A
//! document of the Explorer (`DocKey::Settings`), the last row of its tree.

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use super::SECTIONS;
use super::section::{Home, Section};
use crate::app::BenchApp;

impl BenchApp {
    /// `project`'s settings, filling the document.
    pub(crate) fn render_project_settings(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let inset = px(crate::ui::CHAT_LEFT);
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
        let general = crate::model::ProjectKey::parse(&e.id).is_host();
        let mut groups: Vec<(&'static str, Vec<&'static dyn Section>)> = vec![];
        for s in SECTIONS
            .iter()
            .copied()
            .filter(|s| s.home() == Home::Settings && s.shown(self, e))
        {
            let title = s.block(general);
            match groups.last_mut() {
                Some((t, group)) if *t == title => group.push(s),
                _ => groups.push((title, vec![s])),
            }
        }
        let mut out = vec![];
        for (title, group) in groups {
            let heads: Vec<AnyElement> = group.iter().filter_map(|s| s.head(self, e, cx)).collect();
            out.push(
                bench_ui::settings_heading(title, heads, cx)
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
