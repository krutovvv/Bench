//! The input: the thread's next message — the person's avatar and the text,
//! at the messages' size, growing with what is typed. No buttons (⌘Enter sends, ⌘⇧D dictates,
//! files are dropped); stopping the agent is Send's Stop, ⌘..

use gpui_kit::component::input::Textarea;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bench_ui::icons::Ico;

use crate::thread::ThreadView;

/// The `/` list's scrollable body: its rows in a window of `rows` rows, and the wheel is the
/// list's alone — the thread behind it must not run away while the person picks a command.
/// gpui hands a wheel to every scrollable under the pointer at once, so the list scrolls here,
/// clamped to what it has, and the event stops (`rows` is how many fit, `row_h` their height).
fn slash_body(
    id: &'static str,
    row_h: f32,
    rows: f32,
    scroll: &ScrollHandle,
    items: Vec<AnyElement>,
) -> AnyElement {
    let handle = scroll.clone();
    v_flex()
        .id(id)
        .max_h(px(row_h * rows))
        .overflow_y_scroll()
        .track_scroll(&handle)
        .on_scroll_wheel(move |ev: &ScrollWheelEvent, _, cx| {
            let line_h = px(row_h);
            let dy = f32::from(ev.delta.pixel_delta(line_h).y);
            let max = f32::from(handle.max_offset().y);
            let next = (f32::from(handle.offset().y) + dy).clamp(-max, 0.).round();
            handle.set_offset(point(px(0.), px(next)));
            cx.stop_propagation();
        })
        .children(items)
        .into_any_element()
}


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
        if !crate::thread::look::COMPOSER_IN_LIST {
            return Some(self.render_completions(matches, sel, &theme, cx));
        }
        // Under the line, or above it when the window has no room below (the window's edge is
        // there): the list's own height decides.
        let list_h = (matches.len() as f32).min(SLASH_ROWS) * SLASH_ROW + 10.;
        let below = f32::from(self.composer_y) + bench_ui::TURN_Y + 30.;
        let room = f32::from(self.viewport_h) - 36. - below;
        let top = if room < list_h && f32::from(self.composer_y) > list_h + 48. {
            bench_ui::TURN_Y - list_h - 6.
        } else {
            bench_ui::TURN_Y + 30.
        };
        let rows: Vec<AnyElement> = matches.into_iter().enumerate().map(|(i, c)| {
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
                .into_any_element()
        })
        .collect();
        Some(
            // Under the line being typed, from where the text starts: the composer
            // is the thread's last row, and the thread scrolls on past it, so there is room below.
            div()
                .absolute()
                .top(px(top))
                .left(px(if crate::thread::look::COMPOSER_IN_LIST {
                    bench_ui::CHAT_TEXT - 4.
                } else {
                    bench_ui::INPUT_PAD_X - 4.
                }))
                .w(px(bench_ui::SLASH_LIST_W))
                .child(
                    div()
                        .bg(theme.popover)
                        .border_1()
                        .rounded_md()
                        .border_color(theme.border)
                        .py_1()
                        .child(slash_body("slash-list", SLASH_ROW, SLASH_ROWS, &self.slash_scroll, rows)),
                )
                .into_any_element(),
        )
    }

    /// The chat's "/" list as a Mac's completion list: right over the field and as wide, one
    /// selection — the accent with white text, as a menu's — that follows the pointer too, the
    /// names at the sidebar's size, what each is quieter, its kind faint at the end.
    fn render_completions(
        &mut self,
        matches: Vec<crate::thread::composer::slash::SlashCommand>,
        sel: usize,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows: Vec<AnyElement> = matches.into_iter().enumerate().map(|(i, c)| {
            let on = i == sel;
            let (fg, quiet, faint) = if on {
                (
                    theme.primary_foreground,
                    theme.primary_foreground.opacity(0.85),
                    theme.primary_foreground.opacity(0.7),
                )
            } else {
                (theme.foreground, theme.muted_foreground, bench_ui::faint(theme))
            };
            h_flex()
                .id(("slash", i))
                .flex_none()
                .h(px(bench_ui::COMPLETION_ROW_H))
                .px_2()
                .gap_2()
                .items_center()
                .rounded_md()
                .text_size(px(bench_ui::SIDEBAR_TEXT))
                .cursor_pointer()
                .when(on, |d| d.bg(theme.primary))
                .child(div().flex_none().text_color(fg).child(c.name))
                .child(div().flex_1().min_w_0().truncate().text_color(quiet).child(c.desc))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(bench_ui::SIDEBAR_HEAD_TEXT))
                        .text_color(faint)
                        .child(c.category),
                )
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if this.slash_sel != i {
                        this.slash_sel = i;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.pick_slash(i, window, cx)))
                .into_any_element()
        })
        .collect();
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_full()
            .mb(px(bench_ui::COMPLETION_GAP))
            .child(
                div()
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border.opacity(0.7))
                    .rounded_lg()
                    .shadow_lg()
                    .p_1()
                    .child(slash_body(
                        "slash-list",
                        bench_ui::COMPLETION_ROW_H,
                        SLASH_ROWS,
                        &self.slash_scroll,
                        rows,
                    )),
            )
            .into_any_element()
    }

    pub(crate) fn render_composer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // No buttons here: ⌘Enter sends, ⌘⇧D (or the composer's mic) dictates, files are
        // dropped or pasted. Stopping the agent is Send's Stop, ⌘..
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
        let rec = app.as_ref().is_some_and(|a| a.read(cx).media.recorder.is_some());
        let trans = app.as_ref().is_some_and(|a| a.read(cx).media.transcribing);
        let level = app
            .as_ref()
            .and_then(|a| a.read(cx).media.recorder.as_ref().map(|r| r.level().min(1.)))
            .unwrap_or(0.);
        let empty = self.composer.read(cx).value().trim().is_empty();

        // Empty: level with the avatar, at the text's right column. Once a single character
        // is there, they leave the gutter and travel below the field, a line of their own
        // at the text's left edge — where a queue chip sits under a message. The row grows
        // its bottom padding by their line, so they cover no word and the hover band
        // reaches them. The bottom is computed from the composer's measured height rather
        // than anchored with `bottom`, whose stretch semantics fought the fixed height.
        let row_h = self
            .composer_h
            .max(2. * bench_ui::TURN_Y + bench_ui::AVATAR);
        // The chat's composer is pinned under the list: its tools are a line of their own at
        // the card's foot, always shown, with Send (or Stop while the agent works) at its end.
        let in_list = crate::thread::look::COMPOSER_IN_LIST;
        // Attach: a plus in the chat (Messages' and ChatGPT's apps button), the paperclip in
        // the document.
        let attach = if in_list {
            bench_ui::icon_button("composer-attach", Ico::Paperclip, "Attach files", false, cx)
        } else {
            bench_ui::round_icon_button("composer-attach", Ico::Plus, "Attach files", cx)
        }
        .on_click(cx.listener(|this, _, window, cx| this.pick_attachments(window, cx)))
        .into_any_element();
        let mic: AnyElement = if trans {
                div()
                    .id("composer-mic")
                    .flex_none()
                    // Send's size, as the mic at rest (`round_icon_button`).
                    .size(px(bench_ui::SEND_BUTTON))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(muted)
                    .child(
                        gpui_kit::component::spinner::Spinner::new()
                            .icon(Ico::Loader.icon())
                            .small(),
                    )
                    .tooltip(|window, cx| Tooltip::new("Transcribing…").build(window, cx))
                    .into_any_element()
            } else if rec {
                div()
                    .id("composer-mic")
                    .flex_none()
                    // Send's size, as the mic at rest (`round_icon_button`).
                    .size(px(bench_ui::SEND_BUTTON))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .text_color(muted)
                    .child(
                        div().relative().child(Ico::Mic.icon().small()).child(
                            div()
                                .absolute()
                                .right(px(-2.))
                                .bottom(px(bench_ui::HAIRLINE))
                                .w(px(bench_ui::CARET_W))
                                .h(px(2. + level * 9.))
                                .rounded(px(bench_ui::HAIRLINE))
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
                if in_list {
                    bench_ui::icon_button("composer-mic", Ico::Mic, "Dictate  ⌘⇧D", false, cx)
                } else {
                    bench_ui::round_icon_button("composer-mic", Ico::Mic, "Dictate  ⌘⇧D", cx)
                }
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(app) = this.app.upgrade() {
                            app.update(cx, |app, cx| app.toggle_mic(window, cx));
                        }
                    }))
                    .into_any_element()
            };
        let (mut attach, mut mic) = (Some(attach), Some(mic));
        let tools = h_flex()
            .when(in_list, |d| d.absolute())
            .when(in_list && empty, |d| {
                // The messages' right column, not the window's edge: the icons end where
                // the thread's text ends (`right_0` measures from the row's border box,
                // past its right padding).
                d.top(px(bench_ui::TURN_Y)).right(px(bench_ui::CHAT_RIGHT))
            })
            .when(in_list && !empty, |d| {
                // Their line in the row's grown bottom band: bottom-anchored by their own
                // height, at the text's left edge.
                d.top(px(row_h - bench_ui::AVATAR - 2.))
                    .left(px(bench_ui::CHAT_TEXT))
            })
            .h(px(bench_ui::AVATAR))
            .items_center()
            .gap_1()
            .when(
                in_list && !(rec || trans) && std::env::var("BENCH_TOOLS_SHOWN").is_err(),
                |d| d.opacity(0.).group_hover("composer-row", |s| s.opacity(1.)),
            )
            .when(in_list, |d| d.children(attach.take()).children(mic.take()));
        // The chat's Send at the line's end: the accent with words to send, a quiet disc
        // without, Stop (a square) while the agent works.
        let live = self.thread.live.is_some() || self.thread.journal.running();
        let send = (!in_list).then(|| {
            div()
                .id("composer-send")
                .flex_none()
                .size(px(bench_ui::SEND_BUTTON))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .map(|d| {
                    if live {
                        d.bg(theme.foreground).child(
                            div()
                                .size(px(bench_ui::STOP_MARK))
                                .rounded(px(bench_ui::HAIRLINE * 2.))
                                .bg(theme.background),
                        )
                    } else if empty {
                        d.bg(theme.foreground.opacity(0.07))
                            .text_color(theme.muted_foreground)
                            .child(Ico::ArrowUp.icon().small())
                    } else {
                        d.bg(theme.primary)
                            .text_color(theme.primary_foreground)
                            .child(Ico::ArrowUp.icon().small())
                    }
                })
                .tooltip(move |window, cx| {
                    Tooltip::new(if live { "Stop  ⌘." } else { "Send  ⌘⏎" }).build(window, cx)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    if live {
                        if let Some(thread) = this.thread.id.clone() {
                            this.tell(protocol::StopThread { thread }, cx);
                        }
                    } else {
                        this.send(window, cx);
                    }
                }))
        });
        // A Large textarea pads its text 12 left and 10 above and below (gpui-component's
        // input_px/py): pulled back by exactly that, so its first line sits where a message's
        // does. Large = the messages' size; it grows with the text (no scroll of its own: the
        // thread scrolls). The leading is set on the field itself — the library's input pins a
        // line height inside — and it must be given in REM: the widget builds its rows from an
        // absolute leading and lets a relative one fall through to the font's own 1.41 em. 1.5rem
        // is the thread's leading exactly, since the thread's prose is 1rem.
        let field = div().w_full().relative().child(
            div()
                .ml(px(-bench_ui::INPUT_PAD_X))
                .mr(px(if in_list { bench_ui::COMPOSER_TOOLS_W } else { 0. }))
                .my(px(-bench_ui::INPUT_PAD_Y))
                .child(
                    Textarea::new(&self.composer)
                        .appearance(false)
                        .large()
                        .line_height(rems(bench_ui::LEAD)),
                ),
        );
        // Files waiting to go: under the text in the document (where they sit once sent), over
        // it in the chat's field, as Claude's.
        let chips = (!chips.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .gap_2()
                .when(in_list, |d| {
                    d.pr(px(bench_ui::COMPOSER_TOOLS_W)).pb(px(bench_ui::COMPOSER_PB))
                })
                .children(chips)
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
            .child(if in_list {
                h_flex()
                    .id("composer-row")
                    .group("composer-row")
                    .relative()
                    .w_full()
                    .pl(px(bench_ui::CHAT_LEFT))
                    .pr(px(bench_ui::CHAT_RIGHT))
                    .py(px(bench_ui::TURN_Y))
                    // The tools' line under the field, reserved while words are in it.
                    .when(!empty, |d| d.pb(px(bench_ui::COMPOSER_TOOLS_H)))
                    .gap(px(bench_ui::CHAT_TEXT - bench_ui::CHAT_LEFT - bench_ui::AVATAR))
                    .items_start()
                    .text_base()
                    // The person's avatar even in a new chat: an empty sheet with no sign of
                    // whose turn it is was not clear (the user, after trying a bare caret).
                    .child(bench_ui::avatar(false, &theme))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(bench_ui::COMPOSER_GAP))
                            .child(field)
                            .children(chips),
                    )
                    .child(tools)
                    .into_any_element()
            } else {
                // The chat: one line of a field, as Claude's — the plus, the text, the mic and
                // Send; the text grows up from the buttons, which stay on its last line. Files
                // to send ride over the text. No shadow: a Mac's text field is a rule on the
                // page, not a card over it.
                v_flex()
                    .id("composer-row")
                    .group("composer-row")
                    .w_full()
                    .px(px(bench_ui::COMPOSER_CARD_PX))
                    .py(px(bench_ui::COMPOSER_CARD_PY))
                    .gap_2()
                    .rounded(px(bench_ui::COMPOSER_CARD_R))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .text_base()
                    .children(chips.map(|c| div().px(px(bench_ui::ICON_BUTTON_PAD)).child(c)))
                    .child(
                        h_flex()
                            .w_full()
                            .items_end()
                            .gap_1()
                            // The small buttons centred on Send's height, the line they share.
                            .children(attach.take())
                            // Its line centred on the buttons' height.
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .pl(px(bench_ui::ICON_BUTTON_PAD))
                                    .py(px((bench_ui::SEND_BUTTON - bench_ui::LEAD * 14.) / 2.))
                                    .child(field),
                            )
                            .children(mic.take())
                            .children(send),
                    )
                    .into_any_element()
            })
            // Last and deferred: painted over the rows under it.
            .children(slash.map(|s| deferred(s).with_priority(1)))
            .into_any_element()
    }
}
