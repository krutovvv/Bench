//! The input: the thread's next message — the person's avatar and the text,
//! at the messages' size, growing with what is typed. No buttons (⌘Enter sends, ⌘⇧D dictates,
//! files are dropped); stopping the agent is the status bar's control.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bench_ui::icons::Ico;

use super::ThreadView;

/// Rows the command list shows before it scrolls, and their height.
const SLASH_ROWS: f32 = 8.;
const SLASH_ROW: f32 = 30.;

impl ThreadView {
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
        let below = f32::from(self.composer_y) + bench_ui::TURN_Y + 30.;
        let room = f32::from(self.viewport_h) - 36. - below;
        let top = if room < list_h && f32::from(self.composer_y) > list_h + 48. {
            bench_ui::TURN_Y - list_h - 6.
        } else {
            bench_ui::TURN_Y + 30.
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
                .left(px(bench_ui::CHAT_TEXT - 4.))
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

    pub(crate) fn render_composer(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
        // What the Explorer sent (⌘L, a dropped tab, problems): its file and lines, or the problems.
        chips.extend(self.thread_references(cx).iter().enumerate().map(|(k, r)| {
            let close = div()
                .id(("ref-x", k))
                .flex_none()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .child(bench_ui::icons::Ico::X.icon().xsmall())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.remove_reference(k, cx)
                }));
            let (ico, color) = r.icon(&theme);
            h_flex()
                .id(("ref", k))
                .h(px(bench_ui::CHIP_H))
                .px_1p5()
                .gap_1()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .text_sm()
                .child(ico.icon().xsmall().text_color(color))
                .child(r.label())
                .child(close)
                .into_any_element()
        }));

        let view = cx.entity();
        // Attach and dictate, the writer's own two: while the field is empty they ride the
        // text's opposite edge, centred on the avatar; once words come they step down to the
        // last line's side, under the text. The field reserves their gutter always, so they
        // never cover a word and nothing jumps when they move. Hidden until the composer is
        // hovered — but a live recording or transcription stays on show, in quiet colours:
        // the moving level bar already says "recording", red would only shout it twice.
        let app = self.app.upgrade();
        let muted = theme.muted_foreground;
        let rec = app.as_ref().is_some_and(|a| a.read(cx).recorder.is_some());
        let trans = app.as_ref().is_some_and(|a| a.read(cx).transcribing);
        let level = app
            .as_ref()
            .and_then(|a| a.read(cx).recorder.as_ref().map(|r| r.level().min(1.)))
            .unwrap_or(0.);
        let empty = self.composer.read(cx).value().trim().is_empty();
        // Empty: level with the avatar. Words: under them, at the last line's side. The
        // bottom is computed from the composer's measured height rather than anchored
        // with `bottom`, whose stretch semantics fought the fixed height.
        let row_h = self.composer_h.max(2. * bench_ui::TURN_Y + bench_ui::AVATAR);
        let top = if empty {
            bench_ui::TURN_Y
        } else {
            (row_h - bench_ui::TURN_Y - bench_ui::AVATAR).max(bench_ui::TURN_Y)
        };
        let tools = h_flex()
            .absolute()
            .right_0()
            .top(px(top))
            .h(px(bench_ui::AVATAR))
            .items_center()
            .gap_1()
            .when(!(rec || trans) && std::env::var("BENCH_TOOLS_SHOWN").is_err(), |d| {
                d.opacity(0.).group_hover("composer-row", |s| s.opacity(1.))
            })
            .child(
                bench_ui::icon_button("composer-attach", Ico::Paperclip, "Attach files", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_attachments(window, cx)
                    })),
            )
            .child(if trans {
                div()
                    .id("composer-mic")
                    .flex_none()
                    .p_1()
                    .text_color(muted)
                    .child(
                        gpui_kit::component::spinner::Spinner::new()
                            .icon(Ico::Loader.icon())
                            .small(),
                    )
                    .tooltip(|window, cx| {
                        Tooltip::new("Transcribing…").build(window, cx)
                    })
                    .into_any_element()
            } else if rec {
                div()
                    .id("composer-mic")
                    .flex_none()
                    .p_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(muted)
                    .child(
                        div().relative().child(Ico::Mic.icon().small()).child(
                            div()
                                .absolute()
                                .right(px(-2.))
                                .bottom(px(1.))
                                .w(px(2.))
                                .h(px(2. + level * 9.))
                                .rounded(px(1.))
                                .bg(theme.foreground),
                        ),
                    )
                    .tooltip(|window, cx| {
                        Tooltip::new("Stop and transcribe  ⌘⇧D").build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(app) = this.app.upgrade() {
                            app.update(cx, |app, cx| app.toggle_mic(window, cx));
                        }
                    }))
                    .into_any_element()
            } else {
                bench_ui::icon_button("composer-mic", Ico::Mic, "Dictate  ⌘⇧D", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(app) = this.app.upgrade() {
                            app.update(cx, |app, cx| app.toggle_mic(window, cx));
                        }
                    }))
                    .into_any_element()
            });
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
            // No background of its own: it is the thread's last row and sits on the page, so a
            // fill here only ever showed — a white slab under the words when a dropped file
            // tints the chat area.
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
                    .relative()
                    .w_full()
                    .pl(px(bench_ui::CHAT_LEFT))
                    .pr(px(bench_ui::CHAT_RIGHT))
                    .py(px(bench_ui::TURN_Y))
                    .gap(px(bench_ui::CHAT_TEXT
                        - bench_ui::CHAT_LEFT
                        - bench_ui::AVATAR))
                    .items_start()
                    .text_base()
                    // The person's avatar even in a new chat: an empty sheet with no sign of whose
                    // turn it is was not clear (the user, after trying a bare caret).
                    .child(bench_ui::avatar(false, &theme))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(10.))
                            .child(
                                div().w_full().relative().child(
                                    // A Large textarea pads its text 12 left and 10 above
                                    // and below (gpui-component's input_px/py): pulled back
                                    // by exactly that, so its first line sits where a
                                    // message's does. Large = the messages' size; it grows
                                    // with the text (no scroll of its own: the thread
                                    // scrolls). The leading is set on the field itself — the
                                    // library's input pins a line height inside — and it must
                                    // be given in REM: the widget builds its rows from an
                                    // absolute leading and lets a relative one fall through to
                                    // the font's own 1.41 em, which set typed lines 3 px
                                    // tighter than the thread's. 1.5rem is the thread's leading
                                    // exactly, since the thread's prose is 1rem.
                                    div()
                                        .ml(px(-12.))
                                        .mr(px(56.))
                                        .my(px(-10.))
                                        .child(
                                        Textarea::new(&self.composer)
                                            .appearance(false)
                                            .large()
                                            .line_height(rems(bench_ui::LEAD)),
                                    ),
                                ),
                            )
                            // Files waiting to go, under the text — where they sit once sent,
                            // so the same message never flips while it is being written.
                            .when(!chips.is_empty(), |d| {
                                d.child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap_2()
                                        .pr(px(56.))
                                        .pb(px(6.))
                                        .children(chips),
                                )
                            }),
                    )
                    .child(tools),
            )
            // Last and deferred: painted over the rows under it.
            .children(slash.map(|s| deferred(s).with_priority(1)))
            .into_any_element()
    }
}
