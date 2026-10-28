//! Buttons. A form's buttons are as tall as its fields (`FIELD_H`); a block head's are compact.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::Ico;
use crate::metrics::FIELD_H;

/// The main action of a form or a card ("Save", "Add"). One per place.
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).primary().label(label)
}

/// A bordered action beside the main one ("Check", "Revert").
pub fn secondary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).outline().label(label)
}

/// A quiet action that reads as text until hovered ("Cancel", "Decline").
pub fn ghost_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().label(label)
}

/// An action that destroys something.
pub fn danger_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).danger().label(label)
}

/// A compact text button for a block's head or a pane strip.
pub fn compact_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).small().ghost().label(label)
}

/// A square icon button with a tooltip: block heads, rows, the header. `on`: the thing it
/// toggles is on (accent).
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    on: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    div()
        .id(id)
        .flex_none()
        .p_1()
        .rounded_md()
        .cursor_pointer()
        .text_color(if on {
            theme.primary
        } else {
            theme.muted_foreground
        })
        .when(on, |d| d.bg(theme.list_active))
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.live(false))
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// A composer's round button beside Send, as Claude's and Messages' plus and mic: Send's size,
/// its glyph at the icon's own size, a soft disc on hover.
pub fn round_icon_button(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme().clone();
    div()
        .id(id)
        .flex_none()
        .size(px(crate::metrics::SEND_BUTTON))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.icon())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// The small icon button inside a block's head or a row (an insert's controls, a tree row's
/// actions): no ground until hovered.
pub fn head_icon_button(id: &str, icon: Ico, tip: &'static str, cx: &App) -> Stateful<Div> {
    let theme = cx.theme().clone();
    div()
        .id(SharedString::from(id.to_string()))
        .p_0p5()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|s| s.text_color(theme.foreground).bg(theme.list_hover))
        .child(icon.icon().small())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// A form's action as Bench draws it in the thread: the primary one filled, the others muted
/// text. As tall as a field.
pub fn form_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let (fg, bg, hover, muted, text) = (
        theme.primary_foreground,
        theme.primary,
        theme.primary_hover,
        theme.muted_foreground,
        theme.foreground,
    );
    div()
        .id(id)
        .h(px(FIELD_H))
        .flex()
        .items_center()
        // A control's size, as the segmented ones beside it — not the row's text it sits in.
        .text_size(px(crate::metrics::SIDEBAR_TEXT))
        .cursor_pointer()
        .when(primary, |d| {
            d.px_3()
                .rounded_md()
                .bg(bg)
                .text_color(fg)
                .hover(move |s| s.bg(hover))
        })
        .when(!primary, |d| {
            d.text_color(muted).hover(move |s| s.text_color(text))
        })
        .child(label.into())
}

/// Delete without a dialog: the first click arms it (red, "Confirm delete"), the second deletes.
/// The caller keeps `armed` and clears it on any other click.
pub fn arm_delete_button(id: impl Into<ElementId>, armed: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme().clone();
    h_flex()
        .id(id)
        .flex_none()
        .px_1p5()
        .py_0p5()
        .gap_1()
        .items_center()
        .rounded_md()
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
}

/// An armed delete asking in place: the danger action ("Delete", "Remove") filled red, then a
/// quiet Cancel. Each click stops there: the row under it is not clicked too.
pub fn confirm_buttons(
    id: impl Into<SharedString>,
    action: &'static str,
    on_yes: crate::OnClick,
    on_no: crate::OnClick,
    cx: &App,
) -> Div {
    let theme = cx.theme().clone();
    let id: SharedString = id.into();
    h_flex()
        .gap_1()
        .text_xs()
        .child(
            div()
                .id(SharedString::from(format!("{id}-yes")))
                .px_1p5()
                .py_0p5()
                .rounded_md()
                .cursor_pointer()
                .bg(theme.danger)
                .text_color(theme.danger_foreground)
                .child(action)
                .on_click(move |e, window, cx| {
                    cx.stop_propagation();
                    on_yes(e, window, cx)
                }),
        )
        .child(
            div()
                .id(SharedString::from(format!("{id}-no")))
                .px_1()
                .py_0p5()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .child("Cancel")
                .on_click(move |e, window, cx| {
                    cx.stop_propagation();
                    on_no(e, window, cx)
                }),
        )
}

/// A row's or a head's small icon button (its menu "⋯", its settings): no ground until hovered.
pub fn row_icon_button(id: impl Into<ElementId>, icon: Ico) -> Button {
    Button::new(id).ghost().xsmall().icon(icon.icon())
}

/// What shows only while the row `group` is hovered (its "⋯", its "+"); a press on it is its own,
/// not the row's.
pub fn on_group_hover(group: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .opacity(0.)
        .group_hover(group.into(), |s| s.opacity(1.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}
