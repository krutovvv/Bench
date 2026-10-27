//! The gear's Appearance tab, and where Bench keeps the person's choice (the look itself lives in
//! `bench_ui::theme`).

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bench_ui::theme::Appearance;

use crate::app::BenchApp;

pub fn load() -> Appearance {
    Appearance::load_from(&crate::client::connection::state_dir())
}

pub fn save(a: &Appearance) {
    a.save_to(&crate::client::connection::state_dir())
}

impl BenchApp {
    pub(crate) fn render_appearance(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        use bench_ui::theme::{ACCENTS, Mode};
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let a = self.appearance.clone();

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
                         items: Vec<(&'static str, bool, bench_ui::theme::Appearance)>,
                         cx: &mut Context<Self>| {
            h_flex()
                .border_1()
                .rounded_md()
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
                            this.set_appearance(next.clone(), window, cx)
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
        .map(|(l, m)| (l, a.mode == m, bench_ui::theme::Appearance { mode: m, ..a.clone() }))
        .collect();
        let view = cx.entity();
        let look = a.clone();
        let compact = Checkbox::new("compact-sidebar")
            .checked(a.compact_sidebar)
            .on_click(move |on, window, cx| {
                let next = bench_ui::theme::Appearance {
                    compact_sidebar: *on,
                    ..look.clone()
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
                        let next = bench_ui::theme::Appearance { accent: k, ..a.clone() };
                        div()
                            .id(("accent", k))
                            .p(px(3.))
                            .border_1()
                            .rounded_md()
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
                                this.set_appearance(next.clone(), window, cx)
                            }))
                    }),
            )
            .into_any_element();

        // A colour theme paints the surfaces and the code; the accent below stays the person's.
        let view = cx.entity();
        let chips = bench_ui::theme_chips(
            &a.scheme,
            theme.mode.is_dark(),
            cx,
            std::rc::Rc::new(move |k, window, cx| {
                let name = bench_ui::theme::palettes::SCHEMES[k].name;
                view.update(cx, |this, cx| {
                    let next = bench_ui::theme::Appearance {
                        scheme: name.into(),
                        ..this.appearance.clone()
                    };
                    this.set_appearance(next, window, cx);
                });
            }),
        );

        // The page's own width: these rows are rows of the same list, and a narrower column for
        // them alone read as a mistake (their rules ended short of every other rule).
        v_flex().w_full().child(
            v_flex()
                .w_full()
                .child(row("Theme", a.scheme().halves, chips))
                .child(row(
                    "Mode",
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
}

impl BenchApp {
    pub fn set_appearance(
        &mut self,
        a: bench_ui::theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::settings::appearance::save(&a);
        self.appearance = a;
        bench_ui::theme::apply(&self.appearance, Some(window), cx);
        self.repaint_terminals(cx);
        cx.notify();
    }
}
