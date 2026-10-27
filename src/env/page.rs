//! A project's settings drawn: the gear's tabs on top, then the open tab's groups — a heading
//! with its actions, and under it the fields of a form or a list in a bordered block, the open
//! row's card under its list. Instructions is the editor of `ide.rs` and fills the page.

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::section::{Card, Section, Tab};
use super::{SECTIONS, section};
use crate::app::BenchApp;

impl BenchApp {
    /// The page of `Panel::Project`.
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
        let open = e.tab;
        let error = e.error.clone();
        // A tab with nothing shown for this project (a server's project has no Access) is left
        // out.
        let tabs: Vec<Tab> = Tab::ALL
            .into_iter()
            .filter(|t| SECTIONS.iter().any(|s| s.tab() == *t && s.shown(self)))
            .collect();
        let labels: Vec<_> = tabs.iter().map(|t| t.label()).collect();
        let view = cx.entity();
        let picks = tabs.clone();
        let pick: bench_ui::OnPick = std::rc::Rc::new(move |k, window, cx| {
            let tab = picks[k];
            view.update(cx, |this, cx| this.env_tab(tab, window, cx));
        });
        let at = tabs.iter().position(|t| *t == open).unwrap_or(0);
        let head = div()
            .flex_none()
            .pt(inset)
            .px(inset)
            .flex()
            .child(bench_ui::settings_tabs("env-tabs", &labels, at, cx, pick))
            .when_some(error, |d, err| {
                d.child(div().pt_2().text_color(theme.danger).child(err))
            });
        let body = if open == Tab::Instructions {
            div()
                .flex_1()
                .min_h_0()
                .pt_4()
                .px(inset)
                .pb(inset)
                .child(self.render_env_ide(window, cx))
                .into_any_element()
        } else {
            v_flex()
                .id("env-page")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(inset)
                .pb(inset)
                .children(self.render_env_groups(open, cx))
                .into_any_element()
        };
        v_flex()
            .size_full()
            .child(head)
            .child(body)
            .into_any_element()
    }

    /// The groups of a tab: each section's `block`, its sections' heading actions, their forms
    /// or rows; the open card under the list it belongs to, a new server's or file host's form
    /// under Machines or File hosts.
    fn render_env_groups(&mut self, tab: Tab, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let Some(e) = self.project_env.as_ref() else {
            return vec![];
        };
        let general = e.id == "general";
        let card_kind = e.card.as_ref().map(|(k, _)| *k);
        let shown: Vec<&'static dyn Section> = SECTIONS
            .iter()
            .copied()
            .filter(|s| s.tab() == tab && s.shown(self))
            .collect();
        let mut out = vec![];
        let mut at = 0;
        while at < shown.len() {
            let title = shown[at].block(general);
            let end = shown[at..]
                .iter()
                .position(|s| s.block(general) != title)
                .map_or(shown.len(), |n| at + n);
            let group = &shown[at..end];
            at = end;
            let heads: Vec<AnyElement> = group.iter().filter_map(|s| s.head(self, cx)).collect();
            out.push(bench_ui::settings_heading(title, heads, cx).into_any_element());
            let mut forms = vec![];
            let mut rows = vec![];
            for s in group {
                match s.form(self, cx) {
                    Some(f) => forms.extend(f),
                    None => rows.extend(s.rows(self, cx)),
                }
            }
            if !forms.is_empty() {
                out.push(
                    v_flex()
                        .w_full()
                        .border_t_1()
                        .border_color(theme.border)
                        .children(forms)
                        .into_any_element(),
                );
            }
            if !rows.is_empty() {
                out.push(
                    v_flex()
                        .w_full()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .bg(theme.background)
                        .children(rows)
                        .into_any_element(),
                );
            }
            if card_kind.is_some_and(|k| group.iter().any(|s| s.key() == k)) {
                if let Some(card) = self.render_env_card(cx) {
                    out.push(div().pt_3().child(card).into_any_element());
                }
            }
            // The forms of the accesses' "+ Add" (and a machine's edit), where they were asked.
            let form = match title {
                "File hosts" if self.files_remote.form => Some(self.render_host_form(cx)),
                "Machines" if self.ssh.form.is_some() => Some(self.render_server_form(cx)),
                _ => None,
            };
            if let Some(form) = form {
                out.push(
                    div()
                        .mt_3()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .child(form)
                        .into_any_element(),
                );
            }
        }
        out
    }

    /// The open row's card: its fields, and in its head Save (live once something changed),
    /// Revert, its own actions, Remove (twice) and Close.
    fn render_env_card(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let (kind, name) = self.project_env.as_ref()?.card.clone()?;
        let Card {
            title,
            body,
            actions,
        } = section(kind)?.card(self, &name, cx)?;
        let close =
            crate::inserts::icon_button("env-card-close", crate::icons::Ico::X, "Close", &theme)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.env_card(kind, name.clone(), window, cx)
                }));
        let head = crate::ui::block_head(&theme, true)
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.foreground)
                    .child(title),
            )
            .children(actions)
            .child(close);
        Some(crate::inserts::card(head, body, &theme))
    }
}
