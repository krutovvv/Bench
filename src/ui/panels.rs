//! Settings panels: Telegram and appearance (providers are in providers.rs). MCP, skills, prompts and cron live in projects.

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;

impl HermesApp {

    pub(super) fn render_telegram(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let all = self.tg_visible.is_none();
        let rows = self.tg_people.iter().enumerate().map(|(i, (user, name))| {
            let on = all || self.tg_visible.as_ref().is_some_and(|v| v.contains(user));
            let id = user.clone();
            h_flex()
                .id(("tg", i))
                .gap_3()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    div()
                        .size(px(14.))
                        .border_1()
                        .border_color(if on { theme.primary } else { theme.input })
                        .when(on, |d| d.bg(theme.primary)),
                )
                .child(div().flex_1().text_base().child(name.clone()))
                .child(div().text_sm().text_color(muted).child(user.clone()))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.toggle_telegram_person(id.clone(), cx)),
                )
        });
        v_flex()
            .w_full()
            .gap_3()
            .child(div().text_base().text_color(muted).child(
                "Whose Telegram sessions the list shows. Checked people are shown, the rest are hidden (the sessions themselves are not touched).",
            ))
            .child(
                v_flex()
                    .max_w(px(560.))
                    .border_t_1()
                    .border_color(theme.border)
                    .children(rows)
                    .when(self.tg_people.is_empty(), |d| {
                        d.child(div().py_3().text_base().text_color(muted).child("No Telegram sessions for this agent."))
                    }),
            )
    }

    pub(super) fn render_appearance(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::theme::{ACCENTS, Mode};
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
                        .child(div().text_base().child(title))
                        .child(div().text_sm().text_color(muted).child(hint)),
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
                        .text_base()
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
        let view = cx.entity();
        let compact = Checkbox::new("compact-sidebar")
            .checked(a.compact_sidebar)
            .on_click(move |on, window, cx| {
                let next = crate::theme::Appearance {
                    compact_sidebar: *on,
                    ..a
                };
                view.update(cx, |this, cx| this.set_appearance(next, window, cx));
            })
            .into_any_element();
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
                    "Compact sidebar",
                    "Chats and servers as titles only, closer together.",
                    compact,
                )),
        )
    }

    /// Settings: Appearance, one scrolling page.
    pub(super) fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let heading = |t: &'static str| div().pt_4().pb_1().text_sm().text_color(muted).child(t);
        let appearance = self.render_appearance(cx).into_any_element();
        v_flex()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(heading("APPEARANCE"))
            .child(appearance)
    }
}
