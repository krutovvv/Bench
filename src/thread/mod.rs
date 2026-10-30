//! A thread on screen: its `ThreadView` (`view.rs`, an entity of its own) draws `gpui::list` over
//! the rows of `crate::thread` (the journal).
//!
//! The list holds: the head (a project's environment, a form, "Loading…"), one item per `Row`,
//! the tail (approval, question, processes, the composer), and the room past the end that
//! lets the last row reach the top, as in an editor. Only what is on screen is laid out.
//!
//! Files: `view.rs` (the view: its state, composer, events), `active.rs` (the thread's state, one
//! struct, and `ThreadRef`), `list.rs` (the list's state and scroll; its scrollbar is the kit's),
//! `meters.rs` and `speed.rs` (its context and speed, in the title bar); `rows/` what the
//! list shows, `composer/` the next message; `open.rs` (opening, creating and deleting threads:
//! the app's), `store.rs` (every agent's threads, drafts, the journals seen), `panel.rs` (the
//! right dock's thread panel), `beside.rs` (a second thread beside the open one).

pub(crate) mod store;
mod active;
pub(crate) mod composer;
pub(crate) mod rows;
mod beside;
mod list;
pub(crate) mod look;
mod meters;
mod open;
mod panel;
pub(crate) mod speed;
mod view;

pub use active::{ActiveThread, ThreadRef};
pub use list::View;
pub use view::ThreadView;

use std::time::Duration;

use ::thread::Row;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bench_ui::icons::Ico;

/// A change of the room past the end smaller than this is rounding, not a new height.
const ROOM_EPSILON: f32 = 0.5;
/// A press and its release closer than this are a click, not a drag (no text is selected).
const CLICK_SLOP: f32 = 4.;

