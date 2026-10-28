//! The gear's Appearance tab, and where Bench keeps the person's choice (the look itself lives in
//! `bench_ui::theme`).

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
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
        use bench_ui::theme::Mode;
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
        // The kit's segmented control (a Mac's): a grey track, the chosen one raised.
        let segmented = |id: &'static str,
                         items: Vec<(&'static str, bool, bench_ui::theme::Appearance)>,
                         cx: &mut Context<Self>| {
            bench_ui::segment_track(&theme)
                .children(items.into_iter().enumerate().map(|(k, (label, on, next))| {
                    bench_ui::segment(div().id((id, k)), on, &theme)
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
        .map(|(l, m)| {
            (
                l,
                a.mode == m,
                bench_ui::theme::Appearance {
                    mode: m,
                    ..a.clone()
                },
            )
        })
        .collect();
        // The page's own width: these rows are rows of the same list, and a narrower column for
        // them alone read as a mistake (their rules ended short of every other rule).
        v_flex().w_full().child(
            v_flex()
                .w_full()
                .child(row(
                    "Mode",
                    "Light, dark, or follow macOS.",
                    segmented("mode", modes, cx),
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
