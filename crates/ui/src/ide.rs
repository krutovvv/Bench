//! The Explorer's pieces (`src/explorer/`): the rows and headings of a tree, an open document's
//! head, its tab, ⌘P's list, the right dock's width. Documents fill their pane and scroll inside,
//! as in an IDE.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::BLOCK_HEAD_H;

/// How far a level of the tree is set in.
pub const TREE_INDENT: f32 = 14.;

/// Where a row of the tree `depth` deep starts.
pub fn tree_pl(depth: usize) -> Pixels {
    px(8. + TREE_INDENT * depth as f32)
}

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
/// color, the name and a muted note after it; lit when chosen; `actions` on hover.
#[allow(clippy::too_many_arguments)]
pub fn tree_row(
    id: impl Into<SharedString>,
    depth: usize,
    chevron: Option<bool>,
    icon: (Ico, Hsla),
    name: impl Into<SharedString>,
    note: Option<SharedString>,
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
        .pl(tree_pl(depth))
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
        .child(div().min_w_0().truncate().child(name.into()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(theme.muted_foreground)
                .children(note),
        )
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

/// A tab of an open document in the header: its icon, name, a dot while it has unsaved changes,
/// its × (on hover, always on the open one).
pub fn doc_tab(
    id: impl Into<SharedString>,
    icon: (Ico, Hsla),
    name: impl Into<SharedString>,
    open: bool,
    dirty: bool,
    close: Stateful<Div>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    let group: SharedString = format!("{id}-g").into();
    h_flex()
        .id(ElementId::Name(id))
        .group(group.clone())
        .flex_none()
        .h_full()
        .pl_3()
        .pr_1()
        .gap_1p5()
        .items_center()
        .text_sm()
        .cursor_pointer()
        .border_r_1()
        .border_color(theme.border)
        .when(open, |d| {
            d.bg(theme.background).text_color(theme.foreground)
        })
        .when(!open, |d| {
            d.text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
        })
        .child(icon.0.icon().small().text_color(icon.1))
        .child(div().max_w(px(TAB_MAX_W)).truncate().child(name.into()))
        .when(dirty, |d| {
            d.child(div().size(px(6.)).rounded_full().bg(theme.foreground))
        })
        .child(
            div()
                .when(!open, |d| {
                    d.opacity(0.).group_hover(group, |s| s.opacity(1.))
                })
                .child(close),
        )
}

/// ⌘P's list over the middle: its width, and how tall it grows.
pub const QUICK_W: f32 = 560.;
pub const QUICK_MAX_H: f32 = 420.;

/// A dock's size (a side dock's width, the bottom dock's height): at first, and how far it can be
/// dragged.
#[derive(Clone, Copy, Debug)]
pub struct DockSize {
    pub start: f32,
    pub min: f32,
    pub max: f32,
}

/// The left dock: the threads, the Explorer's tree.
pub const LEFT_DOCK: DockSize = DockSize {
    start: 280.,
    min: 200.,
    max: 480.,
};
/// The right dock: the thread beside a document.
pub const RIGHT_DOCK: DockSize = DockSize {
    start: 520.,
    min: 280.,
    max: 1000.,
};
/// The bottom dock, under the center: a terminal.
pub const BOTTOM_DOCK: DockSize = DockSize {
    start: 260.,
    min: 120.,
    max: 640.,
};

/// A tab's name is cut past this.
pub const TAB_MAX_W: f32 = 180.;
