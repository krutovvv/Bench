//! A block: the bordered, rounded box every insert, card, code block and pane lives in.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::{AVATAR, BLOCK_HEAD_H, CHAT_LEFT, CHAT_RIGHT, CHAT_TEXT, CODE_FOLD_H, LIST_ROW_H, TURN_Y};

/// A block's head: one height, on the block's own ground (a filled head, a rule and the frame
/// were three lines around every block), muted text a step under the messages'. `over_body`: a
/// rule under it, the frame's colour made lighter.
pub fn block_head(theme: &Theme, over_body: bool) -> Div {
    h_flex()
        .flex_none()
        .h(px(BLOCK_HEAD_H))
        .px_3()
        .gap_2()
        .items_center()
        .when(over_body, |d| d.border_b_1().border_color(theme.border.opacity(0.6)))
        .text_size(px(crate::metrics::THREAD_META))
        .text_color(theme.muted_foreground)
}

/// A block's frame: the page's ground, a soft hairline, a card's corners; what is inside is cut
/// to them. Every block of the thread is one (an insert, a card, the processes, the changed files).
pub fn block_frame(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .min_w_0()
        .bg(theme.background)
        .border_1()
        // A Mac's grouped box (System Settings', Xcode's inspectors): a softer hairline and the
        // larger corner a card has, where a field keeps the small one.
        .border_color(theme.border.opacity(0.7))
        .rounded_lg()
        .overflow_hidden()
}

/// A whole block: the frame, a head with `title` and `actions` on the right, then `body`.
pub fn block(
    title: impl Into<SharedString>,
    actions: Vec<AnyElement>,
    body: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    block_frame(cx)
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

/// A turn of the thread: who speaks in the gutter (`Some(true)` the agent, `Some(false)` the
/// person, `None` the gutter left empty), what they say in the text column.
pub fn turn_row(agent: Option<bool>, body: impl IntoElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .pl(px(CHAT_LEFT))
        .pr(px(CHAT_RIGHT))
        .py(px(TURN_Y))
        .gap(px(CHAT_TEXT - CHAT_LEFT - AVATAR))
        .items_start()
        .text_base()
        .child(match agent {
            Some(agent) => avatar(agent, theme).into_any_element(),
            None => div().flex_none().w(px(AVATAR)).into_any_element(),
        })
        .child(div().flex_1().min_w_0().child(body))
}

/// A turn's row without an avatar's gutter (the chat): the body between the thread's margins.
pub fn turn_row_plain(body: impl IntoElement, _cx: &App) -> Div {
    h_flex()
        .w_full()
        .px(px(CHAT_LEFT))
        .py(px(TURN_Y))
        .items_start()
        .text_base()
        .child(div().flex_1().min_w_0().child(body))
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
    crate::button::own_press(block_head(theme, open))
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

/// What a fold's line says: "Show less" when open, else what unfolding shows.
pub fn fold_words(open: bool, shut: impl Into<SharedString>) -> SharedString {
    if open { "Show less".into() } else { shut.into() }
}

/// The line that unfolds what was cut: muted words that darken under the pointer. Its size of
/// text and its click are the caller's.
pub fn fold_more(id: impl Into<ElementId>, words: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let fg = theme.foreground;
    crate::button::own_press(div())
        .id(id)
        .text_color(theme.muted_foreground)
        .cursor_pointer()
        .hover(move |s| s.text_color(fg))
        .child(words.into())
}

/// `fold_more` as a block's foot: the block's inset and a rule over it.
pub fn fold_foot(id: impl Into<ElementId>, words: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let border = cx.theme().border;
    fold_more(id, words, cx).px_3().py_1().border_t_1().border_color(border)
}

/// `fold_more` as a row of a block's list (the changed files): the rows' height and inset.
pub fn fold_row(id: impl Into<ElementId>, words: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    fold_more(id, words, cx)
        .flex()
        .items_center()
        .h(px(LIST_ROW_H))
        .px_3()
}

/// A block of code, an answer's or a step card's: the frame, a head with its language or name on
/// the left and `action` on the right, the code (cut to its first lines while `folded`) and the
/// line that unfolds it.
pub fn code_frame(
    title: impl Into<SharedString>,
    action: impl IntoElement,
    body: impl IntoElement,
    folded: bool,
    more: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    block_frame(cx)
        .child(
            // As tall as the changed-files card's line: small text, 4 above and below.
            block_head(theme, true)
                .justify_between()
                .child(title.into())
                .child(action),
        )
        .child(
            div()
                .when(folded, |d| d.max_h(px(CODE_FOLD_H)).overflow_hidden())
                .child(body),
        )
        .children(more)
}

/// A command or a path shown as it will run, inside a card (an approval's): the code's font on
/// the bubble's grey, no frame.
pub fn code_note(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .p_2()
        .bg(cx.theme().secondary)
        .font_family(crate::theme::MONO)
        .text_size(px(crate::metrics::THREAD_META))
        .child(text.into())
}

/// A code editor set among a form's fields: a field's corner and border round it.
pub fn editor_frame(cx: &App) -> Div {
    div().border_1().rounded_md().border_color(cx.theme().border)
}

/// One part of a panel hung under a chip (a popover): its inset, a rule above when it is not the
/// first, so the rules run edge to edge. The panel itself has no padding.
pub fn popover_section(ruled: bool, cx: &App) -> Div {
    let border = cx.theme().border.opacity(0.6);
    v_flex()
        .px_3()
        .py_2()
        .when(ruled, |d| d.border_t_1().border_color(border))
}

/// A popover section's title, a label over its lines as a sidebar's group head.
pub fn popover_title(title: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(crate::metrics::SIDEBAR_HEAD_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(title.into())
}
