//! House-style building blocks, the same as Bench's (`~/Projects/bench`). Copy this file
//! into `src/ui/kit.rs` of an app made by `scripts/new-app.sh`, add `pub mod kit;` to `src/ui/mod.rs`,
//! and build views from these functions instead of styling elements yourself.
//!
//! Click handlers: pass `cx.listener(|this, _, window, cx| this.some_action(window, cx))`.
//! Every element that is clicked needs a unique id: `"name"` or `("name", index)`.
// Not every app uses every block.
#![allow(dead_code)]

use std::rc::Rc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;

/// A click handler, as `cx.listener(...)` makes it.
pub type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
/// A handler that gets the index of what was clicked (tabs, segmented cells).
pub type OnPick = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

// ---- sizes ------------------------------------------------------------------------------

/// Every bar is as tall as the header: 36 with its 1px rule. `bar_h` is the 35 inside; a bar
/// adds the rule it draws (the bottom button, a composer: `bar_h + 1`, the search band with its
/// two rules: `bar_h + 1`). 35 at the 14px font.
pub fn bar_h(cx: &App) -> Pixels {
    cx.theme().font_size * 2.5
}

/// Left padding of a row nested under a group header (its text lines up with the header's
/// name, past the icon).
pub fn nest_pl(cx: &App) -> Pixels {
    cx.theme().font_size * 2.375 - px(4.)
}

// ---- buttons ----------------------------------------------------------------------------

/// The one filled button: the main action at the bottom of the sidebar ("New", "Open").
/// Use `.flex_1()` on it to fill the bar. Square, accent color, 2 rem high.
pub fn primary_button(
    id: impl Into<ElementId>,
    icon: Ico,
    label: impl Into<SharedString>,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme();
    h_flex()
        .id(id)
        .h(rems(2.))
        .px_3()
        .gap_2()
        .items_center()
        .justify_center()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .bg(theme.primary)
        .text_color(theme.primary_foreground)
        .cursor_pointer()
        .hover(|s| s.bg(theme.primary_hover))
        .child(icon.icon().small())
        .child(label.into())
        .on_click(on_click)
}

/// A small square icon button for the header (right side) and inside rows. `active` = the
/// thing it toggles is on (accent color on a light background).
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    active: bool,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    div()
        .id(id)
        .flex_none()
        .p_1()
        .cursor_pointer()
        .text_color(if active {
            theme.primary
        } else {
            theme.muted_foreground
        })
        .when(active, |d| d.bg(theme.list_active))
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.icon().small())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        .on_click(on_click)
}

/// A button in the status bar: full bar height, extra-small icon, one muted tone.
pub fn status_button(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    active: bool,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    h_flex()
        .id(id)
        .h_full()
        .px_2()
        .items_center()
        .cursor_pointer()
        .when(active, |d| d.text_color(theme.primary))
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.icon().xsmall())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        .on_click(on_click)
}

/// Delete without a dialog: the first click arms (the button turns red and says so), the
/// second one deletes. Keep `armed` in your state; clear it when anything else is clicked.
pub fn arm_delete_button(
    id: impl Into<ElementId>,
    armed: bool,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    h_flex()
        .id(id)
        .flex_none()
        .px_1p5()
        .py_0p5()
        .gap_1()
        .items_center()
        .text_xs()
        .cursor_pointer()
        .map(|d| {
            if armed {
                d.bg(theme.danger)
                    .text_color(theme.danger_foreground)
                    .child(Ico::Trash.icon().xsmall())
                    .child("Confirm delete")
            } else {
                d.text_color(theme.muted_foreground)
                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                    .child(Ico::Trash.icon().xsmall())
            }
        })
        .on_click(on_click)
}

// ---- bars -------------------------------------------------------------------------------

/// The strip over the sidebar, beside the window buttons: 35, so with the search band's top
/// rule it is the header's 36 and the two rules are one line across the window. The mode
/// toggles (`mode_toggle`) sit on its right, at the sidebar's 16px inset.
pub fn sidebar_top(toggles: Vec<AnyElement>) -> Div {
    h_flex()
        .h(px(35.))
        .flex_none()
        .justify_end()
        .items_center()
        .gap_1()
        // The icon's 4px box padding + 12 = the sidebar's 16px inset.
        .pr(px(12.))
        .children(toggles)
}

/// A mode's switch in `sidebar_top`: its icon, lit (accent on the active row color) while the
/// mode is on. `tip` is "Tools  ⌘3"; while on it reads "Back  ⌘3".
pub fn mode_toggle(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    on: bool,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .p_1()
        .cursor_pointer()
        .text_color(if on {
            theme.primary
        } else {
            theme.muted_foreground
        })
        .when(on, |d| d.bg(theme.list_active))
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.icon().small())
        .tooltip(move |window, cx| {
            let text = match (on, tip.rsplit_once("  ")) {
                (true, Some((_, keys))) => format!("Back  {keys}"),
                _ => tip.to_string(),
            };
            Tooltip::new(text).build(window, cx)
        })
        .on_click(on_click)
}

