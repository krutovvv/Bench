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
        if self.app(cx).is_some_and(|a| a.demo) {
            self.demo_hooks(window, cx);
        }
        self.sync_rows(window, cx);
        // `BENCH_TRACE_SCROLL=1`: where the thread's content stands each frame (a jerk shows).
        if std::env::var("BENCH_TRACE_SCROLL").is_ok() {
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
        div()
            .relative()
            .size_full()
            // A file dropped anywhere on the thread — in the middle, in a dock, beside another —
            // waits in the composer with the next message (`attachments.rs`).
            .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().list_active))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                let paths = paths.paths().to_vec();
                this.add_attachments(paths, cx)
            }))
            .child(
                v_flex()
                    .id("chat")
                    .size_full()
                    // A click anywhere in the thread means "I'm about to type": the composer takes
                    // the focus, unless the click went into something that types itself, or the
                    // mouse moved (a drag selects text). The click is the window's: whatever it
                    // landed in — an insert's terminal, a field — claims it on the app.
                    .capture_any_mouse_down(self.app_listener(|app, ev: &MouseDownEvent, _, _| {
                        app.click_claimed = false;
                        app.click_at = Some(ev.position);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        self.app_listener(move |app, ev: &MouseUpEvent, window, cx| {
                            let still = app.click_at.take().is_some_and(|p| {
                                (p.x - ev.position.x).abs() < px(CLICK_SLOP)
                                    && (p.y - ev.position.y).abs() < px(CLICK_SLOP)
                            });
                            if still && !app.click_claimed {
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
            })
            .into_any_element()
    }

    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.thread.tv.to_end();
        cx.notify();
    }

    /// A click inside the thread that is the thread's own (a fold, a button): it keeps the
    /// focus where it is.
    pub(crate) fn claim_click(&self, cx: &mut App) {
        if let Some(app) = self.app.upgrade() {
            app.update(cx, |app, _| app.click_claimed = true);
        }
    }

    /// What the live rows had becomes their entries': the answer being written passes its parsed
    /// texts to the answer that replaces it, the live thinking its fold to its piece of work.
    /// Made anew, either would change on screen at the run's end.
    fn carry_over(&mut self, rows: &[Row]) {
        let t = &mut self.thread;
        for (k, old) in t.tv.rows.iter().enumerate() {
            match (old, rows.get(k)) {
                (Row::Writing { .. }, Some(Row::Answer { key, .. })) => {
                    t.texts.rename("answer-writing-", &format!("answer-{key}-"));
                    t.texts.rename("code-writing-", &format!("code-{key}-"));
                }
                (Row::Work { key: was, .. }, Some(Row::Work { key, .. }))
                    if was != key && t.open_work.remove(was) =>
                {
                    t.open_work.insert(key.clone());
                }
                _ => {}
            }
        }
        // The live work opens on its own: thinking, steps and the agent's words show in the
        // list they belong to, and the stream's narration enters where its note will sit —
        // nothing vanishes from a seat of its own. The run's end folds what the view opened;
        // a person who clicked the line keeps it their way, open or shut.
        let live_key = |rs: &[Row]| {
            rs.iter().find_map(|r| match r {
                Row::Work {
                    key, live: true, ..
                } => Some(key.clone()),
                _ => None,
            })
        };
        let was = live_key(&t.tv.rows);
        let now = live_key(rows);
        match (was.as_deref(), now.as_deref(), t.auto_work.as_deref()) {
            (Some(o), Some(k), Some(a)) if a == o && o != k => {
                // the run moved to another line of work: the auto-open follows it
                t.open_work.remove(o);
                t.open_work.insert(k.to_string());
                t.auto_work = Some(k.to_string());
            }
            (None, Some(k), _) if t.auto_work.is_none() && !t.open_work.contains(k) => {
                t.open_work.insert(k.to_string());
                t.auto_work = Some(k.to_string());
            }
            (Some(_), None, Some(a)) => {
                t.open_work.remove(a);
                t.auto_work = None;
            }
            (None, None, Some(_)) => t.auto_work = None,
            _ => {}
        }
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
        h_flex()
            .w_full()
            .justify_center()
            .child(div().w_full().max_w(px(bench_ui::THREAD_MAX_W)).child(item))
            .into_any_element()
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
    fn render_head(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
                        .pl(px(bench_ui::CHAT_TEXT))
                        .pr(px(bench_ui::CHAT_RIGHT))
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
                        .pl(px(bench_ui::CHAT_TEXT))
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
                        .pl(px(bench_ui::CHAT_TEXT))
                        .pr(px(bench_ui::CHAT_RIGHT))
                        .py(px(bench_ui::TURN_Y))
                        .child(
                            div()
                                .w(px(bench_ui::PATH_FIELD_W))
                                .child(gpui_kit::component::input::Input::new(&field).small()),
                        )
                        .into_any_element(),
                );
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

    /// After the last row: the composer. Everything the agent produced stands in the journal —
    /// its plans, questions and background processes inside the work that wrote them.
    fn render_tail(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let composer = self.render_composer(cx);
        v_flex().w_full().child(composer).into_any_element()
    }
}
