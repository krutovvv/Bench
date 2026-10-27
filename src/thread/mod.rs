//! A thread on screen: its `ThreadView` (`view.rs`, an entity of its own) draws `gpui::list` over
//! the rows of `crate::thread` (the journal).
//!
//! The list holds: the head (a project's environment, a form, "Loading…"), one item per `Row`,
//! the tail (approval, question, processes, the composer), and the room past the end that
//! lets the last row reach the top, as in an editor. Only what is on screen is laid out.
//!
//! Files: `view.rs` (the view: its state, composer, events), `active.rs` (the thread's state, one
//! struct, and `ThreadRef`), `list.rs` (the list's state and scroll), `message.rs` (the person's
//! and the agent's rows), `work.rs` (the folded steps), `queued.rs` (a waiting message), `rail.rs` (the ticks on
//! the right), `extras.rs` (approval, sources), `processes.rs`, `changed_files.rs`, `asks.rs` and
//! `answer.rs` (the agent's question and plan), `composer.rs` (the input) with `slash.rs` (its
//! pop-up), `prompts.rs`, `attachments.rs` and `drafts.rs`; `send.rs` (sending, the reasoning
//! level), `open.rs` (opening, creating and deleting threads: the app's), `panel.rs` (the right
//! dock's thread panel: its head over a view), `beside.rs` (a second thread beside the open one).

mod active;
mod answer;
mod asks;
pub(crate) mod attachments;
mod beside;
mod changed_files;
mod composer;
pub(crate) mod diff;
pub(crate) mod drafts;
mod extras;
mod list;
mod message;
mod open;
mod panel;
mod processes;
mod prompts;
mod queued;
mod rail;
pub(crate) mod send;
mod slash;
mod view;
pub(crate) mod work;

pub use active::{ActiveThread, ThreadRef};
pub use list::View;
pub use view::ThreadView;

use std::time::Duration;

use ::thread::Row;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bench_ui::icons::Ico;

