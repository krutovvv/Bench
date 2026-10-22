//! The input: multi-line text on the left, a quiet icon column on the right (mic, send / stop).
//! No buttons, no shadow — the icons just light up on hover.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::icons::Ico;

/// Bars in the waveform; it scrolls right to left, newest on the right (25 ms each = 3 s).
const WAVE_BARS: usize = 120;

impl HermesApp {
    /// While recording the text field is not needed (speech is recognized at the end), so its
    /// place shows the voice level live.
    fn render_wave(&self, theme: &gpui_kit::component::Theme) -> impl IntoElement {
        let levels = self
            .recorder
            .as_ref()
            .map(|r| r.levels(WAVE_BARS))
            .unwrap_or_default();
        let bars = levels.into_iter().map(|v| {
            div()
                .flex_1()
                .h(px(2. + v * 26.))
                .bg(theme.primary.opacity(0.35 + v * 0.65))
        });
        h_flex()
            .flex_1()
            .min_w_0()
            .h(px(32.))
            .px_4()
            .gap(px(2.))
            .items_center()
            .children(bars)
    }

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
                .child(icon.live(true))
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        let mic = area(
            "mic",
            if self.transcribing {
                Ico::Loader
            } else if recording {
                Ico::AudioLines
            } else {
                Ico::Mic
            },
            if recording {
                "Stop and transcribe"
            } else {
                "Dictate"
            },
            if recording {
                theme.primary
            } else {
                theme.muted_foreground
            },
        )
        .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)));

        let action = if busy {
            area("stop", Ico::Square, "Stop", theme.foreground)
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            area(
                "send",
                Ico::ArrowUp,
                "Send (Enter)",
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
            .min_h(px(super::BAR_H))
            .items_center()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .when(recording, |d| d.child(self.render_wave(&theme)))
            .when(!recording, |d| {
                d.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // The textarea pads its text by 8px itself: 8 + 8 = the 16px content inset.
                        .pl_2()
                        .py_1()
                        .child(Textarea::new(&self.composer).appearance(false)),
                )
            })
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .px_2()
                    .items_center()
                    .child(mic)
                    .child(action),
            )
    }
}
