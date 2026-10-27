//! Forms: label + field rows, settings rows, a segmented pick.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::OnPick;
use crate::icons::Ico;
use crate::metrics::RING_CLEAR;

/// A form in the thread: a title, rows of a muted label and its field, a note, the actions.
pub fn inline_form<L: Into<SharedString>>(
    title: impl Into<SharedString>,
    rows: Vec<(L, AnyElement)>,
    note: Option<SharedString>,
    actions: Vec<AnyElement>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap(px(RING_CLEAR))
        .child(
            div()
                .pb_1()
                .font_weight(FontWeight::MEDIUM)
                .child(title.into()),
        )
        .children(
            rows.into_iter()
                .map(|(label, field)| field_row(label, field, cx)),
        )
        .children(note.map(|n| div().pt_1().text_color(theme.muted_foreground).child(n)))
        .child(h_flex().pt_2().gap_3().children(actions))
        .into_any_element()
}

/// One row of a form: the muted label in a fixed column, the field after it.
pub fn field_row(label: impl Into<SharedString>, field: impl IntoElement, cx: &App) -> Div {
    h_flex()
        .gap_3()
        .items_center()
        .child(
            div()
                .w(px(110.))
                .flex_none()
                .text_color(cx.theme().muted_foreground)
                .child(label.into()),
        )
        .child(div().flex_1().max_w(px(560.)).child(field))
}

/// The left column of a settings page's field: its title and hint.
pub const SETTING_LABEL_W: f32 = 220.;

/// A setting whose control is wide (a field, a list of choices): title and hint in a fixed column
/// on the left, the control taking the rest, a rule below.
pub fn settings_field(
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    control: AnyElement,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let hint: SharedString = hint.into();
    h_flex()
        .w_full()
        .gap_4()
        .py_2p5()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .child(
            v_flex()
                .w(px(SETTING_LABEL_W))
                .flex_none()
                .gap_0p5()
                .child(div().child(title.into()))
                .when(!hint.is_empty(), |d| {
                    d.child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(hint),
                    )
                }),
        )
        .child(div().flex_1().min_w_0().child(control))
}

/// A setting: title and a muted hint on the left, the control on the right, a rule below.
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
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(hint.into()),
                ),
        )
        .child(control)
}

/// A group's title on a settings page: small, muted, in capitals, with air above; its actions
/// ("+ Add", "Check") on the right.
pub fn settings_heading(title: impl Into<SharedString>, actions: Vec<AnyElement>, cx: &App) -> Div {
    let title: SharedString = title.into();
    h_flex()
        .w_full()
        .pt_6()
        .pb_2()
        .gap_3()
        .items_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(title.to_uppercase()),
        )
        .children(actions)
}

/// Pick one of a few: cells sharing borders, the chosen one in the accent.
pub fn segmented(
    id: &'static str,
    labels: &[SharedString],
    chosen: usize,
    cx: &App,
    on_pick: OnPick,
) -> Div {
    let theme = cx.theme().clone();
    h_flex()
        .rounded_md()
        .overflow_hidden()
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
                .when(on, |d| {
                    d.bg(theme.primary).text_color(theme.primary_foreground)
                })
                .when(!on, |d| d.hover(|s| s.bg(theme.list_hover)))
                .child(label.clone())
                .on_click(move |_, window, cx| pick(k, window, cx))
        }))
}

/// Sections of a settings panel: quiet text tabs with icons, the open one in the foreground
/// with a 2 px accent underline over the rule that crosses the page beneath them.
pub fn settings_tabs(
    id: &'static str,
    tabs: &[(Ico, &'static str)],
    open: usize,
    cx: &App,
    on_pick: OnPick,
) -> Div {
    let theme = cx.theme().clone();
    h_flex()
        .flex_1()
        .gap_2()
        .items_end()
        .border_b_1()
        .border_color(theme.border)
        .children(tabs.iter().enumerate().map(|(k, (icon, label))| {
            let on = k == open;
            let pick = on_pick.clone();
            h_flex()
                .id((id, k))
                .gap_1p5()
                .items_center()
                .px_2()
                .py_2()
                .cursor_pointer()
                // the 2 px accent under the open tab stands in for the 1 px rule
                .border_b_2()
                .border_color(if on { theme.primary } else { theme.transparent })
                .text_color(if on {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .when(!on, |d| d.hover(|s| s.text_color(theme.foreground)))
                .child(icon.icon().small().text_color(theme.foreground))
                .child(
                    div()
                        .text_sm()
                        .when(on, |d| d.font_weight(FontWeight::MEDIUM))
                        .child(*label),
                )
                .on_click(move |_, window, cx| pick(k, window, cx))
        }))
}
