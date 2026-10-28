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
                .font_weight(FontWeight::SEMIBOLD)
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

/// One of a few answers to one question, as a card in a row of them: an icon and a radio on top,
/// a title, a line of what it means. `on`: picked (the accent's ring and tint); `off`: not
/// available here (dimmed, and `text` says why).
pub fn choice_card(
    id: impl Into<ElementId>,
    icon: crate::Ico,
    title: impl Into<SharedString>,
    text: impl Into<SharedString>,
    on: bool,
    off: bool,
    cx: &App,
) -> Stateful<Div> {
    use crate::metrics::{CHOICE_CARD_MIN_H, RADIO, RADIO_ON_RING};
    let theme = cx.theme().clone();
    v_flex()
        .id(id)
        .flex_1()
        .min_w_0()
        .min_h(px(CHOICE_CARD_MIN_H))
        .p_3()
        .gap_1()
        .rounded_lg()
        .border_1()
        .map(|d| {
            if on {
                d.border_color(theme.primary).bg(theme.primary.opacity(0.08))
            } else {
                d.border_color(theme.border).bg(theme.background)
            }
        })
        .when(off, |d| d.opacity(0.5))
        .when(!off && !on, |d| d.cursor_pointer().hover(|s| s.bg(theme.list_hover)))
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .pb_2()
                .child(icon.icon().small().text_color(if on { theme.primary } else { theme.muted_foreground }))
                .child(
                    div()
                        .size(px(RADIO))
                        .rounded_full()
                        .map(|d| {
                            if on {
                                d.border(px(RADIO_ON_RING)).border_color(theme.primary).bg(theme.background)
                            } else {
                                d.border_1().border_color(theme.border)
                            }
                        }),
                ),
        )
        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title.into()))
        .child(div().text_xs().text_color(theme.muted_foreground).child(text.into()))
}

/// A settings page: its rows and headings, scrolled, as far from the edges as the thread's avatar.
pub fn settings_page(id: &'static str, rows: Vec<AnyElement>) -> AnyElement {
    v_flex()
        .id(id)
        .size_full()
        .overflow_y_scroll()
        .px(px(crate::metrics::CHAT_LEFT))
        .pb(px(crate::metrics::CHAT_LEFT))
        .child(v_flex().flex_1().children(rows))
        .into_any_element()
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

/// A group's title on a settings page, with air above; its actions
/// ("+ Add", "Check") on the right.
pub fn settings_heading(title: impl Into<SharedString>, actions: Vec<AnyElement>, cx: &App) -> Div {
    let title: SharedString = title.into();
    h_flex()
        .w_full()
        .pt_6()
        .pb_2()
        .gap_3()
        .items_center()
        // A group's title as macOS Settings has them: in its own case, semibold, the text's
        // colour, over the rows it heads.
        .text_size(px(crate::metrics::SIDEBAR_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(title),
        )
        .children(actions)
}

/// Pick one of a few, as a Mac's segmented control: a grey track, the chosen segment a raised
/// chip on the page's ground, the others words in the muted colour (`segment_track`,
/// `segment`; the header's Chat | Code is the same).
pub fn segmented(
    id: &'static str,
    labels: &[SharedString],
    chosen: usize,
    cx: &App,
    on_pick: OnPick,
) -> Div {
    let theme = cx.theme().clone();
    segment_track(&theme).children(labels.iter().enumerate().map(|(k, label)| {
        let pick = on_pick.clone();
        segment(div().id((id, k)), k == chosen, &theme)
            .child(label.clone())
            .on_click(move |_, window, cx| pick(k, window, cx))
    }))
}

/// A segmented control's track: the segments sit in it with a hair of air round them.
pub fn segment_track(theme: &gpui_kit::component::Theme) -> Div {
    h_flex()
        .flex_none()
        .p(px(crate::metrics::MODE_SWITCH_PAD))
        .gap(px(crate::metrics::MODE_SWITCH_PAD))
        .rounded_md()
        .bg(theme.foreground.opacity(0.06))
}

/// One segment of a track: `on`, a raised chip; else muted words that darken on hover.
pub fn segment(d: Stateful<Div>, on: bool, theme: &gpui_kit::component::Theme) -> Stateful<Div> {
    d.h(px(crate::metrics::SEGMENT_H))
        .px_3()
        .flex()
        .items_center()
        .rounded_sm()
        .text_size(px(crate::metrics::SIDEBAR_TEXT))
        .cursor_pointer()
        .map(|d| {
            if on {
                d.bg(theme.background).shadow_xs().text_color(theme.foreground)
            } else {
                let fg = theme.foreground;
                d.text_color(theme.muted_foreground).hover(move |s| s.text_color(fg))
            }
        })
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
