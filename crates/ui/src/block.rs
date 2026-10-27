//! A block: the bordered, rounded box every insert, card, code block and pane lives in.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::{AVATAR, BLOCK_HEAD_H};

/// A block's head: one height, a faint ground, small muted text. `over_body`: a rule under it.
pub fn block_head(theme: &Theme, over_body: bool) -> Div {
    h_flex()
        .flex_none()
        .h(px(BLOCK_HEAD_H))
        .px_3()
        .gap_2()
        .items_center()
        .bg(theme.muted.opacity(0.35))
        .when(over_body, |d| d.border_b_1().border_color(theme.border))
        .text_base()
        .text_color(theme.muted_foreground)
}

/// A whole block: the frame, a head with `title` and `actions` on the right, then `body`.
pub fn block(
    title: impl Into<SharedString>,
    actions: Vec<AnyElement>,
    body: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .child(
            block_head(theme, body.is_some())
                .child(div().flex_1().min_w_0().truncate().child(title.into()))
                .children(actions),
        )
        .children(body)
}

/// The padding inside a block's body.
pub fn block_body() -> Div {
    v_flex().w_full().p_3().gap_2()
}

/// Who speaks, in the thread's gutter: the person a dark disc, the agent the accent with a bot.
pub fn avatar(agent: bool, theme: &Theme) -> Div {
    div()
        .flex_none()
        .size(px(AVATAR))
        .mt(px(-0.5))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .map(|d| {
            if agent {
                d.bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .child(Ico::Bot.icon().xsmall())
            } else {
                d.bg(theme.foreground.opacity(0.78))
                    .text_color(theme.background)
                    .child(Ico::UserRound.icon().xsmall())
            }
        })
}

/// What an empty area says: one muted line.
pub fn empty_state(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

/// A foldable block's head, the same in every block of the thread (an insert, the background
/// processes, the changed files): the chevron, an icon, the title in the head's plain text; what
/// follows (counts, actions) is the caller's.
pub fn fold_head(
    id: impl Into<ElementId>,
    open: bool,
    icon: Option<Ico>,
    title: impl Into<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let fg = theme.foreground;
    block_head(theme, open)
        .id(id)
        .gap_1p5()
        .cursor_pointer()
        .hover(move |s| s.text_color(fg))
        .child(
            if open {
                Ico::ChevronDown
            } else {
                Ico::ChevronRight
            }
            .icon()
            .xsmall(),
        )
        .children(icon.map(|i| i.icon().xsmall()))
        .child(div().flex_none().child(title.into()))
}
