//! The open thread on screen: `gpui::list` over the rows of `crate::thread` (the journal).
//!
//! The list holds: the head (a project's environment, a form, "Loading…"), one item per `Row`,
//! the tail (approval, question, plan, processes, the composer), and the room past the end that
//! lets the last row reach the top, as in an editor. Only what is on screen is laid out.
//!
//! Files: `message.rs` (the person's and the agent's rows), `work.rs` (the folded steps),
//! `queued.rs` (a waiting message), `rail.rs` (the ticks on the right), `extras.rs` (approval,
//! processes, changed files, sources).

mod extras;
mod message;
mod queued;
mod rail;
mod work;

use std::time::Duration;

use ::thread::Row;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::icons::Ico;

/// What the list shows and how it is scrolled. Owned by `BenchApp`.
pub struct View {
    pub list: ListState,
    /// The thread's rows, as last given to the list.
    pub rows: Vec<Row>,
    /// The thread's version they were made at.
    version: u64,
    /// Which thread they are of: another one starts at the top.
    owner: Option<String>,
    /// While a run is live, a timer repaints once a second (its clock).
    ticker: Option<Task<()>>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            list: ListState::new(3, ListAlignment::Top, px(800.)),
            rows: vec![],
            version: u64::MAX,
            owner: None,
            ticker: None,
        }
    }
}

impl View {
    /// The list's item of row `k`; the head is item 0.
    pub fn item_of(k: usize) -> usize {
        k + 1
    }

    /// The tail's item (the composer's).
    pub fn tail(&self) -> usize {
        self.rows.len() + 1
    }

    /// Give the list `rows`. Unchanged rows keep their measured heights; a thread of another
    /// owner starts at the top. `follow`: keep the composer in view if it was.
    fn sync(&mut self, owner: Option<&str>, rows: Vec<Row>, follow: bool) {
        if owner != self.owner.as_deref() {
            self.owner = owner.map(str::to_string);
            self.rows = rows;
            self.list.reset(self.rows.len() + 3);
            self.list.scroll_to(ListOffset::default());
            return;
        }
        let same = self
            .rows
            .iter()
            .zip(&rows)
            .take_while(|(a, b)| a == b)
            .count();
        if same == self.rows.len() && same == rows.len() {
            return;
        }
        let tail_seen = self.list.item_is_below_viewport(self.tail()) == Some(false)
            && self.list.item_is_above_viewport(self.tail()) == Some(false);
        let old = self.rows.len();
        self.rows = rows;
        self.list.splice(
            Self::item_of(same)..Self::item_of(old),
            self.rows.len() - same,
        );
        if follow || tail_seen {
            self.list.scroll_to_reveal_item(self.tail());
        }
    }
}

impl BenchApp {
    /// The rows of the open thread into the list, if the thread changed since the last frame.
    fn sync_rows(&mut self, cx: &mut Context<Self>) {
        let version = self.thread.version();
        let owner = self.session.clone();
        if version != self.tv.version || owner != self.tv.owner {
            self.tv.version = version;
            let rows = self.thread.rows();
            let follow = std::mem::take(&mut self.follow);
            self.tv.sync(owner.as_deref(), rows, follow);
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
            && self.conn.is_some()
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
        self.sync_rows(cx);
        let theme = cx.theme().clone();
        let tail = self.tv.tail();
        let below = self.tv.list.item_is_below_viewport(tail) == Some(true);
        let rail = self.render_rail(cx);
        let list = list(
            self.tv.list.clone(),
            cx.processor(|this, ix: usize, window, cx| this.render_item(ix, window, cx)),
        )
        .size_full();
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

    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.tv.list.scroll_to_reveal_item(self.tv.tail());
        cx.notify();
    }

    /// One item of the list.
    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let n = self.tv.rows.len();
        let el = if ix == 0 {
            self.render_head(window, cx)
        } else if ix <= n {
            self.render_row(ix - 1, window, cx)
        } else if ix == n + 1 {
            self.render_tail(window, cx)
        } else {
            // Room past the end: the last row can be scrolled up to the top.
            div().h(self.viewport_h * 0.7).into_any_element()
        };
        // The first row keeps as much air above it as the thread has at its sides.
        if ix == 0 {
            div()
                .pt(px(crate::ui::CHAT_LEFT - crate::ui::TURN_Y))
                .child(el)
                .into_any_element()
        } else {
            el
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
            Row::Writing { thought, text } => self.render_writing(&thought, &text, after_work, cx),
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

    /// Before the first row: a project's environment, a form opened at the top, or why the
    /// thread is empty.
    fn render_head(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let subject = self.current_subject();
        let mut rows: Vec<AnyElement> = vec![];
        if let Some(id) = subject.strip_prefix("env:") {
            if self.project_env.as_ref().is_none_or(|e| e.id != id) {
                self.load_env(id, window, cx);
            }
            let under = self.env_card_section();
            for (title, part) in self.render_env_sections(window, cx) {
                rows.push(self.agent_row(part, cx));
                if under == Some(title) && !title.is_empty() {
                    if let Some(card) = self.render_env_card(cx) {
                        rows.push(self.agent_row(card, cx));
                    }
                }
            }
        }
        let empty = self.tv.rows.is_empty() && rows.is_empty();
        if empty {
            if let (Some(id), Some(why)) = (self.session.clone(), self.history_error.clone()) {
                rows.push(
                    h_flex()
                        .w_full()
                        .pl(px(crate::ui::CHAT_TEXT))
                        .pr(px(crate::ui::CHAT_RIGHT))
                        .py(px(crate::ui::TURN_Y))
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
                            crate::ui::form_action("history-retry", "Retry", false, cx).on_click(
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
                        .pl(px(crate::ui::CHAT_TEXT))
                        .py(px(crate::ui::TURN_Y))
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
                        .pl(px(crate::ui::CHAT_TEXT))
                        .pr(px(crate::ui::CHAT_RIGHT))
                        .py(px(crate::ui::TURN_Y))
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
        let form = self.render_tail_form(cx);
        let composer = self.render_composer(cx);
        v_flex()
            .w_full()
            .children(plan)
            .children(procs)
            .children(approval)
            .children(clarify)
            .children(form)
            .child(composer)
            .into_any_element()
    }
}