impl ThreadView {
    /// The rows of the thread into the list, if the thread changed since the last frame.
    fn sync_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.thread.tv.reveal_after_send();
        let version = self.thread.journal.version();
        let owner = self.thread.id.as_ref().map(|i| i.to_string());
        if version != self.thread.tv.version || owner != self.thread.tv.owner {
            self.thread.tv.version = version;
            let rows = self.thread.journal.rows();
            self.carry_over(&rows);
            let follow = std::mem::take(&mut self.follow);
            let changed = self.thread.tv.sync(owner.as_deref(), rows, follow);
            self.parse_ahead(changed, cx);
            // The tail's new height is known only after this frame's layout: look again then.
            window.request_animation_frame();
        }
        // The list keeps an item's first measured height: the room is measured again when it
        // changes (the composer shrank after sending, the window was resized).
        let room = self.thread.tv.room_past_end(self.viewport_h);
        if std::mem::take(&mut self.thread.tv.jump) {
            // `to_end`: this frame lays out without the room.
            window.request_animation_frame();
        } else if (room - self.thread.tv.room).abs() > px(ROOM_EPSILON) {
            self.thread.tv.room = room;
            let end = self.thread.tv.tail() + 1;
            self.thread.tv.list.remeasure_items(end..end + 1);
            window.request_animation_frame();
        }
        // A live run's clock moves once a second, and so does a running background process's,
        // which outlives the run.
        let ticking = |t: &::thread::Thread| t.running() || t.processes_running();
        if ticking(&self.thread.journal) && self.thread.tv.ticker.is_none() {
            self.thread.tv.ticker = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let Ok(running) = this.update(cx, |this, cx| {
                        cx.notify();
                        ticking(&this.thread.journal)
                    }) else {
                        break;
                    };
                    if !running {
                        let _ = this.update(cx, |this, _| this.thread.tv.ticker = None);
                        break;
                    }
                }
            }));
        }
    }

    pub(crate) fn render_chat(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.viewport_h = window.viewport_size().height;
        self.sync_rows(window, cx);
        // `BENCH_TRACE_SCROLL=1`: where the thread's content stands each frame (a jerk shows).
        static TRACE_SCROLL: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| std::env::var("BENCH_TRACE_SCROLL").is_ok());
        if *TRACE_SCROLL {
            let tv = &self.thread.tv;
            let top = tv.list.logical_scroll_top();
            let first = tv.list.bounds_for_item(top.item_ix).map(|b| b.top());
            let heights: Vec<i32> = (1..=tv.tail())
                .map(|i| {
                    tv.list
                        .bounds_for_item(i)
                        .map_or(-1, |b| f32::from(b.size.height) as i32)
                })
                .collect();
            eprintln!(
                "scroll: item {} offset {:?} first-top {:?} rows {} room {:?} composer-bottom {:?} viewport-bottom {:?} heights {:?}",
                top.item_ix,
                top.offset_in_item,
                first,
                tv.rows.len(),
                tv.room,
                tv.list.bounds_for_item(tv.tail()).map(|b| b.bottom()),
                tv.list.viewport_bounds().bottom(),
                heights
            );
        }
        let theme = cx.theme().clone();
        let tail = self.thread.tv.tail();
        // Unknown for the frame after rows were remeasured: the button stays as it was.
        if let Some(b) = self.thread.tv.list.item_is_below_viewport(tail) {
            self.thread.tv.below = b;
        }
        let below = self.thread.tv.below;
        let list = list(
            self.thread.tv.list.clone(),
            cx.processor(|this, ix: usize, window, cx| this.render_item(ix, window, cx)),
        )
        .flex_1()
        .min_h_0();
        let composer = self.composer.clone();
        // The thread's own area: the whole view in the document; over the pinned composer in
        // the chat, taking what the composer leaves.
        let area = div()
            .relative()
            .w_full()
            .when(look::COMPOSER_IN_LIST, |d| d.size_full())
            .when(!look::COMPOSER_IN_LIST, |d| d.flex_1().min_h_0())
            .child(
                v_flex()
                    .id("chat")
                    .size_full()
                    // A click on the thread itself means "I'm about to type": the composer takes
                    // the focus. What handles a press or a click (a fold, a button, an insert's
                    // terminal) stops its propagation, so it never gets here; a mouse that moved
                    // (a drag selects text) or a focus that moved leaves things as they are.
                    .capture_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, _, _| {
                        this.press = None;
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                            this.press = Some((ev.position, window.focused(cx)));
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseUpEvent, window, cx| {
                            let Some((at, focused)) = this.press.take() else {
                                return;
                            };
                            let still = (at.x - ev.position.x).abs() < px(CLICK_SLOP)
                                && (at.y - ev.position.y).abs() < px(CLICK_SLOP);
                            if still && focused == window.focused(cx) {
                                composer.update(cx, |s, cx| s.focus(window, cx));
                            }
                        }),
                    )
                    .child(list),
            )
            // A plain scrollbar, as a Mac's: thin, over the edge, shown while scrolling.
            .child(gpui_kit::component::scroll::Scrollbar::vertical(&self.thread.tv.list))
            .when(below, |d| {
                // A small round pill over the thread's middle, as macOS shows "new messages":
                // it floats, so it is a popover's ground and shadow, not a bordered button.
                d.child(
                    h_flex()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_4()
                        .justify_center()
                        .child(
                            h_flex()
                                .id("jump")
                                .h(px(bench_ui::CHIP_H))
                                .px_3()
                                .gap_1()
                                .items_center()
                                .rounded_full()
                                .bg(theme.popover)
                                .border_1()
                                .border_color(theme.border.opacity(0.7))
                                .shadow_md()
                                .text_size(px(bench_ui::BAR_TEXT))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.muted_foreground)
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.foreground))
                                .child(Ico::ArrowDown.icon().xsmall())
                                .child("Jump to latest")
                                .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx))),
                        ),
                )
            });
        if look::COMPOSER_IN_LIST {
            return area.into_any_element();
        }
        // The chat: the composer pinned under the list, in the thread's column, with the agent's
        // wait said over it while the person is elsewhere in the thread.
        let waiting = !self.thread.journal.asks().is_empty() && below;
        let composer = self.render_composer(cx);
        let foot = self.render_composer_foot(cx);
        let view = cx.entity();
        let side = if self.narrow {
            bench_ui::CHAT_LEFT_NARROW
        } else {
            bench_ui::CHAT_LEFT
        };
        v_flex()
            .size_full()
            .relative()
            // Its own width, measured: a dock's is narrow, and its sides come in.
            .child(
                canvas(
                    move |bounds, _, cx| {
                        view.update(cx, |this, cx| {
                            let narrow = f32::from(bounds.size.width) < bench_ui::THREAD_NARROW_W;
                            if this.narrow != narrow {
                                this.narrow = narrow;
                                // The rows' widths change, not the list's: measured again.
                                let n = this.thread.tv.list.item_count();
                                this.thread.tv.list.remeasure_items(0..n);
                                cx.notify();
                            }
                        })
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(area)
            .child(
                h_flex().w_full().flex_none().justify_center().child(
                    v_flex()
                        .w_full()
                        .max_w(px(bench_ui::THREAD_MAX_W))
                        .px(px(side))
                        .pb(px(bench_ui::COMPOSER_CARD_PB))
                        // The line under it close, as Claude's: its chips' hover is the air.
                        .gap_1()
                        .when(waiting, |d| d.child(div().pb_1().child(self.render_waiting_line(cx))))
                        .child(composer)
                        .child(foot),
                ),
            )
            .into_any_element()
    }

    /// Over the pinned composer while the agent waits for an answer and the question is out of
    /// sight: one line, a click brings the question into view.
    fn render_waiting_line(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        h_flex()
            .id("waiting")
            .h(px(bench_ui::CHIP_H))
            .px_3()
            .gap_2()
            .items_center()
            .rounded(px(bench_ui::COMPOSER_CARD_R))
            .bg(theme.warning.opacity(0.12))
            .text_size(px(bench_ui::THREAD_META))
            .text_color(theme.foreground)
            .cursor_pointer()
            .child(bench_ui::dot(theme.warning))
            .child("The agent is waiting for your answer")
            .child(div().flex_1())
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("Show"),
            )
            .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx)))
            .into_any_element()
    }

    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.thread.tv.to_end();
        cx.notify();
    }

    /// What the live rows had becomes their entries': the answer being written passes its parsed
    /// texts to the answer that replaces it; the live work's line keeps its open-or-shut state
    /// under its new key. Made anew, either would change on screen at the run's end.
    fn carry_over(&mut self, rows: &[Row]) {
        // What the person left open or shut on the live work, by thread: its rows change keys
        // as the run goes on, and coming back to the thread finds it as left.
        let mut left = self
            .thread
            .id
            .as_ref()
            .and_then(|id| self.work_left.get_mut(id.as_str()));
        let t = &mut self.thread;
        for (k, old) in t.tv.rows.iter().enumerate() {
            match (old, rows.get(k)) {
                (Row::Writing { .. }, Some(Row::Answer { key, .. })) => {
                    t.texts.rename("answer-writing-", &format!("answer-{key}-"));
                    t.texts.rename("code-writing-", &format!("code-{key}-"));
                }
                (Row::Work { key: was, .. }, Some(Row::Work { key, .. })) if was != key => {
                    if t.open_work.remove(was) {
                        t.open_work.insert(key.clone());
                    }
                    if let Some(left) = left.as_deref_mut() {
                        if left.remove(was) {
                            left.insert(key.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        // Nothing opens on its own: a work row shows its counts and, while the run lives, what
        // the agent is doing now; its steps are the person's to open (`rows/work.rs`).
    }

    /// Start parsing the texts of the rows from `from` on, before they are on screen: a row
    /// scrolled into view then has its whole height in its first frame.
    fn parse_ahead(&mut self, from: usize, cx: &mut Context<Self>) {
        let t = &mut self.thread;
        for row in t.tv.rows[from.min(t.tv.rows.len())..].to_vec() {
            match row {
                Row::User { key, text, .. } => {
                    let (said, _) = crate::media::attach::split(&text);
                    if !said.is_empty() {
                        t.texts
                            .state(&key, &crate::media::text::escape_plain(&said), cx);
                    }
                }
                Row::Answer { key, text, .. } => {
                    for (k, seg) in crate::media::text::answer_segments(&text)
                        .into_iter()
                        .enumerate()
                    {
                        if let crate::media::Segment::Text(t2) = seg {
                            t.texts.state(&format!("answer-{key}-{k}"), &t2, cx);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// One item of the list.
    /// A row of the list in the thread's column: as wide as the pane up to `THREAD_MAX_W`, then
    /// centered — past a width, a line is too long to read and the thread stops growing.
    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item = self.render_list_item(ix, window, cx);
        let row = h_flex()
            .justify_center()
            .child(div().w_full().max_w(px(bench_ui::THREAD_MAX_W)).child(item));
        if self.narrow {
            // Narrow: every row's sides (CHAT_LEFT, set by each row) pulled out to the narrow
            // ones at once — the row stretched past the list's edges by the difference, which
            // the list clips.
            let pull = bench_ui::CHAT_LEFT - bench_ui::CHAT_LEFT_NARROW;
            v_flex()
                .w_full()
                .child(row.mx(px(-pull)))
                .into_any_element()
        } else {
            row.w_full().into_any_element()
        }
    }

    fn render_list_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let n = self.thread.tv.rows.len();
        if ix == 0 {
            // The air above the first row: with the row's own, as much as between two rows.
            // Scrolled to the end, the row above the composer shows exactly its own air here.
            div()
                .pt(px(bench_ui::TURN_Y))
                .child(self.render_head(window, cx))
                .into_any_element()
        } else if ix <= n {
            self.render_row(ix - 1, window, cx)
        } else if ix == n + 1 {
            self.render_tail(window, cx)
        } else {
            div().h(self.thread.tv.room).into_any_element()
        }
    }

    fn render_row(&mut self, k: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let row = self.thread.tv.rows[k].clone();
        // An answer right after the work that led to it is that work's continuation: no avatar.
        let after_work = k > 0 && matches!(self.thread.tv.rows[k - 1], Row::Work { .. });
        match row {
            Row::User { key, text, .. } => self.render_user(&key, &text, cx),
            Row::Work {
                key,
                steps,
                from,
                to,
                live,
                holds,
                plan,
                asks,
                processes,
            } => self.render_work(
                &key,
                &steps,
                from,
                to,
                live,
                &holds,
                plan.as_ref(),
                &asks,
                &processes,
                // The answer that follows this work: its read-aloud and copy ride this
                // line's far edge — the very place Pause and Stop held while the run lived.
                match self.thread.tv.rows.get(k + 1) {
                    Some(Row::Answer { key: ak, text, .. }) if text.trim() != "" => {
                        Some((ak.clone(), text.clone()))
                    }
                    _ => None,
                },
                cx,
            ),
            Row::Answer { key, text, error } => {
                self.render_agent_answer(k, &key, &text, error.as_deref(), after_work, cx)
            }
            Row::Writing { text } => self.render_writing(&text, after_work, cx),
            Row::Fail { text, .. } => self.render_fail(&text, cx),
            Row::Changes {
                key,
                root,
                base,
                head,
                files,
                review,
            } => self.render_turn_changes(
                &key,
                crate::explorer::pane::doc::Turn { root, base, head },
                &files,
                review,
                cx,
            ),
            Row::Insert { id, .. } => {
                let els = self.render_insert_by_id(&id, window, cx);
                v_flex().w_full().children(els).into_any_element()
            }
            Row::Queued {
                key,
                sub,
                mode,
                text,
            } => self.render_queued(&key, &sub, mode, &text, window, cx),
        }
    }

    /// Before the first row: why the thread is empty, or the new chat's folder being typed.
    fn render_head(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<AnyElement> = vec![];
        let empty = self.thread.tv.rows.is_empty();
        let path_entry = self
            .app(cx)
            .filter(|a| a.new_project.path_entry)
            .map(|a| a.new_project.path.clone());
        if empty {
            if let (Some(_), Some(why)) = (
                self.thread.id.clone(),
                self.thread.history_error.clone(),
            ) {
                rows.push(
                    h_flex()
                        .w_full()
                        .pl(px(look::text_x()))
                        .pr(px(look::text_right()))
                        .py(px(bench_ui::TURN_Y))
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_base()
                                .text_color(theme.danger)
                                .child(format!("Could not open the thread: {why}")),
                        )
                        .child(
                            bench_ui::form_action("history-retry", "Retry", false, cx)
                                .on_click(cx.listener(|this, _, _, cx| this.reopen(cx))),
                        )
                        .into_any_element(),
                );
            } else if self.thread.id.is_some() && !self.thread.loaded {
                rows.push(
                    h_flex()
                        .w_full()
                        .pl(px(look::text_x()))
                        .py(px(bench_ui::TURN_Y))
                        .child(
                            gpui_kit::component::shimmer::ShimmerText::new("Loading…")
                                .text_base()
                                .text_color(theme.muted_foreground),
                        )
                        .into_any_element(),
                );
            } else if let Some(field) = path_entry {
                rows.push(
                    h_flex()
                        .w_full()
                        .pl(px(look::text_x()))
                        .pr(px(look::text_right()))
                        .py(px(bench_ui::TURN_Y))
                        .child(
                            div()
                                .w(px(bench_ui::PATH_FIELD_W))
                                .child(gpui_kit::component::input::Input::new(&field).small()),
                        )
                        .into_any_element(),
                );
            } else if self.thread.id.is_none() && !look::COMPOSER_IN_LIST {
                // A new chat: Bench's welcome and the first steps (`rows/welcome.rs`).
                rows.push(self.render_welcome(window, cx));
            }
        }
        v_flex().w_full().children(rows).into_any_element()
    }

    /// The thread's history asked again (Retry): what it shows stays until it comes.
    fn reopen(&mut self, cx: &mut Context<Self>) {
        if self.thread.id.is_none() {
            return;
        }
        self.thread.history_error = None;
        let at = self.thread_ref();
        self.app.update(cx, |app, cx| app.fetch_thread(at, cx)).ok();
        cx.notify();
    }

    /// The list's last item: the composer in the document; in the chat (where the composer is
    /// pinned under the list) a row of air, so the last message does not touch the edge.
    fn render_tail(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if !look::COMPOSER_IN_LIST {
            return div().h(px(bench_ui::TURN_Y)).into_any_element();
        }
        let composer = self.render_composer(cx);
        v_flex().w_full().child(composer).into_any_element()
    }
}
