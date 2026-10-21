//! The input box: multi-line text, mic, reasoning effort, send / stop. Flat, no shadow.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::model::EFFORTS;

impl HermesApp {
    pub(super) fn render_composer(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let busy = self.live.is_some();
        let recording = self.recorder.is_some();

        let efforts = EFFORTS.iter().enumerate().map(|(i, name)| {
            Button::new(("effort", i))
                .xsmall()
                .label(*name)
                .map(|b| {
                    if i == self.effort {
                        b.secondary()
                    } else {
                        b.ghost()
                    }
                })
                .on_click(cx.listener(move |this, _, _, cx| this.set_effort(i, cx)))
        });

        let mic = Button::new("mic")
            .small()
            .map(|b| if recording { b.danger() } else { b.ghost() })
            .icon(if self.transcribing {
                IconName::LoaderCircle
            } else {
                IconName::Mic
            })
            .tooltip(if recording {
                "Остановить и распознать"
            } else {
                "Надиктовать"
            })
            .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)));

        let action = if busy {
            Button::new("stop")
                .small()
                .secondary()
                .icon(IconName::Square)
                .tooltip("Остановить")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("send")
                .small()
                .primary()
                .icon(IconName::ArrowUp)
                .on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
        };

        v_flex()
            .w_full()
            .flex_none()
            .bg(theme.title_bar)
            .border_t_1()
            .border_color(if recording {
                theme.danger
            } else {
                theme.border
            })
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .px_4()
                    .pt_2()
                    .pb_2()
                    .child(Textarea::new(&self.composer).appearance(false))
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(mic)
                            .child(h_flex().gap_0p5().children(efforts))
                            .child(div().flex_1())
                            .when(recording, |d| {
                                d.child(div().text_xs().text_color(theme.danger).child("Запись…"))
                            })
                            .child(action),
                    ),
            )
    }
}
