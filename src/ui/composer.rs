//! The input: multi-line text on the left, a quiet icon column on the right (mic, send / stop).
//! No buttons, no shadow — the icons just light up on hover.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::icons::Ico;

impl HermesApp {
    pub(super) fn render_composer(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let busy = self.live.is_some();
        let recording = self.recorder.is_some();
        let has_text = !self.composer.read(cx).value().trim().is_empty();

        let area = |id: &'static str, icon: Ico, tip: &'static str, color: Hsla| {
            div()
                .id(id)
                .size(px(34.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(color)
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        let mic = area(
            "mic",
            if self.transcribing {
                Ico::Loader
            } else {
                Ico::Mic
            },
            if recording {
                "Остановить и распознать"
            } else {
                "Надиктовать"
            },
            if recording {
                theme.danger
            } else {
                theme.muted_foreground
            },
        )
        .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)));

        let action = if busy {
            area("stop", Ico::Square, "Остановить", theme.foreground)
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            area(
                "send",
                Ico::ArrowUp,
                "Отправить (Enter)",
                if has_text {
                    theme.primary
                } else {
                    theme.muted_foreground
                },
            )
            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
        };

        h_flex()
            .w_full()
            .flex_none()
            .items_end()
            .bg(theme.background)
            .border_t_1()
            .border_color(if recording {
                theme.danger
            } else {
                theme.border
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_4()
                    .py_3()
                    .child(Textarea::new(&self.composer).appearance(false)),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .px_2()
                    .py_2()
                    .items_center()
                    .when(recording, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.danger)
                                .pr_1()
                                .child("запись…"),
                        )
                    })
                    .child(mic)
                    .child(action),
            )
    }
}
