//! A small editor of a few texts, as a settings page has it: a tree of them on the left, the open
//! one on the right under its head. The page around it does not scroll: the tree and the editor
//! scroll each on their own, as in an IDE.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::BLOCK_HEAD_H;

/// The tree's column.
pub const IDE_TREE_W: f32 = 260.;
/// A list's column (a name and a line under it, so wider than a tree).
pub const LIST_W: f32 = 340.;
/// How far a level of the tree is set in.
pub const TREE_INDENT: f32 = 14.;

/// A group's title in the tree ("Prompts", "This project"): small, muted, its actions on the
/// right, always shown.
pub fn tree_heading(title: impl Into<SharedString>, actions: Vec<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    let title: SharedString = title.into();
    h_flex()
        .flex_none()
        .h(px(BLOCK_HEAD_H))
        .px_3()
        .gap_1()
        .items_center()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(title.to_uppercase()),
        )
        .children(actions)
}

/// A row of the tree: set in by `depth`, a chevron when it opens (`Some(open)`), its icon in its
/// color, the name; lit when chosen; `actions` on hover.
#[allow(clippy::too_many_arguments)]
pub fn tree_row(
    id: impl Into<SharedString>,
    depth: usize,
    chevron: Option<bool>,
    icon: (Ico, Hsla),
    name: impl Into<SharedString>,
    chosen: bool,
    actions: Vec<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    let group: SharedString = format!("{id}-g").into();
    h_flex()
        .id(ElementId::Name(id))
        .group(group.clone())
        .flex_none()
        .pl(px(8. + TREE_INDENT * depth as f32))
        .pr_2()
        .py_1()
        .gap_1p5()
        .items_center()
        .text_sm()
        .cursor_pointer()
        .when(chosen, |d| d.bg(theme.list_active))
        .when(!chosen, |d| d.hover(|s| s.bg(theme.list_hover)))
        .child(
            div()
                .w(px(12.))
                .flex_none()
                .text_color(theme.muted_foreground)
                .when_some(chevron, |d, open| {
                    d.child(
                        if open {
                            Ico::ChevronDown
                        } else {
                            Ico::ChevronRight
                        }
                        .icon()
                        .xsmall(),
                    )
                }),
        )
        .child(icon.0.icon().small().text_color(icon.1))
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
        .when(!actions.is_empty(), |d| {
            d.child(
                h_flex()
                    .flex_none()
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .children(actions),
            )
        })
}

/// The editor's head: the open text's name and where it is, its actions on the right.
pub fn ide_head(
    title: impl Into<SharedString>,
    note: Option<SharedString>,
    actions: Vec<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .h(px(BLOCK_HEAD_H))
        .px_3()
        .gap_3()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.muted.opacity(0.35))
        .text_sm()
        .child(div().flex_none().child(title.into()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(theme.muted_foreground)
                .children(note),
        )
        .children(actions)
}

/// Tree (or list) `tree_w` wide | the open one, in one bordered, rounded block filling the room it
/// is given.
pub fn ide(tree_w: f32, tree: AnyElement, head: AnyElement, body: AnyElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .size_full()
        .items_start()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .child(
            div()
                .id("ide-tree")
                .w(px(tree_w))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .border_r_1()
                .border_color(theme.border)
                .bg(theme.sidebar)
                .py_1()
                .child(tree),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(head)
                .child(div().flex_1().min_h_0().child(body)),
        )
}