/// The sidebar's search as a band under `sidebar_top`: the input on the background color
/// between two rules, as tall as the header.
pub fn search_band(input: &Entity<InputState>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .w_full()
        .h(bar_h(cx) + px(1.))
        // The input's own padding plus the icon's side bearing put the icon at the 16 inset.
        .pl(px(3.5))
        .pr_2()
        .items_center()
        .bg(theme.background)
        .border_t_1()
        .border_b_1()
        .border_color(theme.sidebar_border)
        .child(
            Input::new(input)
                .appearance(false)
                .cleanable(true)
                .prefix(Ico::Search.icon().small().text_color(theme.muted_foreground)),
        )
}

/// The bottom of the sidebar: the primary button filling a bar as tall as the composer with
/// its top rule, so the button's top edge and that rule are one line across the window.
pub fn bottom_bar(button: Stateful<Div>, cx: &App) -> Div {
    h_flex()
        .flex_none()
        .h(bar_h(cx) + px(1.))
        .child(button.flex_1().h_full())
}

/// The header over the main area: 36px, title on the left (medium weight, cut with "…"),
/// `right` (icon buttons) on the right. Put it at the top of the main column.
pub fn header_bar(title: impl Into<SharedString>, right: AnyElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .h(px(36.))
        .flex_none()
        .bg(theme.title_bar)
        .border_b_1()
        .border_color(theme.border)
        .pl_4()
        .pr_2()
        .gap_1()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(title.into()),
        )
        .child(right)
}

/// A piece of text in the status bar (left side): a fact, not a control.
pub fn status_text(text: impl Into<SharedString>) -> Div {
    h_flex()
        .h_full()
        .px_2()
        .gap_1()
        .items_center()
        .child(text.into())
}

// ---- sidebar ----------------------------------------------------------------------------

/// A group header in the sidebar, like a folder in Zed's file tree: kind icon, name, a fold
/// chevron on the right (always shown when folded, on hover when open), then an optional "+"
/// that is always visible. Clicking the header folds/unfolds.
pub fn group_header(
    id: impl Into<SharedString>,
    icon: Ico,
    name: impl Into<SharedString>,
    collapsed: bool,
    first: bool,
    cx: &App,
    on_toggle: OnClick,
    on_add: Option<OnClick>,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    let group: SharedString = format!("{id}-group").into();
    h_flex()
        .id(ElementId::Name(id.clone()))
        .group(group.clone())
        .cursor_pointer()
        .px_4()
        .pt(rems(if first { 0.5 } else { 0.75 }))
        .pb_1()
        .gap_1p5()
        .items_center()
        .text_sm()
        .child(icon.icon().small().text_color(theme.muted_foreground))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(name.into()),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex_none()
                .p_0p5()
                .text_color(theme.muted_foreground)
                .opacity(if collapsed { 1. } else { 0. })
                .group_hover(group, |s| s.opacity(1.))
                .child(
                    if collapsed {
                        Ico::ChevronRight
                    } else {
                        Ico::ChevronDown
                    }
                    .icon()
                    .xsmall(),
                ),
        )
        .when_some(on_add, |d, on_add| {
            d.child(
                div()
                    .id(ElementId::Name(format!("{id}-add").into()))
                    .p_0p5()
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                    .child(Ico::Plus.icon().xsmall())
                    .on_click(move |ev, window, cx| {
                        cx.stop_propagation();
                        on_add(ev, window, cx)
                    }),
            )
        })
        .on_click(on_toggle)
}

/// A row in the sidebar list: title (cut with "…"), an optional muted second line, a 2px
/// accent stripe and a darker background when selected. `nested`: under a group header,
/// `Some(is_last_row_of_the_group)` (draws the group's indent guide). `actions`: buttons that
/// show on hover only (e.g. `arm_delete_button`).
pub fn list_row(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    meta: Option<SharedString>,
    selected: bool,
    nested: Option<bool>,
    actions: Option<AnyElement>,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    let group: SharedString = format!("{id}-row").into();
    let font = theme.font_size;
    h_flex()
        .id(ElementId::Name(id))
        .group(group.clone())
        .relative()
        .gap_1()
        .pl(if nested.is_some() { nest_pl(cx) } else { px(14.) })
        .pr_3()
        .py_2()
        .cursor_pointer()
        .border_l_2()
        .border_color(if selected {
            theme.primary
        } else {
            theme.transparent
        })
        .when(selected, |d| d.bg(theme.sidebar_accent))
        .hover(|s| s.bg(theme.sidebar_accent))
        // The group's indent guide: a 1px line under the header icon's center; in the last
        // row it stops where the text ends.
        .when_some(nested, |d, last| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .when(last, |d| d.bottom(font * 0.5))
                    .when(!last, |d| d.bottom_0())
                    .left(font * 1.5 - px(2.5))
                    .w(px(1.))
                    .bg(theme.border),
            )
        })
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().text_sm().truncate().child(title.into()))
                .when_some(meta, |d, meta| {
                    d.child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(meta),
                    )
                }),
        )
        .when_some(actions, |d, actions| {
            d.child(
                div()
                    .flex_none()
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .child(actions),
            )
        })
        .on_click(on_click)
}

