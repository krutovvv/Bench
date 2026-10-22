//! The input: multi-line text on the left, a quiet icon column on the right (mic, send / stop).
//! No buttons, no shadow — the icons just light up on hover.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::icons::Ico;

/// Rows the command list shows before it scrolls.
const SLASH_ROWS: f32 = 8.;

/// Bars in the waveform; it scrolls right to left, newest on the right (25 ms each = 3 s).
const WAVE_BARS: usize = 120;

impl HermesApp {
    /// Commands matching a bare `/word`, in a small list right above the input.
    fn render_slash(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let matches = self.slash_matches(cx);
        if matches.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let sel = self.slash_sel.min(matches.len() - 1);
        let rows = matches.into_iter().enumerate().map(|(i, c)| {
            let on = i == sel;
            h_flex()
                .id(("slash", i))
                .h(px(28.))
                .px_3()
                .gap_3()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    div()
                        .flex_none()
                        .font_family("Menlo")
                        .text_xs()
                        .child(c.name),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(c.desc),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground.opacity(0.7))
                        .child(c.category),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.pick_slash(i, window, cx)))
        });
        Some(
            // Rests on the input: the chat's width with its 16px sides; the input's top rule is
            // its bottom edge.
            div()
                .absolute()
                .bottom_full()
                .left_0()
                .right_0()
                .px_4()
                .child(
                    div()
                        .bg(theme.popover)
                        .border_t_1()
                        .border_l_1()
                        .border_r_1()
                        .border_color(theme.border)
                        .child(
                            v_flex()
                                .id("slash-list")
                                .max_h(px(28. * SLASH_ROWS))
                                .overflow_y_scroll()
                                .track_scroll(&self.slash_scroll)
                                .children(rows),
                        ),
                )
                .into_any_element(),
        )
    }

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
            // The right gap runs to the mic glyph: 16 + the icon column's 8 + 9 inside its
            // 34px box around a 16px icon. The left gets the same 33.
            .pl(px(33.))
            .pr_4()
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

        let slash = self.render_slash(cx);

        h_flex()
            .id("composer")
            .relative()
            .w_full()
            .flex_none()
            .min_h(super::bar_h(cx))
            .items_center()
            .bg(theme.background)
            // The command list takes ↑ ↓ Tab Esc before the text field does.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let k = &ev.keystroke;
                if !k.modifiers.modified() && this.slash_key(k.key.as_str(), window, cx) {
                    cx.stop_propagation();
                }
            }))
            .children(slash)
            .border_t_1()
            .border_color(theme.border)
            .when(recording, |d| d.child(self.render_wave(&theme)))
            .when(!recording, |d| {
                d.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // Measured: the "New session" button sits 14pt from the divider; the
                        // textarea pads its text by ~10.5 itself, so 3.5 more lands the text at 14.
                        .pl(px(3.5))
                        .py_1()
                        .child(Textarea::new(&self.composer).appearance(false)),
                )
            })
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .pl_2()
                    // A little more room right of the send button than the 8 it had.
                    .pr_3()
                    .items_center()
                    .child(mic)
                    .child(action),
            )
    }
}
