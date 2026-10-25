//! The input: the thread's next message — the person's avatar and the text,
//! at the messages' size, growing with what is typed. No buttons (⌘Enter sends, ⌘⇧D dictates,
//! files are dropped); stopping the agent is the status bar's control.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

/// The avatar's offset from the row's top (it replaces the avatar's own): the textarea's line
/// sits higher than a message's, so the avatar goes up to center on it (the caret).

const AVATAR_TOP: f32 = -2.5;

/// Rows the command list shows before it scrolls, and their height.
const SLASH_ROWS: f32 = 8.;
const SLASH_ROW: f32 = 30.;

impl BenchApp {
    /// Commands matching a bare `/word`, in a small list right above the input.
    fn render_slash(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let matches = self.slash_matches(cx);
        if matches.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let sel = self.slash_sel.min(matches.len() - 1);
        // Under the line, or above it when the window has no room below (the status bar is
        // there): the list's own height decides.
        let list_h = (matches.len() as f32).min(SLASH_ROWS) * SLASH_ROW + 10.;
        let below = f32::from(self.composer_y) + super::TURN_Y + 30.;
        let room = f32::from(self.viewport_h) - 36. - below;
        let top = if room < list_h && f32::from(self.composer_y) > list_h + 48. {
            super::TURN_Y - list_h - 6.
        } else {
            super::TURN_Y + 30.
        };
        let rows = matches.into_iter().enumerate().map(|(i, c)| {
            let on = i == sel;
            h_flex()
                .id(("slash", i))
                .h(px(SLASH_ROW))
                .px_3()
                .gap_3()
                .items_center()
                .text_base()
                .cursor_pointer()
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(div().flex_none().child(c.name))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(c.desc),
                )
                .child(
                    div()
                        .flex_none()
                        .text_sm()
                        .text_color(theme.muted_foreground.opacity(0.7))
                        .child(c.category),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.pick_slash(i, window, cx)))
        });
        Some(
            // Under the line being typed, from where the text starts: the composer
            // is the thread's last row, and the thread scrolls on past it, so there is room below.
            div()
                .absolute()
                .top(px(top))
                .left(px(super::CHAT_TEXT - 4.))
                .w(px(560.))
                .child(
                    div()
                        .bg(theme.popover)
                        .border_1()
                        .rounded_md()
                        .border_color(theme.border)
                        .py_1()
                        .child(
                            v_flex()
                                .id("slash-list")
                                .max_h(px(SLASH_ROW * SLASH_ROWS))
                                .overflow_y_scroll()
                                .track_scroll(&self.slash_scroll)
                                .children(rows),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_composer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // No buttons here: ⌘Enter sends, ⌘⇧D (or the status bar's mic) dictates, files are
        // dropped or pasted. Stopping the agent is the status bar's control, by the paperclip.
        let slash = self.render_slash(cx);
        let mut chips: Vec<AnyElement> = vec![];
        chips.extend(self.attachments.clone().iter().enumerate().map(|(k, p)| {
            self.attachment_chip(
                format!("attach-{k}").into(),
                &p.to_string_lossy(),
                Some(k),
                cx,
            )
        }));

        let view = cx.entity();
        v_flex()
            .id("composer")
            .relative()
            // Where it is in the window, for the `/` list (above or under the line), and how
            // tall it is: the thread above scrolls past its last row by the room the window has
            // left over, so the composer's height is part of that arithmetic.
            .child(
                canvas(
                    move |bounds, _, cx| {
                        view.update(cx, |this, cx| {
                            this.composer_y = bounds.origin.y;
                            let h = f32::from(bounds.size.height);
                            if (this.composer_h - h).abs() > 0.5 {
                                this.composer_h = h;
                                // The thread's scroll room depends on it: one more frame, settled.
                                cx.notify();
                            }
                        })
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .w_full()
            .flex_none()
            .bg(theme.background)
            // ⌘V of a screenshot or of Finder files attaches them. The command list's ↑ ↓ Tab
            // Esc are answered at the root (app/keys.rs), where every key is seen first.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                let k = &ev.keystroke;
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
            // The next turn of the thread: it is the thread's last row (it
            // scrolls with it). Avatar, the text and
            // the buttons sit on one line; the text starts where messages' text does and grows
            // down. Enter is a new line, ⌘Enter sends; the buttons — an answer's own small ones —
            // show on hover (⌘⇧D dictates, files are dropped or pasted), a Stop that matters
            // always.
            .child(
                h_flex()
                    .id("composer-row")
                    .group("composer-row")
                    .w_full()
                    .pl(px(super::CHAT_LEFT))
                    .pr(px(super::CHAT_RIGHT))
                    .py(px(super::TURN_Y))
                    .gap(px(super::CHAT_TEXT - super::CHAT_LEFT - super::AVATAR))
                    .items_start()
                    .text_base()
                    // The person's avatar even in a new chat: an empty sheet with no sign of whose
                    // turn it is was not clear (the user, after trying a bare caret).
                    .child(super::avatar(false, &theme).mt(px(AVATAR_TOP)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(10.))
                            // Files waiting to go, over the text — a little more air than before,
                            // so a picture's square never reads as part of the first line.
                            .when(!chips.is_empty(), |d| {
                                d.child(h_flex().flex_wrap().gap_2().pt(px(6.)).children(chips))
                            })
                            .child(
                                div().w_full().relative().child(
                                    // A Large textarea pads its text 12 left and 10 above
                                    // and below (gpui-component's input_px/py): pulled back
                                    // by exactly that, so its first line sits where a
                                    // message's does. Large = the messages' size; it grows
                                    // with the text (no scroll of its own: the thread
                                    // scrolls).
                                    div().ml(px(-12.)).my(px(-10.)).child(
                                        Textarea::new(&self.composer).appearance(false).large(),
                                    ),
                                ),
                            ),
                    ),
            )
            // Last and deferred: painted over the rows under it.
            .children(slash.map(|s| deferred(s).with_priority(1)))
            .into_any_element()
    }
}
