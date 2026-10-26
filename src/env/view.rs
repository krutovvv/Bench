//! The environment drawn: its blocks (one per `Section::block`, the rows of its sections), and
//! the open card under the block it belongs to.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::section::Card;
use super::{SECTIONS, section};
use crate::app::BenchApp;

impl BenchApp {
    /// A project's environment as rows in blocks (the task list's look: a line between rows, a
    /// row lit under the pointer); a row's click opens its card under the block, as the agent's
    /// answer (`render_env_card`), where it is edited, saved or removed. Each block with its
    /// title, for the card to find its place.
    pub(crate) fn render_env_sections(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<(&'static str, AnyElement)> {
        let _ = window;
        let theme = cx.theme().clone();
        let Some(e) = self.project_env.as_ref() else {
            return vec![(
                "",
                div()
                    .text_color(theme.muted_foreground)
                    .child("Loading…")
                    .into_any_element(),
            )];
        };
        let general = e.id == "general";
        let folded = e.folded.clone();
        let error = e.error.clone();
        let mut blocks = vec![];
        let mut at = 0;
        while at < SECTIONS.len() {
            let title = SECTIONS[at].block(general);
            let end = SECTIONS[at..]
                .iter()
                .position(|s| s.block(general) != title)
                .map_or(SECTIONS.len(), |n| at + n);
            let shown: Vec<_> = SECTIONS[at..end].iter().filter(|s| s.shown(self)).collect();
            at = end;
            if shown.is_empty() {
                continue;
            }
            let mut rows = vec![];
            let mut extra = None;
            for s in shown {
                rows.extend(s.rows(self, cx));
                if extra.is_none() {
                    extra = s.head(self, cx);
                }
            }
            let shut = folded.contains(title);
            blocks.push((title, block(title, shut, extra, rows, cx)));
        }
        if let Some(err) = error {
            blocks.push((
                "",
                div().text_color(theme.danger).child(err).into_any_element(),
            ));
        }
        blocks
    }

    /// The block the open card belongs to (it shows under it).
    pub(crate) fn env_card_section(&self) -> Option<&'static str> {
        let e = self.project_env.as_ref()?;
        let general = e.id == "general";
        Some(section(e.card.as_ref()?.0)?.block(general))
    }

    /// The open row's card, under its block as the agent's answer: its fields, and in its head
    /// Save (live once something changed), Revert, its own actions, Remove (twice) and Close.
    pub(crate) fn render_env_card(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let (kind, name) = self.project_env.as_ref()?.card.clone()?;
        let Card {
            title,
            body,
            actions,
            follow,
        } = section(kind)?.card(self, &name, cx)?;
        let close =
            crate::inserts::icon_button("env-card-close", crate::icons::Ico::X, "Close", &theme)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
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
        let card = crate::inserts::card(head, body, &theme);
        if follow.is_empty() {
            return Some(card);
        }
        Some(
            v_flex()
                .w_full()
                .gap_4()
                .child(card)
                .children(follow)
                .into_any_element(),
        )
    }
}

/// A block: its head (a click folds it) with the title and what its section adds, its rows.
fn block(
    title: &'static str,
    shut: bool,
    extra: Option<AnyElement>,
    rows: Vec<AnyElement>,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let theme = cx.theme().clone();
    let head = crate::ui::block_head(&theme, !shut)
        .id(SharedString::from(format!("env-sec-{title}")))
        .cursor_pointer()
        .child(
            if shut {
                crate::icons::Ico::ChevronRight
            } else {
                crate::icons::Ico::ChevronDown
            }
            .icon()
            .xsmall(),
        )
        .child(div().text_color(theme.foreground).child(title))
        .children(extra)
        .child(div().flex_1())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.click_claimed = true;
            if let Some(e) = this.project_env.as_mut() {
                if !e.folded.remove(title) {
                    e.folded.insert(title);
                }
            }
            cx.notify();
        }));
    v_flex()
        .w_full()
        .border_1()
        .rounded_md()
        .border_color(theme.border)
        .overflow_hidden()
        .bg(theme.background)
        .child(head)
        .when(!shut, |d| d.children(rows))
        .into_any_element()
}
