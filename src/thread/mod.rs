//! The open thread on screen: `gpui::list` over the rows of `crate::thread` (the journal).
//!
//! The list holds: the head (a project's environment, a form, "Loading…"), one item per `Row`,
//! the tail (approval, question, plan, processes, the composer), and the room past the end that
//! lets the last row reach the top, as in an editor. Only what is on screen is laid out.
//!
//! Files: `list.rs` (the list's state and scroll), `message.rs` (the person's and the agent's
//! rows), `work.rs` (the folded steps), `queued.rs` (a waiting message), `rail.rs` (the ticks on
//! the right), `extras.rs` (approval, sources), `processes.rs`, `changed_files.rs`, `asks.rs` and
//! `answer.rs` (the agent's question and plan), `composer.rs` (the input) with `slash.rs` (its
//! pop-up), `prompts.rs`, `attachments.rs` and `drafts.rs`; `send.rs` (sending, the reasoning
//! level), `open.rs` (opening, creating and deleting threads).

mod answer;
mod asks;
pub(crate) mod attachments;
mod changed_files;
mod composer;
pub(crate) mod diff;
pub(crate) mod drafts;
mod extras;
mod list;
mod message;
mod queued;
mod rail;
mod open;
mod processes;
mod prompts;
pub(crate) mod send;
mod slash;
mod work;

pub use list::View;

use std::time::Duration;

