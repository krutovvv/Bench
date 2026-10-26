//! Bars: the sidebar's search band, the header, the status bar's pieces, a pane's strip.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::bar_h;

/// The sidebar's search: a transparent band, the magnifier over the group icons.
pub fn search_band(input: &Entity<InputState>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .w_full()
        .h(bar_h(cx))
        .pl(px(3.5))
        .pr_2()
        .items_center()
        .child(
            Input::new(input).appearance(false).cleanable(true).prefix(
                Ico::Search
                    .icon()
                    .small()
                    .text_color(theme.muted_foreground),
            ),
        )
}

/// The header over the main area: the title on the left (cut with "…"), `right` on the right.
pub fn header_bar(title: impl Into<SharedString>, right: AnyElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .h(bar_h(cx) + px(1.))
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
                .truncate()
                .child(title.into()),
        )
        .child(right)
}

/// A fact in the status bar.
pub fn status_text(text: impl Into<SharedString>) -> Div {
    h_flex()
        .h_full()
        .px_2()
        .gap_1()
        .items_center()
        .child(text.into())
}

/// A button in the status bar: the bar's height, an extra-small icon.
pub fn status_button(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    on: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    h_flex()
        .id(id)
        .h_full()
        .px_2()
        .items_center()
        .cursor_pointer()
        .text_color(if on {
            theme.primary
        } else {
            theme.muted_foreground
        })
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.icon().xsmall())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// The strip over a pane (Input / Output): a muted title — or an error in its place — and
/// buttons on the right.
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
        .h(bar_h(cx) + px(1.))
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