impl ThreadView {
    /// The rows of the thread into the list, if the thread changed since the last frame.
    fn sync_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.thread.tv.reveal_after_send();
        let version = self.thread.journal.version();
        let owner = self.thread.session.clone();
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
        } else if (room - self.thread.tv.room).abs() > px(0.5) {
            self.thread.tv.room = room;
            let end = self.thread.tv.tail() + 1;
            self.thread.tv.list.remeasure_items(end..end + 1);
            window.request_animation_frame();
        }
        // A live run's clock moves once a second.
        if self.thread.journal.running() && self.thread.tv.ticker.is_none() {
            self.thread.tv.ticker = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let Ok(running) = this.update(cx, |this, cx| {
                        cx.notify();
                        this.thread.journal.running()
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

    /// The demo's hooks on the thread (`BENCH_DEMO_DRAFT`, `BENCH_DEMO_SCROLL`, for screenshots).
    fn demo_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).value().is_empty() {
            if let Some(text) = std::env::var("BENCH_DEMO_DRAFT")
                .ok()
                .filter(|_| !self.demo_draft_done)
            {
                self.demo_draft_done = true;
                self.composer
                    .update(cx, |s, cx| s.set_value(text, window, cx));
            }
        }
        // `BENCH_DEMO_SCROLL=end`: the thread keeps to its end (a tail shot: what grows after the
        // first frame — the queue, an answer — is still in the picture).
        if std::env::var("BENCH_DEMO_SCROLL").as_deref() == Ok("end") {
            self.demo_scrolled = true;
            self.thread.tv.to_end();
        }
        // `BENCH_DEMO_SCROLL=<item>:<px>`: the thread scrolled to there (a reader in the middle).
        if !self.demo_scrolled {
            if let Some((i, o)) = std::env::var("BENCH_DEMO_SCROLL").ok().and_then(|v| {
                let (i, o) = v.split_once(':')?;
                Some((i.parse::<usize>().ok()?, o.parse::<f32>().ok()?))
            }) {
                self.demo_scrolled = true;
                self.thread.tv.list.scroll_to(ListOffset {
                    item_ix: i,
                    offset_in_item: px(o),
                });
            }
        }
        // `BENCH_DEMO_QUEUE=1`: two messages waiting after the last row, one the agent will take
        // up next, one a follow-up, and the plan the agent is working through — which stands
        // above them, with the work it plans.
        if std::env::var("BENCH_DEMO_QUEUE").is_ok() && self.thread.journal.queue().is_empty() {
            self.thread.journal.apply(protocol::ThreadEvent::Queue(vec![
                protocol::QueuedItem {
                    key: "q-1".into(),
                    sub: "q-1".into(),
                    mode: "steer".into(),
                    text: "And the cover: the one from the spring shoot.".into(),
                },
                protocol::QueuedItem {
                    key: "q-2".into(),
                    sub: "q-2".into(),
                    mode: "follow-up".into(),
                    text: "Skip the pricing table for now, it goes in the next release.".into(),
                },
            ]));
            // A queue like this means a live run: the plan of that run shows, and it shows in
            // the work it belongs to — a journal item, as any plan is. Before the plan: what
            // the agent said aloud and the call it went on to make — the header line keeps the
            // words while the command runs.
            self.thread.journal.apply(protocol::ThreadEvent::Run(true));
            self.thread.journal.apply(protocol::ThreadEvent::Append(vec![
                protocol::ThreadItem::Tool {
                    key: "demo-live-1".into(),
                    seq: u64::MAX - 5,
                    call_id: "demo-live-c".into(),
                    name: "bash".into(),
                    args: serde_json::json!({ "command": "python3 bills/match.py --month oct" }),
                    preview: "python3 bills/match.py --month oct".into(),
                    at: None,
                },
                protocol::ThreadItem::Result {
                    key: "demo-live-2".into(),
                    seq: u64::MAX - 4,
                    call_id: "demo-live-c".into(),
                    name: "bash".into(),
                    output: "3 unmatched".into(),
                    failed: false,
                    diff: None,
                    at: None,
                },
                protocol::ThreadItem::Text {
                    key: "demo-live-3".into(),
                    // The words of a message that went on to call: narration, a note on the
                    // work line (`rows_of`), exactly as the agent marks them.
                    seq: u64::MAX - 2,
                    text: "Reading the billing export, then matching it against the invoices."
                        .into(),
                    at: None,
                    error: None,
                    narration: Some(true),
                },
                protocol::ThreadItem::Tool {
                    key: "demo-live-4".into(),
                    seq: u64::MAX - 2,
                    call_id: "demo-live-c2".into(),
                    name: "bash".into(),
                    args: serde_json::json!({ "command": "grep -c INV-2026 invoices/*.csv" }),
                    preview: "grep -c INV-2026 invoices/*.csv".into(),
                    at: None,
                },
                protocol::ThreadItem::Plan {
                    key: "demo-plan".into(),
                    seq: u64::MAX - 1,
                    id: "demo-plan".into(),
                    todos: ["done", "doing", "next"]
                        .into_iter()
                        .zip(["completed", "in_progress", "pending"])
                        .enumerate()
                        .map(|(k, (c, s))| protocol::PlanStep {
                            id: format!("{k}"),
                            content: format!("{c}: the step the agent wrote for itself"),
                            status: s.into(),
                            parent: None,
                        })
                        .collect(),
                },
            ]));
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
        let rail = self.render_rail(cx);
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
                                (p.x - ev.position.x).abs() < px(4.)
                                    && (p.y - ev.position.y).abs() < px(4.)
                            });
                            if still && !app.click_claimed {
                                composer.update(cx, |s, cx| s.focus(window, cx));
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
    fn render_item(
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
            } => self.render_work(
                &key,
                &steps,
                from,
                to,
                live,
                &holds,
                plan.as_ref(),
                &asks,
                live && !self.thread.processes.is_empty(),
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
            } => self.render_queued(&key, &sub, &mode, &text, window, cx),
        }
    }

    /// Before the first row: why the thread is empty, or the new chat's folder being typed.
    fn render_head(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<AnyElement> = vec![];
        let empty = self.thread.tv.rows.is_empty();
        let path_entry = self
            .app(cx)
            .filter(|a| a.path_entry)
            .map(|a| a.project_path.clone());
        if empty {
            if let (Some(_), Some(why)) = (
                self.thread.session.clone(),
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
            } else if self.thread.session.is_some() && self.thread.runtime.is_none() {
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
                                .w(px(360.))
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
        let Some(session) = self.thread.session.clone() else {
            return;
        };
        self.thread.history_error = None;
        if let Some(profile) = self.profile(cx) {
            self.cmd(crate::client::rpc::Cmd::Open { profile, session }, cx);
        }
        cx.notify();
    }

    /// After the last row: the composer. Everything the agent produced stands in the journal —
    /// its plans, questions and background processes inside the work that wrote them.
    fn render_tail(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let composer = self.render_composer(cx);
        v_flex().w_full().child(composer).into_any_element()
    }
}
