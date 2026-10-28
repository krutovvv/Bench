//! The Explorer's pieces (`src/explorer/`): the rows and headings of a tree, an open document's
//! head, its tab, the quick list (⌘P, ⌘K), the right dock's width. Documents fill their pane and scroll inside,
//! as in an IDE.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
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

/// A group's title in the tree ("Agent", "Project"): a section header as the threads panel's
/// (11 pt semibold, secondary, in its own case), its actions on the right, always shown.
pub fn tree_heading(title: impl Into<SharedString>, actions: Vec<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    let title: SharedString = title.into();
    h_flex()
        .flex_none()
        .h(px(BLOCK_HEAD_H))
        .px_3()
        .gap_1()
        .items_center()
        .text_size(px(crate::metrics::SIDEBAR_HEAD_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(title),
        )
        .children(actions)
}

/// A row of the tree: set in by `depth`, a chevron when it opens (`Some(open)`), its icon in its
/// color, the name and a muted note after it; lit when chosen; `dim`: git ignores this path — a
/// faded icon and name; `actions` on hover.
#[allow(clippy::too_many_arguments)]
pub fn tree_row(
    id: impl Into<SharedString>,
    depth: usize,
    chevron: Option<bool>,
    icon: (Ico, Hsla),
    name: impl Into<SharedString>,
    note: Option<SharedString>,
    dim: bool,
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
        // The left dock's one size, the sidebar's: the tree is a list too.
        .text_size(px(crate::metrics::SIDEBAR_TEXT))
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
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .when(dim, |d| d.opacity(0.5))
                .child(icon.0.icon().small().text_color(icon.1)),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(dim, |d| d.text_color(theme.muted_foreground).opacity(0.5))
                .child(name.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(crate::text::faint(&theme))
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
        .border_color(theme.border.opacity(0.6))
        // A pane's title as Xcode's: the bar's title size, semibold; its note quieter.
        .text_size(px(crate::metrics::BAR_TEXT))
        .child(
            div()
                .flex_none()
                .max_w_2_3()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(crate::text::faint(theme))
                .children(note),
        )
        .children(actions)
}

/// A band under an open document's editor, a field in it: a comment being written on its
/// lines (Xcode's find bar has the same place). `label` says what it is about.
pub fn ide_band(label: impl Into<SharedString>, field: AnyElement, actions: Vec<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .min_h(px(BLOCK_HEAD_H))
        .px_3()
        .py_1()
        .gap_3()
        .items_center()
        .border_t_1()
        .border_color(theme.border.opacity(0.6))
        .bg(theme.title_bar)
        .text_size(px(crate::metrics::BAR_TEXT))
        .child(
            div()
                .flex_none()
                .text_color(crate::text::faint(theme))
                .child(label.into()),
        )
        .child(div().flex_1().min_w_0().child(field))
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
/// A spec's tab: its text's column, as a document reads (Xcode's documentation viewer).
pub const SPEC_COLUMN_W: f32 = 760.;
/// The width of a spec's link labels ("Under", "Referred to by").
pub const SPEC_LINK_LABEL_W: f32 = 110.;
pub const QUICK_MAX_H: f32 = 420.;

/// ⌘K's list: how far under the window's top it opens, and how wide a row's name may grow
/// before the quieter words after it.
pub const SEARCH_TOP: f32 = 96.;
pub const SEARCH_NAME_W: f32 = 320.;

/// A list over the window to choose from as one types (⌘P's files, ⌘K's threads and projects):
/// a popover's box `QUICK_W` wide, growing to `QUICK_MAX_H`; its field and rows are the caller's.
pub fn quick_frame(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    v_flex()
        .id(id)
        .w(px(QUICK_W))
        .max_h(px(QUICK_MAX_H))
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .shadow_lg()
        .overflow_hidden()
}

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

/// A terminal tab's height in the bottom dock's head.
pub const TERM_TAB_H: f32 = 22.;

/// A tab of the bottom dock's terminals: its label, dimmed once its shell ended (a click starts
/// it anew), its × on hover (always on the shown one); the shown one sits on lit ground.
pub fn term_tab(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    open: bool,
    ended: bool,
    close: Stateful<Div>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    let group: SharedString = format!("{id}-g").into();
    // A segment of the bar's segmented control (as Chat | Code, Files | Threads): the shown one
    // raised on the grey track (`segment_track`), the others its muted text.
    let fg = theme.foreground;
    h_flex()
        .id(ElementId::Name(id))
        .group(group.clone())
        .flex_none()
        .h(px(crate::metrics::SEGMENT_H))
        .pl_2p5()
        .pr_0p5()
        .gap_0p5()
        .items_center()
        .rounded_sm()
        .text_size(px(crate::metrics::BAR_TEXT))
        .cursor_pointer()
        .when(open, |d| {
            d.bg(theme.background).shadow_xs().text_color(theme.foreground)
        })
        .when(!open, |d| {
            d.text_color(if ended {
                crate::faint(&theme)
            } else {
                theme.muted_foreground
            })
            .hover(move |s| s.text_color(fg))
        })
        .child(div().max_w(px(TAB_MAX_W)).truncate().child(name.into()))
        .child(
            div()
                .when(!open, |d| {
                    d.opacity(0.).group_hover(group, |s| s.opacity(1.))
                })
                .child(close),
        )
}
