//! A project's Settings page: its `Home::Settings` sections as groups under headings — name and
//! where it lives, how it runs, its Telegram bot, its removal — fields in rows, scrolling as a
//! page. What the project contains is in the Explorer (`src/explorer/`).

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use super::SECTIONS;
use super::section::{Home, Section};
use crate::app::BenchApp;

impl BenchApp {
    /// The page of `Center::Project`.
    pub(crate) fn render_project_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let inset = px(crate::ui::CHAT_LEFT);
        // The demo opens on the blog's settings (`demo::load`), read here once.
        if self.demo && self.project_env.is_none() {
            self.load_env("p-blog", window, cx);
        }
        let Some(e) = self.project_env.as_ref() else {
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
            .children(self.render_env_form(cx))
            .into_any_element()
    }

    /// Each group's heading, then its sections' fields.
    fn render_env_form(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let general = self.project_env.as_ref().is_some_and(|e| e.id == "general");
        let mut groups: Vec<(&'static str, Vec<&'static dyn Section>)> = vec![];
        for s in SECTIONS
            .iter()
            .copied()
            .filter(|s| s.home() == Home::Settings && s.shown(self))
        {
            let title = s.block(general);
            match groups.last_mut() {
                Some((t, group)) if *t == title => group.push(s),
                _ => groups.push((title, vec![s])),
            }
        }
        let mut out = vec![];
        for (title, group) in groups {
            let heads: Vec<AnyElement> = group.iter().filter_map(|s| s.head(self, cx)).collect();
            out.push(
                bench_ui::settings_heading(title, heads, cx)
                    .flex_none()
                    .into_any_element(),
            );
            let fields: Vec<AnyElement> = group
                .iter()
                .filter_map(|s| s.form(self, cx))
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