// ---- main area --------------------------------------------------------------------------

/// A strip of tabs (open files): 36px, the active tab on the content background, the others
/// muted; "×" on the active tab and on hover; a dot instead of "×" for unsaved edits.
/// `tabs`: (label, active, unsaved).
pub fn tab_strip(
    id: &'static str,
    tabs: Vec<(SharedString, bool, bool)>,
    cx: &App,
    on_select: OnPick,
    on_close: OnPick,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    h_flex()
        .id(id)
        .h(px(36.))
        .flex_none()
        .w_full()
        .bg(theme.title_bar)
        .border_b_1()
        .border_color(theme.border)
        .overflow_x_scroll()
        .children(tabs.into_iter().enumerate().map(|(k, (label, on, dirty))| {
            let group: SharedString = format!("{id}-{k}").into();
            let (select, close) = (on_select.clone(), on_close.clone());
            h_flex()
                .id((id, k))
                .group(group.clone())
                .flex_none()
                .h_full()
                .pl_3()
                .pr_1()
                .gap_1()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .border_r_1()
                .border_color(theme.border)
                .when(on, |d| d.bg(theme.background).text_color(theme.foreground))
                .when(!on, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                })
                .child(label)
                .child(
                    div()
                        .id((SharedString::from(format!("{id}-x")), k))
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.muted_foreground)
                        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                        .map(|d| {
                            if dirty {
                                d.child(div().size(px(6.)).rounded_full().bg(theme.foreground))
                            } else {
                                d.child(Ico::X.icon().xsmall())
                                    .opacity(if on { 1. } else { 0. })
                                    .group_hover(group, |s| s.opacity(1.))
                            }
                        })
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            close(k, window, cx)
                        }),
                )
                .on_click(move |_, window, cx| select(k, window, cx))
        }))
}

/// What an empty area says: one muted line in the middle ("Open a file to start").
pub fn empty_state(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

// ---- settings ---------------------------------------------------------------------------

/// A settings row: title and a muted hint on the left, the control on the right, a rule
/// below. Stack them in a `v_flex().max_w(px(640.))` with `.px_4()` around.
pub fn settings_row(
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    control: AnyElement,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    h_flex()
        .gap_4()
        .py_3()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .child(
            v_flex()
                .flex_1()
                .gap_0p5()
                .child(div().text_sm().child(title.into()))
                .child(div().text_xs().text_color(theme.muted_foreground).child(hint.into())),
        )
        .child(control)
}

/// Pick one of a few options: square cells sharing borders, the chosen one in the accent.
pub fn segmented(
    id: &'static str,
    labels: &[&'static str],
    chosen: usize,
    cx: &App,
    on_pick: OnPick,
) -> Div {
    let theme = cx.theme().clone();
    h_flex()
        .border_1()
        .border_color(theme.border)
        .children(labels.iter().enumerate().map(|(k, label)| {
            let on = k == chosen;
            let pick = on_pick.clone();
            div()
                .id((id, k))
                .px_3()
                .py_1()
                .text_sm()
                .cursor_pointer()
                .when(k > 0, |d| d.border_l_1().border_color(theme.border))
                .when(on, |d| d.bg(theme.primary).text_color(theme.primary_foreground))
                .when(!on, |d| d.hover(|s| s.bg(theme.list_hover)))
                .child(*label)
                .on_click(move |_, window, cx| pick(k, window, cx))
        }))
}

// ---- panes ------------------------------------------------------------------------------

/// The 36px strip over a pane (Input / Output / Preview): a muted title on the left — or an
/// error in red in its place — and small buttons on the right.
pub fn pane_bar(
    title: impl Into<SharedString>,
    error: Option<SharedString>,
    buttons: Vec<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let title = title.into();
    h_flex()
        .flex_none()
        .h(px(36.))
        .px_3()
        .gap_2()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .text_xs()
        .child(div().flex_1().min_w_0().truncate().map(|d| match error {
            Some(e) => d.text_color(theme.danger).child(e),
            None => d.text_color(theme.muted_foreground).child(title),
        }))
        .children(buttons)
}

/// A small square text button for a pane's strip ("Paste", "Copy", "Clear").
pub fn small_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
    on_click: OnClick,
) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .flex_none()
        .px_2()
        .py_0p5()
        .text_xs()
        .border_1()
        .border_color(theme.border)
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(label.into())
        .on_click(on_click)
}