use ::thread::Row;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The rows of the open thread into the list, if the thread changed since the last frame.
    fn sync_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tv.reveal_after_send();
        let version = self.thread.version();
        let owner = self.session.clone();
        if version != self.tv.version || owner != self.tv.owner {
            self.tv.version = version;
            let rows = self.thread.rows();
            self.carry_over(&rows);
            let follow = std::mem::take(&mut self.follow);
            let changed = self.tv.sync(owner.as_deref(), rows, follow);
            self.parse_ahead(changed, cx);
            // The tail's new height is known only after this frame's layout: look again then.
            window.request_animation_frame();
        }
        // The list keeps an item's first measured height: the room is measured again when it
        // changes (the composer shrank after sending, the window was resized).
        let room = self.tv.room_past_end(self.viewport_h);
        if std::mem::take(&mut self.tv.jump) {
            // `to_end`: this frame lays out without the room.
            window.request_animation_frame();
        } else if (room - self.tv.room).abs() > px(0.5) {
            self.tv.room = room;
            let end = self.tv.tail() + 1;
            self.tv.list.remeasure_items(end..end + 1);
            window.request_animation_frame();
        }
        // A live run's clock moves once a second.
        if self.thread.running() && self.tv.ticker.is_none() {
            self.tv.ticker = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let Ok(running) = this.update(cx, |this, cx| {
                        cx.notify();
                        this.thread.running()
                    }) else {
                        break;
                    };
                    if !running {
                        let _ = this.update(cx, |this, _| this.tv.ticker = None);
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
        if !self.demo
            && self.local_conn().is_some()
            && self.prompts_for.as_deref() != Some(self.thread_project.as_str())
        {
            self.prompts_for = Some(self.thread_project.clone());
            self.load_prompts(window, cx);
        }
        if self.demo && self.composer.read(cx).value().is_empty() {
            if let Some(text) = std::env::var("BENCH_DEMO_DRAFT")
                .ok()
                .filter(|_| !self.demo_draft_done)
            {
                self.demo_draft_done = true;
                self.composer
                    .update(cx, |s, cx| s.set_value(text, window, cx));
            }
        }
        if self.demo
            && !self.subject_opened
            && std::env::var("BENCH_DEMO").as_deref() == Ok("new-project")
        {
            self.subject_opened = true;
            self.new_project_form(window, cx);
        }
        self.sync_rows(window, cx);
        // `BENCH_DEMO_RENAME=<thread id>`: that thread being renamed (for screenshots).
        if self.demo {
            static RENAME: std::sync::Once = std::sync::Once::new();
            if let Ok(id) = std::env::var("BENCH_DEMO_RENAME") {
                RENAME.call_once(|| {
                    self.start_rename(crate::sidebar::items::Item::Thread(id), window, cx)
                });
            }
        }
        // `BENCH_DEMO_SCROLL=end`: the thread scrolled as far down as it goes (for screenshots).
        if self.demo
            && !self.demo_scrolled
            && std::env::var("BENCH_DEMO_SCROLL").as_deref() == Ok("end")
        {
            self.demo_scrolled = true;
            self.tv.list.scroll_to_end();
        }
        // `BENCH_DEMO_SCROLL=<item>:<px>`: the thread scrolled to there (a reader in the middle).
        if self.demo && !self.demo_scrolled {
            if let Some((i, o)) = std::env::var("BENCH_DEMO_SCROLL").ok().and_then(|v| {
                let (i, o) = v.split_once(':')?;
                Some((i.parse::<usize>().ok()?, o.parse::<f32>().ok()?))
            }) {
                self.demo_scrolled = true;
                self.tv.list.scroll_to(ListOffset {
                    item_ix: i,
                    offset_in_item: px(o),
                });
            }
        }
        // `BENCH_TRACE_SCROLL=1`: where the thread's content stands each frame (a jerk shows).
        if std::env::var("BENCH_TRACE_SCROLL").is_ok() {
            let top = self.tv.list.logical_scroll_top();
            let first = self.tv.list.bounds_for_item(top.item_ix).map(|b| b.top());
            let heights: Vec<i32> = (1..=self.tv.tail())
                .map(|i| {
                    self.tv
                        .list
                        .bounds_for_item(i)
                        .map_or(-1, |b| f32::from(b.size.height) as i32)
                })
                .collect();
            eprintln!(
                "scroll: item {} offset {:?} first-top {:?} rows {} room {:?} composer-bottom {:?} viewport-bottom {:?} heights {:?}",
                top.item_ix,
                top.offset_in_item,
                first,
                self.tv.rows.len(),
                self.tv.room,
                self.tv
                    .list
                    .bounds_for_item(self.tv.tail())
                    .map(|b| b.bottom()),
                self.tv.list.viewport_bounds().bottom(),
                heights
            );
        }
        let theme = cx.theme().clone();
        let tail = self.tv.tail();
        // Unknown for the frame after rows were remeasured: the button stays as it was.
        if let Some(b) = self.tv.list.item_is_below_viewport(tail) {
            self.tv.below = b;
        }
        let below = self.tv.below;
        let rail = self.render_rail(cx);
        let list = list(
            self.tv.list.clone(),
            cx.processor(|this, ix: usize, window, cx| this.render_item(ix, window, cx)),
        )
        .flex_1()
        .min_h_0();
        div()
            .relative()
            .size_full()
            .child(
                v_flex()
                    .id("chat")
                    .size_full()
                    // A click anywhere in the thread means "I'm about to type": the composer takes
                    // the focus, unless the click went into something that types itself, or the
                    // mouse moved (a drag selects text).
                    .capture_any_mouse_down(cx.listener(|this, ev: &MouseDownEvent, _, _| {
                        this.click_claimed = false;
                        this.click_at = Some(ev.position);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, ev: &MouseUpEvent, window, cx| {
                            let still = this.click_at.take().is_some_and(|p| {
                                (p.x - ev.position.x).abs() < px(4.)
                                    && (p.y - ev.position.y).abs() < px(4.)
                            });
                            if still && !this.click_claimed {
                                this.composer.update(cx, |s, cx| s.focus(window, cx));
                            }
                        }),
                    )
                    .child(list),
            )
            .children(rail)
            .when(below, |d| {
                d.child(
                    h_flex()
                        .id("jump")
                        .absolute()
                        .bottom_3()
                        .right_4()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .text_base()
                        .bg(theme.background)
                        .border_1()
                        .rounded_md()
                        .border_color(theme.border)
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .hover(|s| s.text_color(theme.foreground))
                        .child(Ico::ChevronDown.icon().xsmall())
                        .child("Jump to latest")
                        .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx))),
                )
            })
            .into_any_element()
    }

    /// The thread as the right dock's panel, beside a document: its title and close over the same
    /// thread the center shows in the threads view; a tab dropped on it sends its file.
    pub(crate) fn render_thread_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let title: SharedString = self
            .session
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
            .map(|s| s.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "New thread".into())
            .into();
        let head = bench_ui::ide_head(
            title,
            None,
            vec![
                bench_ui::icon_button("thread-new", Ico::Plus, "New thread  ⌘N", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.new_session(cx);
                        this.composer.update(cx, |s, cx| s.focus(window, cx));
                    }))
                    .into_any_element(),
                bench_ui::icon_button("thread-close", Ico::X, "Close  ⌘J", false, cx)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.toggle_thread_beside(window, cx)),
                    )
                    .into_any_element(),
            ],
            cx,
        )
        .h(bench_ui::bar_h(cx))
        .bg(theme.title_bar);
        v_flex()
            .size_full()
            .drag_over::<crate::explorer::tabs::DraggedTab>(|s, _, _, cx| {
                s.bg(cx.theme().list_active)
            })
            .on_drop(
                cx.listener(|this, d: &crate::explorer::tabs::DraggedTab, window, cx| {
                    if let crate::explorer::DocKey::File(f) = &d.key {
                        this.send_file_to_thread(f, window, cx)
                    }
                }),
            )
            .child(head)
            .child(div().flex_1().min_h_0().child(self.render_chat(window, cx)))
            .into_any_element()
    }

    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.tv.to_end();
        cx.notify();
    }

    /// What the live rows had becomes their entries': the answer being written passes its parsed
    /// texts to the answer that replaces it, the live thinking its fold to its piece of work.
    /// Made anew, either would change on screen at the run's end.
    fn carry_over(&mut self, rows: &[Row]) {
        for (k, old) in self.tv.rows.iter().enumerate() {
            match (old, rows.get(k)) {
                (Row::Writing { .. }, Some(Row::Answer { key, .. })) => {
                    self.texts
                        .rename("answer-writing-", &format!("answer-{key}-"));
                    self.texts.rename("code-writing-", &format!("code-{key}-"));
                }
                (Row::Work { key: was, .. }, Some(Row::Work { key, .. }))
                    if was != key && self.open_work.remove(was) =>
                {
                    self.open_work.insert(key.clone());
                }
                _ => {}
            }
        }
    }

    /// Start parsing the texts of the rows from `from` on, before they are on screen: a row
    /// scrolled into view then has its whole height in its first frame.
    fn parse_ahead(&mut self, from: usize, cx: &mut Context<Self>) {
        for row in self.tv.rows[from.min(self.tv.rows.len())..].to_vec() {
            match row {
                Row::User { key, text, .. } => {
                    let (said, _) = crate::media::attach::split(&text);
                    if !said.is_empty() {
                        self.texts
                            .state(&key, &crate::media::text::escape_plain(&said), cx);
                    }
                }
                Row::Answer { key, text, .. } => {
                    for (k, seg) in crate::media::text::answer_segments(&text)
                        .into_iter()
                        .enumerate()
                    {
                        if let crate::media::Segment::Text(t) = seg {
                            self.texts.state(&format!("answer-{key}-{k}"), &t, cx);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// One item of the list.
    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let n = self.tv.rows.len();
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
            div().h(self.tv.room).into_any_element()
        }
    }

    fn render_row(&mut self, k: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let row = self.tv.rows[k].clone();
        // An answer right after the work that led to it is that work's continuation: no avatar.
        let after_work = k > 0 && matches!(self.tv.rows[k - 1], Row::Work { .. });
        match row {
            Row::User { key, text, .. } => self.render_user(&key, &text, cx),
            Row::Work {
                key,
                steps,
                from,
                to,
                live,
            } => self.render_work(&key, &steps, from, to, live, cx),
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
            } => self.render_queued(&key, &sub, &mode, &text, window, cx),
        }
    }

    /// Before the first row: a form opened at the top, or why the thread is empty.
    fn render_head(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<AnyElement> = vec![];
        let empty = self.tv.rows.is_empty();
        if empty {
            if let (Some(id), Some(why)) = (self.session.clone(), self.history_error.clone()) {
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
                            bench_ui::form_action("history-retry", "Retry", false, cx).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.open_session(id.clone(), cx)
                                }),
                            ),
                        )
                        .into_any_element(),
                );
            } else if self.session.is_some() && self.runtime.is_none() {
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
            } else if self.path_entry {
                rows.push(
                    h_flex()
                        .w_full()
                        .pl(px(bench_ui::CHAT_TEXT))
                        .pr(px(bench_ui::CHAT_RIGHT))
                        .py(px(bench_ui::TURN_Y))
                        .child(div().w(px(360.)).child(
                            gpui_kit::component::input::Input::new(&self.project_path).small(),
                        ))
                        .into_any_element(),
                );
            }
        }
        v_flex().w_full().children(rows).into_any_element()
    }

    /// After the last row: what waits for the person, the live status blocks, the composer.
    fn render_tail(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let approval = self.render_approval(cx);
        let clarify = self.render_clarify(cx);
        let plan = self.render_plan(cx);
        let procs = self.render_processes(cx);
        let composer = self.render_composer(cx);
        v_flex()
            .w_full()
            .children(plan)
            .children(procs)
            .children(approval)
            .children(clarify)
            .child(composer)
            .into_any_element()
    }
}
