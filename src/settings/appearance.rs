//! The gear's Appearance tab, and where Bench keeps the person's choice (the look itself lives in
//! `bench_ui::theme`).

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use bench_ui::Gap;

use bench_ui::theme::Appearance;

use crate::app::BenchApp;
use crate::media::sound;

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
                .gap(Gap::Xl)
                .py(Gap::L)
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .flex_1()
                        .gap(Gap::Xxs)
                        .child(div().text_base().child(title))
                        .child(div().text_sm().text_color(muted).child(hint)),
                )
                .child(control)
        };
        let modes = [Mode::Light, Mode::Dark, Mode::System];
        let chosen = modes.iter().position(|m| *m == a.mode).unwrap_or(usize::MAX);
        let mode = bench_ui::segmented(
            "mode",
            ["Light", "Dark", "System"],
            chosen,
            cx,
            bench_ui::on_pick(cx, move |this: &mut Self, k, window, cx| {
                let next = bench_ui::theme::Appearance {
                    mode: modes[k].clone(),
                    ..this.appearance.clone()
                };
                this.set_appearance(next, window, cx)
            }),
        )
        .into_any_element();
        // The page's own width: these rows are rows of the same list, and a narrower column for
        // them alone read as a mistake (their rules ended short of every other rule).
        let sounds = gpui_kit::component::switch::Switch::new("sounds")
            .checked(sound::enabled())
            .on_click(cx.listener(|_, on: &bool, _, cx| {
                sound::set_enabled(*on);
                // What was switched on, heard at once.
                sound::Sound::MessageSent.play();
                cx.notify();
            }))
            .into_any_element();
        v_flex().w_full().child(
            v_flex()
                .w_full()
                .child(row("Mode", "Light, dark, or follow macOS.", mode))
                .child(row("Sounds", "Soft sounds for the mic and a sent message.", sounds)),
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
