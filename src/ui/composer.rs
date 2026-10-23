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
            // The same gap on both sides: 24 from the divider, and 16 + the button column's 8
            // to the round stop button.
            .pl(px(24.))
            .pr_4()
            .gap(px(2.))
            .items_center()
            .children(bars)
    }

    pub(super) fn render_composer(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        // Nowhere to send to: the field is shown, but closed.
        if self.hermes_missing {
            return h_flex()
                .w_full()
                .min_h(super::bar_h(cx))
                .px_4()
                .items_center()
                .bg(theme.background)
                .border_t_1()
                .border_color(theme.border)
                .text_sm()
                .text_color(theme.muted_foreground.opacity(0.7))
                .child("Install Hermes Agent to start a chat")
                .into_any_element();
        }
        let busy = self.live.is_some();
        let recording = self.recorder.is_some();
        let has_text =
            !self.composer.read(cx).value().trim().is_empty() || !self.attachments.is_empty();

        // The round buttons in a square UI: a quiet one (outline) and a filled one. Glyphs are
        // Lucide at 15px in a "bold" cut (assets/icons/*-bold, mic-fill: the mic's capsule
        // filled, a plain two-loop clip; the clip and the arrow at 2.25 instead of 2); Stop is a
        // filled square, not an outline (an outlined square reads as missing).
        let round =
            |id: &'static str, icon: Option<Ico>, tip: &'static str, filled: Option<Hsla>| {
                // Outlined: a lighter grey than the muted text, so the bold cut stays quiet;
                // hover brings the text color.
                let fg = if filled.is_some() {
                    theme.primary_foreground
                } else {
                    theme.muted_foreground.opacity(0.75)
                };
                div()
                    .id(id)
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .text_color(fg)
                    .map(|d| match filled {
                        Some(bg) => d.bg(bg).hover(|s| s.opacity(0.88)),
                        None => d
                            .border_1()
                            .border_color(theme.border)
                            .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground)),
                    })
                    .map(|d| match icon {
                        // 15px: the bold cut reads at a smaller size than the UI's thin 16px.
                        Some(Ico::Loader) => d.child(Ico::Loader.live(true)),
                        Some(icon) => d.child(icon.icon().size(px(15.))),
                        None => d.child(div().size(px(10.)).rounded(px(1.5)).bg(fg)),
                    })
                    .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
            };

        let mic = if self.transcribing {
            round("mic", Some(Ico::Loader), "Transcribing…", None)
        } else if recording {
            round("mic", None, "Stop and transcribe", Some(theme.danger))
        } else {
            round("mic", Some(Ico::MicFill), "Dictate", None)
        }
        .on_click(cx.listener(|this, _, window, cx| this.toggle_mic(window, cx)));

        let action = if busy {
            round("stop", None, "Stop  ⌘.", Some(theme.foreground))
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            round(
                "send",
                Some(Ico::ArrowUpBold),
                "Send (Enter)",
                // While dictating it sends what is being said.
                (has_text || recording || self.transcribing).then_some(theme.primary),
            )
            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
        };

        let slash = self.render_slash(cx);
        let clip = round(
            "attach",
            Some(Ico::PaperclipBold),
            "Attach files (or drop them, or ⌘V)",
            None,
        )
        .on_click(cx.listener(|this, _, _, cx| this.pick_attachments(cx)));
        let chips: Vec<AnyElement> = self
            .attachments
            .clone()
            .iter()
            .enumerate()
            .map(|(k, p)| {
                self.attachment_chip(
                    format!("attach-{k}").into(),
                    &p.to_string_lossy(),
                    Some(k),
                    cx,
                )
            })
            .collect();

        v_flex()
            .id("composer")
            .relative()
            .w_full()
            .flex_none()
            .bg(theme.background)
            // The command list takes ↑ ↓ Tab Esc before the text field does; ⌘V of a
            // screenshot or of Finder files attaches them.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let k = &ev.keystroke;
                if !k.modifiers.modified() && this.slash_key(k.key.as_str(), window, cx) {
                    cx.stop_propagation();
                }
                let m = &k.modifiers;
                if k.key == "v"
                    && m.platform
                    && !m.shift
                    && !m.alt
                    && !m.control
                    && this.paste_attachments(cx)
                {
                    cx.stop_propagation();
                }
            }))
            .children(slash)
            .border_t_1()
            .border_color(theme.border)
            // Files waiting to go: one row of chips over the text, at the text's inset.
            .when(!chips.is_empty(), |d| {
                d.child(
                    h_flex()
                        .flex_wrap()
                        .gap_1p5()
                        .px(px(14.))
                        .pt_2()
                        .children(chips),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .min_h(super::bar_h(cx))
                    .items_center()
                    .when(recording, |d| d.child(self.render_wave(&theme)))
                    .when(!recording, |d| {
                        d.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                // Measured: the "New session" button sits 14pt from the divider;
                                // the textarea pads its text by ~10.5 itself, so 3.5 more lands
                                // the text at 14.
                                .pl(px(3.5))
                                .py_1()
                                .child(Textarea::new(&self.composer).appearance(false)),
                        )
                    })
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_2()
                            .pl_2()
                            // A little more room right of the send button than the 8 it had.
                            .pr_3()
                            .items_center()
                            .when(!recording, |d| d.child(clip))
                            .child(mic)
                            .child(action),
                    ),
            )
            .into_any_element()
    }
}
