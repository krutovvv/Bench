//! A colour theme as a chip: a page of its own surfaces under a band of its own chrome, its name
//! under that. The gear shows a row of them (`theme_chips`), and the person reads the theme
//! before they click it.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use crate::theme::palettes::{Palette, SCHEMES};
use crate::{Gap, OnPick};

/// The chip's card: wide enough for three lines of "text", as tall as a block head.
pub const SWATCH_W: f32 = 84.;
pub const SWATCH_H: f32 = 40.;
/// The band of chrome (the title bar) over the page.
pub const SWATCH_BAND: f32 = 12.;
/// A line of "text" on the page, and how long the three of them are.
pub const SWATCH_LINE: f32 = 3.;
pub const SWATCH_TEXT: [f32; 3] = [62., 48., 26.];

/// The row of chips, in the order of `SCHEMES`; `pick` gets the index. `dark` is the half the
/// app is in now, so a chip shows what the person would get.
pub fn theme_chips(current: &str, dark: bool, cx: &App, pick: OnPick) -> AnyElement {
    let theme = cx.theme().clone();
    h_flex()
        .flex_wrap()
        .gap_2()
        .children(SCHEMES.iter().enumerate().map(|(k, s)| {
            let on = s.name == current;
            let tip = s.halves;
            let pick = pick.clone();
            div()
                .id(("scheme", k))
                .w(px(SWATCH_W))
                .cursor_pointer()
                .child(swatch(s.pick(dark), s.swatch, on))
                .child(
                    div()
                        .pt_1()
                        .text_sm()
                        .text_color(if on {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(s.name),
                )
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
                })
                .on_click(move |_, window, app| pick(k, window, app))
                .into_any_element()
        }))
        .into_any_element()
}

/// The card: the theme's chrome over the theme's page, two lines of its ink and one in its hue.
fn swatch(p: &Palette, hue: u32, on: bool) -> AnyElement {
    let ink = |hex: u32, a: f32| {
        let mut h: Hsla = rgb(hex).into();
        h.a = a;
        h
    };
    // The chip's rule is its own hue when it is the chosen one.
    let hue: Hsla = rgb(hue).into();
    let edge: Hsla = if on { hue } else { rgb(p.border).into() };
    div()
        .w_full()
        .h(px(SWATCH_H))
        .overflow_hidden()
        .border_1()
        .rounded_md()
        .border_color(edge)
        .child(
            v_flex()
                .child(
                    div().h(px(SWATCH_BAND)).bg(rgb(p.chrome)).child(
                        h_flex()
                            .px_1()
                            .pt(px(3.))
                            .gap(Gap::Xs.px())
                            .child(dot(ink(p.foreground, 0.5)))
                            .child(dot(ink(p.foreground, 0.3))),
                    ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .gap(px(SWATCH_LINE))
                        .px_1p5()
                        .pt(px(4.))
                        .bg(rgb(p.background))
                        .child(line(SWATCH_TEXT[0], ink(p.foreground, 0.9)))
                        .child(line(SWATCH_TEXT[1], ink(p.foreground, 0.45)))
                        .child(line(SWATCH_TEXT[2], hue)),
                ),
        )
        .into_any_element()
}

fn dot(color: Hsla) -> AnyElement {
    div()
        .size(px(3.))
        .rounded_full()
        .bg(color)
        .into_any_element()
}

fn line(w: f32, color: Hsla) -> AnyElement {
    div()
        .w(px(w))
        .h(px(SWATCH_LINE))
        .rounded_sm()
        .bg(color)
        .into_any_element()
}
