//! Settings: one scrolling page; Appearance first (theme, accent, size).

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::MainApp;

impl MainApp {
    pub(super) fn render_appearance(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::theme::{ACCENTS, Mode, Size};
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let a = self.appearance;

        let row = |title: &'static str, hint: &'static str, control: AnyElement| {
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
                        .child(div().text_sm().child(title))
                        .child(div().text_xs().text_color(muted).child(hint)),
                )
                .child(control)
        };
        // A segmented control: square cells sharing borders, the chosen one in the accent.
        let segmented = |id: &'static str,
                         items: Vec<(&'static str, bool, crate::theme::Appearance)>,
                         cx: &mut Context<Self>| {
            h_flex()
                .border_1()
                .border_color(theme.border)
                .children(items.into_iter().enumerate().map(|(k, (label, on, next))| {
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
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_appearance(next, window, cx)
                        }))
                }))
                .into_any_element()
        };

        let modes = [
            ("Light", Mode::Light),
            ("Dark", Mode::Dark),
            ("System", Mode::System),
        ]
        .into_iter()
        .map(|(l, m)| (l, a.mode == m, crate::theme::Appearance { mode: m, ..a }))
        .collect();
        let sizes = [
            ("Small", Size::Small),
            ("Normal", Size::Normal),
            ("Large", Size::Large),
        ]
        .into_iter()
        .map(|(l, s)| (l, a.size == s, crate::theme::Appearance { size: s, ..a }))
        .collect();
        let dark = theme.mode.is_dark();
        let accents = h_flex()
            .gap_1()
            .children(
                ACCENTS
                    .iter()
                    .enumerate()
                    .map(|(k, (name, light, dark_c))| {
                        let on = a.accent == k;
                        let color: Hsla = rgb(if dark { *dark_c } else { *light }).into();
                        let next = crate::theme::Appearance { accent: k, ..a };
                        div()
                            .id(("accent", k))
                            .p(px(3.))
                            .border_1()
                            .border_color(if on {
                                theme.foreground
                            } else {
                                theme.transparent
                            })
                            .cursor_pointer()
                            .child(div().size(px(18.)).bg(color))
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(*name).build(window, cx)
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_appearance(next, window, cx)
                            }))
                    }),
            )
            .into_any_element();

        v_flex().w_full().child(
            v_flex()
                .max_w(px(640.))
                .child(row(
                    "Theme",
                    "Light, dark, or follow macOS.",
                    segmented("mode", modes, cx),
                ))
                .child(row("Accent", "Buttons, selection, links.", accents))
                .child(row(
                    "Size",
                    "Text and spacing across the app.",
                    segmented("size", sizes, cx),
                )),
        )
    }

    pub(super) fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let appearance = self.render_appearance(cx).into_any_element();
        v_flex()
            .id("settings-page")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(div().pt_4().pb_1().text_xs().text_color(muted).child("APPEARANCE"))
            .child(appearance)
    }
}
