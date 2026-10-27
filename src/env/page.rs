//! A project's settings drawn: the gear's tabs on top, then the open tab. General is a form —
//! groups under headings, fields in rows — and scrolls as a page. The others are two panes
//! filling the page, as in an editor: on the left what there is (Instructions' tree of texts in
//! `ide.rs`; the lists of servers, jobs and accesses here), on the right the one chosen, its
//! fields and actions.

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
        let head = v_flex()
            .flex_none()
            .pt(inset)
            .px(inset)
            .child(bench_ui::settings_tabs("env-tabs", &labels, at, cx, pick))
            .when_some(error, |d, err| {
                d.child(div().pt_2().text_color(theme.danger).child(err))
            });
        let body = match open {
            Tab::General => v_flex()
                .id("env-page")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(inset)
                .pb(inset)
                .children(self.render_env_form(open, cx))
                .into_any_element(),
            _ => div()
                .flex_1()
                .min_h_0()
                .pt_4()
                .px(inset)
                .pb(inset)
                .child(if open == Tab::Instructions {
                    self.render_env_ide(window, cx)
                } else {
                    self.render_env_lists(open, cx)
                })
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(head)
            .child(body)
            .into_any_element()
    }

    /// The sections of a tab that are shown, in runs of one `block` each.
    fn env_groups(&self, tab: Tab) -> Vec<(&'static str, Vec<&'static dyn Section>)> {
        let general = self.project_env.as_ref().is_some_and(|e| e.id == "general");
        let mut out: Vec<(&'static str, Vec<&'static dyn Section>)> = vec![];
        for s in SECTIONS
            .iter()
            .copied()
            .filter(|s| s.tab() == tab && s.shown(self))
        {
            let title = s.block(general);
            match out.last_mut() {
                Some((t, group)) if *t == title => group.push(s),
                _ => out.push((title, vec![s])),
            }
        }
        out
    }

    /// A form tab: each group's heading, then its sections' fields.
    fn render_env_form(&mut self, tab: Tab, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let mut out = vec![];
        for (title, group) in self.env_groups(tab) {
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

    /// A list tab: on the left each group's heading ("+ Add" and the like) and its rows; on the
    /// right what is open — a row's fields, a new one's form, a new server's or file host's form.
    fn render_env_lists(&mut self, tab: Tab, cx: &mut Context<Self>) -> AnyElement {
        let mut left = vec![];
        let mut keys = vec![];
        for (title, group) in self.env_groups(tab) {
            let heads: Vec<AnyElement> = group.iter().filter_map(|s| s.head(self, cx)).collect();
            left.push(bench_ui::tree_heading(title, heads, cx).into_any_element());
            for s in group {
                keys.push(s.key());
                left.extend(s.rows(self, cx));
            }
        }
        let list = v_flex().w_full().children(left).into_any_element();
        let (head, body) = self.render_env_open(tab, &keys, cx);
        bench_ui::ide(bench_ui::LIST_W, list, head, body, cx).into_any_element()
    }

    /// The right pane of a list tab: the open row (`keys`: the tab's sections) or the access form
    /// being filled; else a line saying what to do.
    fn render_env_open(
        &mut self,
        tab: Tab,
        keys: &[&str],
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement) {
        let theme = cx.theme().clone();
        let scroll = |body: AnyElement| {
            div()
                .id("env-open")
                .size_full()
                .overflow_y_scroll()
                .child(body)
                .into_any_element()
        };
        // The forms of the accesses' "+ Add" (and a machine's edit) carry their own title and
        // buttons.
        let form = if tab != Tab::Access {
            None
        } else if self.files_remote.form {
            Some(self.render_host_form(cx))
        } else if self.ssh.form.is_some() {
            Some(self.render_server_form(cx))
        } else {
            None
        };
        if let Some(form) = form {
            return (
                bench_ui::ide_head("", None, vec![], cx).into_any_element(),
                scroll(div().p_3().child(form).into_any_element()),
            );
        }
        let open = self
            .project_env
            .as_ref()
            .and_then(|e| e.card.clone())
            .filter(|(k, _)| keys.contains(k));
        let card = open.and_then(|(kind, name)| section(kind)?.card(self, &name, cx));
        match card {
            Some(Card {
                title,
                body,
                actions,
            }) => (
                bench_ui::ide_head(title, None, actions, cx).into_any_element(),
                scroll(body),
            ),
            None => (
                bench_ui::ide_head("", None, vec![], cx).into_any_element(),
                div()
                    .p_3()
                    .text_color(theme.muted_foreground)
                    .child("Choose one on the left, or + Add a new one")
                    .into_any_element(),
            ),
        }
    }
}
