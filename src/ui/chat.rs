//! The conversation as one scrolling column: per turn the user message, folded steps
//! (each unfoldable), the answer, sources and hover actions; under the last answer, the
//! files this session changed.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::media::plain_text;
use super::{clip, clock, plural};
use crate::app::{BenchApp, tool_label};
use crate::history::host_of;
use crate::icons::Ico;
use crate::model::{Step, Turn};

/// The air the deepest scroll leaves under the top edge of the window. The row's own padding
/// keeps it whole; what the room was missing was this fixed breath — measuring the last row's
/// height turned out to be the wrong lever, since it made the depth change with every row.
const AIR: f32 = 16.;

fn past_end(viewport: f32, composer: f32) -> f32 {
    (viewport - super::STATUS_H - composer - LINE - AIR).max(LINE)
}

/// After how many turns a row of the agent's own work sits: after the last turn that had begun
/// when the work did. The plan and the background processes both hang by this, so neither
/// drifts to the newest message — what began earlier stays where it began. Without a moment of
/// its own (a thread planned before this was kept) it stays at the end, as it was then.
fn work_pos(turns: &[Turn], at: Option<f64>) -> usize {
    let Some(at) = at else { return turns.len() };
    turns
        .iter()
        .position(|t| t.started.is_some_and(|s| s > at))
        .unwrap_or(turns.len())
}

/// One frame of following the tail. `off` is how much was scrolled from the top (so going UP
/// the thread makes it smaller), `max` is the bottom with the room under the composer trimmed
/// away, `pinned` is where we last pinned the view. Returns (follow, pinned, offset to jump to).
/// Growing content never lets go — it leaves `off` where it was — while a wheel, a drag or a
/// click moves `off` and is the person's own. Written with the sign reversed, every wheel was
/// undone by the next frame: the thread jerked and would not go up.
pub fn tail_step(off: f32, max: f32, follow: bool, pinned: f32) -> (bool, f32, Option<f32>) {
    const SLACK: f32 = 4.;
    let at_bottom = off >= max - SLACK;
    let mut follow = follow && off >= pinned - SLACK;
    if at_bottom {
        follow = true;
    }
    if follow && !at_bottom {
        (true, max, Some(max))
    } else {
        (follow, off, None)
    }
}

#[cfg(test)]
mod tail_tests {
    use super::tail_step;

    #[test]
    fn an_answer_that_grows_keeps_the_view_at_the_tail() {
        // Streaming adds height: the offset stays where we pinned it, so it must jump, not let go.
        let (follow, pinned, jump) = tail_step(1000., 1200., true, 1000.);
        assert!(follow);
        assert_eq!(jump, Some(1200.));
        assert_eq!(pinned, 1200.);
    }

    #[test]
    fn a_move_up_is_the_persons_own() {
        // The wheel went up the thread: smaller offset, and nothing may pull it back.
        let (follow, _, jump) = tail_step(900., 1200., true, 1200.);
        assert!(!follow);
        assert_eq!(jump, None);
    }

    #[test]
    fn a_hair_of_coasting_is_not_a_move() {
        let (follow, _, jump) = tail_step(1197., 1200., true, 1200.);
        assert!(follow);
        assert_eq!(jump, None);
    }

    #[test]
    fn coming_back_to_the_tail_follows_again() {
        let (follow, _, jump) = tail_step(1200., 1200., false, 900.);
        assert!(follow);
        assert_eq!(jump, None);
    }

    #[test]
    fn content_that_shrinks_clamps_and_does_not_release() {
        // The pinned mark is above what is now scrollable: the clamp brought the view here.
        let (follow, _, jump) = tail_step(1000., 1000., true, 1200.);
        assert!(follow);
        assert_eq!(jump, None);
    }

    #[test]
    fn a_short_thread_never_jumps() {
        let (follow, pinned, jump) = tail_step(0., 0., true, 0.);
        assert!(follow);
        assert_eq!(jump, None);
        assert_eq!(pinned, 0.);
    }
}

#[cfg(test)]
mod past_end_tests {
    use super::{LINE, past_end};
    #[test]
    fn the_last_row_reaches_the_top_and_never_past_it() {
        // A 900-px window, a composer of one line (48): the room is what the window has left.
        let big = past_end(900., 48.);
        assert!(big > 700. && big < 900., "{big}");
        // A taller composer takes room away; a small window keeps at least a line of it.
        assert!(past_end(900., 300.) < big);
        assert_eq!(
            past_end(100., 400.),
            LINE,
            "no room: still a line to scroll"
        );
        // Pulled to the top, the last row stops with the fixed air under the edge, not flush on it.
        assert_eq!(900. - past_end(900., 48.) - 48. - super::super::STATUS_H, 40.);
    }
}

#[cfg(test)]
mod plan_tests {
    use super::work_pos;
    use crate::model::Turn;

    fn turn(started: Option<f64>) -> Turn {
        Turn {
            started,
            ..Default::default()
        }
    }

    #[test]
    fn the_plan_sits_where_it_was_made() {
        let turns = [turn(Some(100.)), turn(Some(200.)), turn(Some(300.))];
        assert_eq!(
            work_pos(&turns, Some(150.)),
            1,
            "made during the first turn"
        );
        assert_eq!(work_pos(&turns, Some(250.)), 2);
        assert_eq!(work_pos(&turns, Some(500.)), 3, "the newest work");
        assert_eq!(
            work_pos(&turns, Some(50.)),
            0,
            "older than the thread: at its top"
        );
        assert_eq!(
            work_pos(&turns, None),
            3,
            "no moment of its own: at the end"
        );
        assert_eq!(
            work_pos(&[turn(None)], Some(10.)),
            1,
            "an undated turn keeps it after"
        );
        assert_eq!(work_pos(&[], Some(10.)), 0, "no turns yet");
    }
}

/// One line of the thread's text, enough to keep a row whole at the edge.
const LINE: f32 = 24.;

/// The room the thread can scroll on past its last row, as a code editor past its last line:
/// enough to bring that row to the very top of the window, so the reading position is the
/// person's, not always the bottom. The composer rides at the end of the thread and the status
/// bar under the window, so both are counted out of the window's height; the estimate errs on
/// the small side — a row stopping a few pixels under the top edge is invisible, one cut by it
/// is not.
/// A terminal's most: under it, it fits its text.
const TERMINAL_MAX: f32 = 280.;

/// A terminal as tall as the lines it shows (the cursor's included), up to TERMINAL_MAX.
pub(super) fn terminal_height(session: &crate::term::Session) -> f32 {
    let line = match session.layout().line_h {
        h if h > 1. => h,
        _ => 20.,
    };
    let used = session.used_rows().min(1000).max(1) as f32;
    // The view's own padding (py_2).
    (used * line + 16.).min(TERMINAL_MAX)
}

impl BenchApp {
    /// New chat: optionally work in a git project, optionally in a branch of its own.
    pub(super) fn render_chat(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.viewport_h = window.viewport_size().height;
        // How far the thread may scroll past its last row, recomputed with the window and the
        // draft in the composer (both change the room).
        // Measured, and only estimated for the first frame, before the composer has been painted.
        let composer_h = if self.composer_h > 0. {
            self.composer_h
        } else {
            f32::from(self.viewport_h) * 0.1 + LINE
        };
        let past = past_end(f32::from(self.viewport_h), composer_h);
        // A thread with no turns yet still is a thread: "Loading history…" while it opens, then
        // the composer; a new chat is the composer alone.
        // A thread about something starts with it, live: a task list, a server's terminal, a tool.
        // @ in the composer offers the open thread's project's prompts.
        if !self.demo
            && self.conn.is_some()
            && self.prompts_for.as_deref() != Some(self.thread_project.as_str())
        {
            self.prompts_for = Some(self.thread_project.clone());
            self.load_prompts(window, cx);
        }
        // A project's environment is the thread's document, at its top; everything else the
        // person puts in is an insert (crate::ui::inserts), where it was put.
        let subject = self.current_subject();
        // The environment: each part its own message, the open row's card right under its part.
        let mut env_rows: Vec<AnyElement> = vec![];
        if let Some(id) = subject.strip_prefix("env:") {
            if self.project_env.as_ref().is_none_or(|e| e.id != id) {
                self.load_env(id, window, cx);
            }
            let under = self.env_card_section();
            for (title, part) in self.render_env_sections(window, cx) {
                env_rows.push(self.agent_row(part, cx));
                if under == Some(title) && !title.is_empty() {
                    if let Some(card) = self.render_env_card(cx) {
                        env_rows.push(self.agent_row(card, cx));
                    }
                }
            }
        }
        let block: Option<AnyElement> = None;
        // `BENCH_DEMO_DRAFT=text`: the composer filled once (for screenshots of a typed line).
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
        // `BENCH_DEMO=new-project`: the new project's page, once (for screenshots).
        if self.demo
            && !self.subject_opened
            && std::env::var("BENCH_DEMO").as_deref() == Ok("new-project")
        {
            self.subject_opened = true;
            self.new_project_form(window, cx);
        }
        // The block comes from the agent: its row, its avatar, like an answer.
        let theme = cx.theme().clone();
        // A form (a new project; a new server or file host) is a block of the thread too: the new
        // project's at the top of its empty thread, the others with the tools, under the
        // conversation.
        // A form opened from an insert shows under that insert (`render_inserts_at`).
        let under_insert = self
            .form_after
            .as_ref()
            .is_some_and(|id| self.thread_inserts().iter().any(|i| &i.id == id));
        let form: Option<(AnyElement, bool)> = if under_insert {
            None
        } else if self.files_remote.form {
            Some((self.render_host_form(cx), false))
        } else if self.ssh.form.is_some() {
            Some((self.render_server_form(cx), false))
        } else {
            None
        };
        let (block, block_on_top_form) = match form {
            Some((f, top)) => (Some(f), Some(top)),
            None => (block, None),
        };
        // A tool attached with / sits where the work goes on: under the conversation, over the
        // composer. A project's environment is the thread's document: at the top.
        let block_on_top = block_on_top_form.unwrap_or_else(|| subject.starts_with("env:"));
        let block_is_terminal = false;
        let subject_block = block.map(|content| {
            h_flex()
                .w_full()
                .pl(px(super::CHAT_LEFT))
                .pr(px(super::CHAT_RIGHT))
                .py(px(super::TURN_Y))
                .gap(px(super::CHAT_TEXT - super::CHAT_LEFT - super::AVATAR))
                .items_start()
                // A terminal is the person's (they opened it); the rest the agent's.
                .child(super::avatar(!block_is_terminal, &theme))
                // Typing in a block keeps the focus there (see the thread's click below).
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // Working in the block (opening a skill, typing) must not pull the
                        // thread to its end when the block grows.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, _| {
                                this.click_claimed = true;
                                this.follow = false;
                            }),
                        )
                        .child(content),
                )
                .into_any_element()
        });
        let mut subject_block = subject_block;
        let placeholder: Option<AnyElement> = if subject_block.is_some()
            || !env_rows.is_empty()
            || !self.thread_inserts().is_empty()
        {
            None
        } else if !self.turns.is_empty() {
            None
        } else if let (Some(id), Some(why)) = (self.session.clone(), self.history_error.clone()) {
            // The history did not come: why, and another try.
            Some(
                h_flex()
                    .w_full()
                    .pl(px(super::CHAT_TEXT))
                    .pr(px(super::CHAT_RIGHT))
                    .py(px(super::TURN_Y))
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
                        super::form_action("history-retry", "Retry", false, cx).on_click(
                            cx.listener(move |this, _, _, cx| this.open_session(id.clone(), cx)),
                        ),
                    )
                    .into_any_element(),
            )
        } else if self.session.is_some() && self.runtime.is_none() {
            // Opening: until its history comes (an empty thread has none to wait for).
            Some(
                h_flex()
                    .w_full()
                    .pl(px(super::CHAT_TEXT))
                    .py(px(super::TURN_Y))
                    .child(
                        ShimmerText::new("Loading history…")
                            .text_base()
                            .text_color(theme.muted_foreground),
                    )
                    .into_any_element(),
            )
        } else if self.path_entry {
            // A folder on a remote machine: typed (there is no picker for it).
            Some(
                h_flex()
                    .w_full()
                    .pl(px(super::CHAT_TEXT))
                    .pr(px(super::CHAT_RIGHT))
                    .py(px(super::TURN_Y))
                    .child(
                        div().w(px(360.)).child(
                            gpui_kit::component::input::Input::new(&self.project_path).small(),
                        ),
                    )
                    .into_any_element(),
            )
        } else {
            // A new chat is a clean sheet: only the composer. Its folder is the
            // group's "+" or the sidebar's "Open folder…".
            None
        };

        // A row measured above what is being read changed the height the thread is counted at:
        // move the view by the same amount, so the words under the eyes stay under the eyes. At
        // the tail this does not matter — the frame pins to the bottom anyway.
        if self.rows_shift.abs() > 0.5 {
            if !self.follow {
                let o = -self.chat_scroll.offset().y + px(self.rows_shift);
                self.chat_scroll.set_offset(point(px(0.), -o));
                self.pinned = o;
            }
            self.rows_shift = 0.;
        }
        // Follow the tail: stay pinned to the composer until the user scrolls up. "The bottom" is
        // the composer's, not the room the thread scrolls on past it (past_end), so a
        // short thread stays put and nothing calls for "Jump to latest".
        let max = (self.chat_scroll.max_offset().y - px(past)).max(px(0.));
        let off = -self.chat_scroll.offset().y;
        // Follow the tail: stay pinned to the composer until the person scrolls up (the rules are
        // `tail_step`, tested against streaming, a wheel, a return and a short thread).
        let (follow, pinned, jump) = tail_step(f32::from(off), f32::from(max), self.follow, f32::from(self.pinned));
        self.follow = follow;
        self.pinned = px(pinned);
        if let Some(m) = jump {
            self.chat_scroll.set_offset(point(px(0.), -px(m)));
        }
        let at_bottom = f32::from(off) >= f32::from(max) - 4.;
        // An opening thread stays invisible until its content is in and its height held for a
        // few frames (inserts placed, the offset pinned to the bottom), at most `settle` frames.
        if self.settle > 0 {
            if self.settle_ready && max == self.last_max {
                self.settle_stable += 1;
            } else {
                self.settle_stable = 0;
            }
            self.settle = if self.settle_stable >= 3 {
                0
            } else {
                self.settle - 1
            };
            window.request_animation_frame();
        }
        let hidden = self.settle > 0;
        self.last_max = max;

        // The turns, and after each the inserts the person put there.
        let n = self.turns.len();
        // The agent's plan, as its row under the work it describes — there, and not after the
        // newest message: a plan made earlier stays where it was made.
        let plan_after = work_pos(&self.turns, self.plan_at);
        let mut plan = self.render_plan(cx);
        // The agent's background processes: its own row too, where the work began, not nailed to
        // the newest answer.
        let procs_after = work_pos(&self.turns, self.proc_moment());
        let mut procs = self.render_processes(cx);

        // The thread as rows the model knows the heights of: the head (the environment's open
        // rows and the placeholder), one row per turn with its inserts and its work rows under
        // it, and the tail (the forms, the queue, the composer). Only the rows the viewport
        // needs, plus a band around it, are built and laid out; the rest of the thread stands in
        // for by two spacers carrying their heights. A frame therefore costs the screen, not the
        // thread, and a long thread stops being slower than a short one.
        let mut keys: Vec<crate::ui::rows::RowKey> =
            vec![crate::ui::rows::RowKey::Block("head")];
        keys.extend((0..n).map(crate::ui::rows::RowKey::Turn));
        keys.push(crate::ui::rows::RowKey::Block("tail"));
        self.rows.sync(&keys);
        // The room the thread scrolls in, as the band's size: the window less the chrome over and
        // under it. The band itself is twice this tall, so being off a little costs nothing.
        let vh = f32::from(window.bounds().size.height) - 140.;
        let band = self.rows.visible(f32::from(off), vh.max(240.));
        let view = cx.entity();
        let head: AnyElement = v_flex()
            .w_full()
            .children(env_rows)
            .when(block_on_top, |d| d.children(subject_block.take()))
            .children(placeholder)
            .into_any_element();
        let mut rows: Vec<AnyElement> = Vec::new();
        if band.start > 0 {
            rows.push(Self::spacer(self.rows.start(band.start)));
        }
        if band.contains(&0) {
            rows.push(self.measured_row(0, head, f32::from(off), view.clone(), cx));
        }
        for i in 0..n {
            if !band.contains(&(i + 1)) {
                continue;
            }
            // The turn's row carries what belongs under it: its inserts, and the plan and the
            // processes when their moment is here.
            let mut row = v_flex().w_full().child(self.render_turn(i, cx));
            row.extend(self.render_inserts_at(i + 1, i + 1 == n, window, cx));
            if plan_after == i + 1 {
                row.extend(plan.take());
            }
            if procs_after == i + 1 {
                row.extend(procs.take());
            }
            rows.push(self.measured_row(
                i + 1,
                row.into_any_element(),
                f32::from(off),
                view.clone(),
                cx,
            ));
        }
        if n == 0 && plan_after == 0 {
            rows.extend(plan.take());
        }
        if n == 0 && procs_after == 0 {
            rows.extend(procs.take());
        }
        // A row whose moment is newer than every turn, or a thread with turns and no plan: the
        // work rows go after the last one, which is where work_pos points in those cases.
        rows.extend(plan.take());
        rows.extend(procs.take());
        let approval = self.render_approval(cx);
        let clarify = self.render_clarify(cx);
        let rail = self.render_turn_rail(at_bottom, f32::from(off), cx);
        // The environment's open row, as the agent's answer under it.
        let queue = self.render_queue(cx);
        let composer = self.render_composer(cx);
        // The thread's last row: the forms, what waits in the queue, and the composer.
        let tail_idx = n + 1;
        let tail: AnyElement = v_flex()
            .w_full()
            .children(approval)
            .children(clarify)
            .children(subject_block)
            .children(queue)
            .child(composer)
            .into_any_element();
        if band.contains(&tail_idx) {
            rows.push(self.measured_row(tail_idx, tail, f32::from(off), view.clone(), cx));
        }
        if band.end < self.rows.lens() {
            rows.push(Self::spacer(self.rows.content() - self.rows.start(band.end)));
        }

        div()
            .relative()
            .size_full()
            .child(
                v_flex()
                    .id("chat")
                    .size_full()
                    .when(hidden, |d| d.opacity(0.))
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
                    .overflow_y_scroll()
                    .track_scroll(&self.chat_scroll)
                    // Room above the first turn (as much as the sides), and below the composer as a
                    // code editor scrolls past its end.
                    // With the first row's own TURN_Y: the avatar's top is CHAT_LEFT down.
                    .pt(px(super::CHAT_LEFT - super::TURN_Y))
                    .pb(px(past))
                    // The window's whole width from its left edge, as an editor's text (no cap,
                    // no centering).
                    .child(v_flex().w_full().children(rows)),
            )
            .children(rail)
            .when(!self.follow, |d| {
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

    /// The messages sent while the agent works, waiting in the thread's queue (Pi's inbox) until
    /// it answers. Each is shown as the person's own row — their avatar, their text, their files
    /// under it — because it is a message they have written, not a note about it: only a caption
    /// and its two controls say that it has not gone out yet (go now into the running work, or
    /// be taken back).
    fn render_queue(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.queue.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let rows: Vec<AnyElement> = self
            .queue
            .clone()
            .into_iter()
            .enumerate()
            .map(|(k, (id, mode, text))| {
                let (send_id, rm_id, edit_id, edit_text) =
                    (id.clone(), id.clone(), id.clone(), text.clone());
                let steer = mode == "steer";
                let (said, paths) = crate::attach::split(&text);
                // The row's own words are being fixed: its text gives way to a field.
                let field = self
                    .queue_edit
                    .as_ref()
                    .filter(|(e, _)| e == &id)
                    .map(|(_, st)| st.clone());
                let actions = h_flex()
                    .flex_none()
                    .gap_1()
                    .items_center()
                    .text_sm()
                    .text_color(muted)
                    .child(if field.is_some() {
                        "Editing"
                    } else if steer {
                        "Next"
                    } else {
                        "Queued"
                    })
                    .when(field.is_some(), |d| {
                        d.child(
                            icon_button(("queued-save", k), Ico::Check, "Keep these words (⌘Enter)", cx)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.click_claimed = true;
                                    this.queue_edit_save(window, cx);
                                })),
                        )
                    })
                    .when(field.is_none(), |d| {
                        d.child(
                            icon_button(
                                ("queued-edit", k),
                                Ico::Pencil,
                                "Fix the words before it goes out",
                                cx,
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                this.queue_edit_open(edit_id.clone(), edit_text.clone(), window, cx);
                            })),
                        )
                        .when(!steer, |d| {
                            d.child(
                                icon_button(
                                    ("queued-send", k),
                                    Ico::Play,
                                    "Send now: into the running work",
                                    cx,
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.click_claimed = true;
                                    this.queued(send_id.clone(), true, cx);
                                })),
                            )
                        })
                    })
                    .child(
                        icon_button(("queued-rm", k), Ico::X, "Take it back", cx).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                if this.queue_edit.as_ref().is_some_and(|(e, _)| e == &rm_id) {
                                    this.queue_edit_cancel(window, cx);
                                } else {
                                    this.queued(rm_id.clone(), false, cx);
                                }
                            }),
                        ),
                    )
                    .into_any_element();
                let editor = field.map(|st| {
                    // The same field as the composer's, at the same size and leading; the row is
                    // as tall as the text (no scroll of its own).
                    div()
                        .ml(px(-12.))
                        .my(px(-10.))
                        .line_height(relative(super::LEAD))
                        .child(Textarea::new(&st).appearance(false).large())
                        .into_any_element()
                });
                self.user_row(
                    format!("queued-{id}"),
                    &said,
                    &paths,
                    Some(actions),
                    true,
                    editor,
                    cx,
                )
            })
            .collect();
        Some(
            v_flex()
                .w_full()
                // A little lighter than the sent ones: it has not gone out.
                .opacity(0.85)
                .children(rows)
                .into_any_element(),
        )
    }

    /// The person's own row, as the thread shows it: their avatar in the gutter, their text at
    /// the thread's text column, the files that went with it under the text. One row for what is
    /// already sent and for what waits in the queue — a queued message is a message the person
    /// has written, not a note about it. `muted` marks one that has not gone out; `actions`
    /// (fix the words, send now, take it back) sit at the row's right; `field` replaces the text
    /// with the one being fixed.
    fn user_row(
        &self,
        key: String,
        text: &str,
        paths: &[String],
        actions: Option<AnyElement>,
        muted: bool,
        field: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        use super::{AVATAR, CHAT_LEFT, CHAT_RIGHT, CHAT_TEXT};
        let attached: Vec<AnyElement> = paths
            .iter()
            .enumerate()
            .map(|(k, p)| self.attachment_chip(format!("{key}-{k}").into(), p, None, cx))
            .collect();
        h_flex()
            .w_full()
            .pl(px(CHAT_LEFT))
            .pr(px(CHAT_RIGHT))
            .py(px(super::TURN_Y))
            .gap(px(CHAT_TEXT - CHAT_LEFT - AVATAR))
            .items_start()
            .child(super::avatar(false, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .whitespace_normal()
                    .when(muted, |d| d.text_color(theme.muted_foreground))
                    // Selectable and copyable, like the answers (the same text view).
                    .when(field.is_none(), |d| {
                        d.when(!text.is_empty(), |d| {
                            d.child(plain_text(SharedString::from(key.as_str()), text))
                        })
                    })
                    .when_some(field, |d, f| d.child(f))
                    // The files that went with it.
                    .when(!attached.is_empty(), |d| {
                        d.child(
                            h_flex()
                                .flex_wrap()
                                .gap_2()
                                .when(!text.is_empty(), |d| d.mt(px(10.)))
                                .pb(px(4.))
                                .children(attached),
                        )
                    }),
            )
            .children(actions)
            .into_any_element()
    }

    fn render_turn(&mut self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let turn: &Turn = &self.turns[i];
        let last = i + 1 == self.turns.len();
        let live = self
            .live
            .as_ref()
            .filter(|_| last)
            // Counted from the turn's own start (the user's message), so leaving the chat and
            // coming back, or reopening the app, keeps the real running time.
            .map(|l| {
                let since_send = l.started.elapsed().as_secs_f64();
                let since_turn = turn
                    .started
                    .map(|t| crate::app::now_secs() - t)
                    .filter(|d| *d >= 0.);
                (
                    l.label.clone(),
                    since_turn.unwrap_or(since_send).max(since_send) as u64,
                )
            });
        let tools = turn.tool_count();
        let errors = turn
            .steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { failed: true, .. }))
            .count();
        let has_steps = !turn.steps.is_empty();
        let expanded = turn.expanded;
        let group: SharedString = format!("turn{i}").into();

        // One line, and the counters come first: they hardly move, so the eye has one place for
        // them. What the agent says it is doing goes last, where the length may jump.
        let summary = (has_steps || live.is_some()).then(|| {
            let acts = if tools > 0 {
                plural(tools, "action", "actions", "actions")
            } else {
                String::new()
            };
            let errs = if errors > 0 {
                plural(errors, "error", "errors", "errors")
            } else {
                String::new()
            };
            // A live turn counts up; a finished one keeps the time it took.
            let time = match &live {
                Some((_, s)) => Some(clock(*s)),
                None => turn
                    .started
                    .zip(turn.finished)
                    .map(|(a, b)| clock((b - a).max(0.) as u64)),
            };
            // Each part after the first says so with a "· ".
            let sep = |first| if first { "" } else { "· " };
            let (has_a, has_e, has_t) = (!acts.is_empty(), !errs.is_empty(), time.is_some());
            let err_s = if has_e {
                format!("{}{errs}", sep(!has_a))
            } else {
                String::new()
            };
            let time_s = time.map(|t| format!("{}{t}", sep(!has_a && !has_e)));
            let said_s = sep(!has_a && !has_e && !has_t);
            h_flex()
                .id(("steps", i))
                .gap_1p5()
                .items_center()
                .text_base()
                .text_color(muted)
                .min_w_0()
                .when(has_steps, |d| {
                    d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
                })
                .child(
                    (if expanded {
                        Ico::ChevronDown
                    } else {
                        Ico::ChevronRight
                    })
                    // Shown from the start: an invisible one read as a stray indent before the
                    // first step arrived.
                    .icon()
                    .xsmall(),
                )
                .when(has_a, |d| d.child(div().flex_none().child(acts)))
                .when(has_e, |d| {
                    d.child(div().flex_none().text_color(theme.danger).child(err_s))
                })
                .when_some(time_s, |d, t| d.child(div().flex_none().child(t)))
                .map(|d| match &live {
                    // The agent's line takes what is left of the row and cuts with an ellipsis.
                    Some((label, _)) => d.child(
                        h_flex()
                            .min_w_0()
                            .flex_shrink(1.)
                            .child(div().flex_none().child(said_s.to_string()))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    // The size is said out loud, not inherited: the animated
                                    // label sits in a box of its own, and whatever it picks up
                                    // from around it must not be another body than the counters'
                                    // beside it.
                                    .child(
                                        ShimmerText::new(label.clone())
                                            .text_base()
                                            .text_color(muted),
                                    ),
                            ),
                    ),
                    None => d.child(div().flex_none().child(format!("{said_s}Done"))),
                })
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_turn(i, cx)))
        });

        let steps: Option<Div> = expanded.then(|| {
            let items: Vec<AnyElement> = (0..self.turns[i].steps.len())
                .map(|k| self.render_step(i, k, cx))
                .collect();
            v_flex()
                .ml(px(6.))
                .pl_4()
                .gap_1()
                .border_l_1()
                .border_color(theme.border)
                .text_base()
                .children(items)
        });

        let answer = self.render_answer(i, cx);
        let turn: &Turn = &self.turns[i];
        let has_answer = !turn.answer.trim().is_empty();
        let time = match (turn.started, turn.finished) {
            (Some(a), Some(b)) if b > a => clock((b - a) as u64),
            _ => String::new(),
        };
        let when = turn.finished.or(turn.started).unwrap_or_default();
        let (user, attached_paths) = crate::attach::split(&turn.user);

        let sources = self.render_sources(i, cx);
        // The session's edited files hang under the last answer: the agent made them too. (Its
        // background processes are a row of their own, where the work began.)
        let files = if i + 1 == self.turns.len() {
            self.render_session_files(cx)
        } else {
            None
        };
        // What the background review did with skills and memory, quietly under the answer.
        let reviews: Vec<AnyElement> = turn
            .reviews
            .iter()
            .map(|r| {
                let text = r.trim_start_matches('💾').trim().to_string();
                h_flex()
                    .gap_2()
                    .items_start()
                    .text_base()
                    .text_color(muted)
                    .child(Ico::Save.icon().xsmall().mt(px(2.)))
                    .child(div().flex_1().min_w_0().child(text))
                    .into_any_element()
            })
            .collect();
        let actions = (live.is_none() && has_answer).then(|| {
            let playing = self.playing.as_ref().is_some_and(|(j, _)| *j == i);
            let speaking = self.speaking == Some(i);
            // Over the top-right corner, on the line of the steps' summary: on hover only (and
            // while its audio plays), on the chat's own background so text under it is covered
            // cleanly when there is no summary line to sit beside.
            h_flex()
                .absolute()
                .top(px(super::TURN_Y + 3.))
                .right_6()
                .pl_1()
                .gap_0p5()
                .items_center()
                .bg(theme.background)
                .when(!(playing || speaking), |d| {
                    d.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.))
                })
                .when(!time.is_empty(), |d| {
                    d.child(
                        div()
                            .id(("time", i))
                            .px_1()
                            .text_base()
                            .text_color(muted)
                            .child(time)
                            .tooltip(move |window, cx| {
                                Tooltip::new(format_when(when)).build(window, cx)
                            }),
                    )
                })
                .child(icon_button(("copy", i), Ico::Copy, "Copy", cx).on_click(
                    cx.listener(move |this, _, window, cx| this.copy_answer(i, window, cx)),
                ))
                .child(
                    icon_button(
                        ("speak", i),
                        if playing {
                            Ico::Square
                        } else if speaking {
                            Ico::Loader
                        } else {
                            Ico::Volume
                        },
                        if playing { "Stop" } else { "Read aloud" },
                        cx,
                    )
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.toggle_speak(i, window, cx)),
                    ),
                )
                .when(last, |d| {
                    d.child(
                        icon_button(("regen", i), Ico::RotateCcw, "Regenerate", cx)
                            .on_click(cx.listener(|this, _, _, cx| this.regenerate(cx))),
                    )
                })
        });

        // Who speaks is an avatar in the left gutter, no band, no rules. The
        // question's text and the agent's side start at one x (CHAT_TEXT); the composer's text,
        // its chips and the cards start there too.
        use super::{AVATAR, CHAT_LEFT, CHAT_RIGHT, CHAT_TEXT};
        let avatar = |agent: bool| super::avatar(agent, &theme);
        v_flex()
            .id(("turn", i))
            .group(group)
            .w_full()
            // The conversation is what the window is for: a size up from the sidebar's lists.
            .text_base()
            .child(self.user_row(format!("user-{i}"), &user, &attached_paths, None, false, None, cx))
            .child(
                div()
                    .relative()
                    .w_full()
                    .child(
                        h_flex()
                            .pl(px(CHAT_LEFT))
                            .pr(px(CHAT_RIGHT))
                            .py(px(super::TURN_Y))
                            .gap(px(CHAT_TEXT - CHAT_LEFT - AVATAR))
                            .items_start()
                            .child(avatar(true))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_3()
                                    .children(summary)
                                    .children(steps)
                                    .children(answer)
                                    // What came with the answer, right under its text (the agent's side).
                                    .children(sources)
                                    .children(files)
                                    .when(!reviews.is_empty(), |d| {
                                        d.child(v_flex().gap_1().children(reviews))
                                    }),
                            ),
                    )
                    // After the column, so the actions paint over it.
                    .children(actions),
            )
            .into_any_element()
    }

    /// A row of the thread that is not on screen: its height, and nothing else.
    fn spacer(h: f32) -> AnyElement {
        div().flex_none().w_full().h(px(h.max(0.))).into_any_element()
    }

    /// Wrap a row so it says how tall it came out, while it is being laid out and without asking
    /// for another frame. `off` is how far the thread was scrolled when the band was picked; a
    /// row that ends above that grew the thread over what is being read, and the difference is
    /// banked in `rows_shift`, to be paid back at the start of the next frame.
    fn measured_row(
        &self,
        idx: usize,
        el: AnyElement,
        off: f32,
        view: Entity<Self>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .relative()
            .child(el)
            .child(
                canvas(
                    move |b, _, cx| {
                        view.update(cx, |this, _| {
                            let h = f32::from(b.size.height);
                            if let Some(delta) = this.rows.set_measured(idx, h) {
                                if delta != 0. && this.rows.start(idx) + h <= off + 1. {
                                    this.rows_shift += delta;
                                }
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }

    /// The turns as ticks along the right edge: this is the thread's navigator, not an addition
    /// to a scrollbar — there is no scrollbar, Bench never draws a thumb. The tick of the turn
    /// being read is lit, hover shows the question, a click jumps to that turn. Only with two
    /// turns or more, since with one there is nothing to move between.
    fn render_turn_rail(
        &mut self,
        at_bottom: bool,
        off: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let n = self.turns.len();
        if n < 2 {
            return None;
        }
        let theme = cx.theme().clone();
        // The turn being read: the last one that begins at or above the top of the viewport. The
        // model answers this for every turn, painted or not, since its height is known or counted
        // at the average of the measured ones.
        let current = if at_bottom {
            n - 1
        } else {
            (0..n)
                .filter(|i| self.rows.start(i + 1) <= off + 2.)
                .last()
                .unwrap_or(0)
        };
        // Ticks pack tighter as turns add up, so the rail stays a modest column.
        let pitch = (240. / n as f32).clamp(3., 8.);
        let ticks = (0..n).map(|i| {
            let on = i == current;
            let tip: SharedString = clip(self.turns[i].user.trim(), 120).into();
            div()
                .id(("rail", i))
                .h(px(pitch))
                .w_full()
                .flex()
                .items_center()
                .justify_end()
                .cursor_pointer()
                .group(SharedString::from(format!("rail-tick-{i}")))
                .child(
                    div()
                        .h(px(2.))
                        .w(px(10.))
                        .bg(if on {
                            theme.primary
                        } else {
                            theme.muted_foreground.opacity(0.35)
                        })
                        .group_hover(SharedString::from(format!("rail-tick-{i}")), |s| {
                            s.bg(theme.foreground)
                        }),
                )
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .on_click(cx.listener(move |this, _, _, cx| {
                    // Bring that row's top just under the top of the viewport: the thread scrolls
                    // on past its last line, so any row can be read from the top, as in an editor.
                    // The model's numbers answer for rows never painted; when such a row arrives
                    // and is measured, the frame holds the place it lands on.
                    let off = this.rows.start(i + 1) - 8.;
                    let max = f32::from(this.chat_scroll.max_offset().y);
                    this.follow = false;
                    this.click_claimed = true;
                    this.chat_scroll
                        .set_offset(point(px(0.), px(-off.clamp(0., max))));
                    cx.notify();
                }))
        });
        Some(
            v_flex()
                .absolute()
                .top_0()
                .bottom_0()
                // Where a scrollbar would be: the ticks are the navigator, so they own the edge.
                .right(px(4.))
                .w(px(14.))
                .justify_center()
                .child(v_flex().children(ticks))
                .into_any_element(),
        )
    }

    /// The agent waits: what it wants to run, and the answers it accepts.
    fn render_approval(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let req = self.approval.clone()?;
        let theme = cx.theme().clone();
        let label = |c: &str| match c {
            "once" => "Allow once",
            "session" => "Allow for this chat",
            "always" => "Always allow",
            "deny" => "Deny",
            _ => "OK",
        };
        let buttons = req.choices.iter().enumerate().map(|(k, c)| {
            let choice = c.clone();
            let b = Button::new(("approve", k)).small().label(label(c));
            let b = if k == 0 {
                b.primary()
            } else if c == "deny" {
                b.ghost()
            } else {
                b.outline()
            };
            b.on_click(cx.listener(move |this, _, _, cx| this.answer_approval(choice.clone(), cx)))
        });
        Some(
            v_flex()
                .ml(px(super::CHAT_TEXT))
                .mr(px(super::CHAT_RIGHT))
                .mt_2()
                .p_3()
                .gap_2()
                .border_1()
                .rounded_md()
                .border_color(theme.warning)
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_base()
                        .font_weight(FontWeight::MEDIUM)
                        .child(Ico::CircleAlert.icon().small().text_color(theme.warning))
                        .child(if req.description.is_empty() {
                            "The agent asks to run this".to_string()
                        } else {
                            req.description.clone()
                        }),
                )
                .when(!req.command.is_empty(), |d| {
                    d.child(
                        div()
                            .p_2()
                            .bg(theme.secondary)
                            .font_family(crate::theme::MONO)
                            .text_base()
                            .child(req.command.clone()),
                    )
                })
                .child(h_flex().gap_2().children(buttons))
                .into_any_element(),
        )
    }

    /// One step line; click unfolds its details (reasoning text, tool args and output).
    fn render_step(&self, i: usize, k: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.open_steps.contains(&(i, k));
        let (icon, color, label, details): (Ico, Hsla, String, Vec<(&str, String)>) =
            match &self.turns[i].steps[k] {
                Step::Tool {
                    name,
                    detail,
                    failed,
                    args,
                    output,
                    ..
                } => (
                    if *failed {
                        Ico::CircleAlert
                    } else {
                        Ico::Terminal
                    },
                    if *failed { theme.danger } else { muted },
                    clip(&tool_label(name, detail), 220),
                    [("Arguments", args.clone()), ("Output", output.clone())]
                        .into_iter()
                        .filter(|(_, v)| !v.trim().is_empty())
                        .collect(),
                ),
                Step::Note(text) => (
                    Ico::ChevronRight,
                    theme.foreground.opacity(0.75),
                    clip(text, 400),
                    vec![],
                ),
                Step::Thought(text) => (
                    Ico::Brain,
                    muted,
                    clip(text, 160),
                    vec![("Reasoning", text.clone())],
                ),
                Step::Subagent {
                    goal,
                    status,
                    lines,
                    summary,
                    ..
                } => (
                    Ico::Bot,
                    if status == "failed" || status == "error" {
                        theme.danger
                    } else {
                        muted
                    },
                    clip(
                        &match status.as_str() {
                            "" | "running" => format!("Subagent: {goal} — working…"),
                            s => format!("Subagent: {goal} — {s}"),
                        },
                        220,
                    ),
                    [("Activity", lines.join("\n")), ("Result", summary.clone())]
                        .into_iter()
                        .filter(|(_, v)| !v.trim().is_empty())
                        .collect(),
                ),
            };
        let expandable = !details.is_empty();
        v_flex()
            .child(
                h_flex()
                    .id(SharedString::from(format!("step-{i}-{k}")))
                    .gap_2()
                    .items_start()
                    .text_color(color)
                    .when(expandable, |d| {
                        d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
                    })
                    .child(icon.icon().xsmall().mt(px(3.)))
                    .child(div().flex_1().min_w_0().child(label))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_step(i, k, cx))),
            )
            .when(open && expandable, |d| {
                d.children(details.into_iter().map(|(title, body)| {
                    v_flex()
                        .ml_5()
                        .mt_1()
                        .mb_1()
                        .child(div().text_base().text_color(muted).child(title))
                        .child(
                            div()
                                .mt_0p5()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .font_family(crate::theme::MONO)
                                .text_base()
                                .text_color(theme.foreground)
                                // Verbatim and selectable, like the user's messages.
                                .child(plain_text(format!("step-{i}-{k}-{title}"), &body)),
                        )
                }))
            })
            .into_any_element()
    }

    /// One chip per site; click lists its pages, each opens on its own.
    fn render_sources(&self, i: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let urls = &self.turns[i].sources;
        if urls.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let mut hosts: Vec<(String, Vec<String>)> = vec![];
        for u in urls {
            let h = host_of(u);
            match hosts.iter_mut().find(|(x, _)| *x == h) {
                Some((_, pages)) => pages.push(u.clone()),
                None => hosts.push((h, vec![u.clone()])),
            }
        }
        hosts.sort_by_key(|(_, p)| std::cmp::Reverse(p.len()));
        let open_host = self
            .open_source
            .as_ref()
            .filter(|(t, _)| *t == i)
            .map(|(_, h)| h.clone());
        let pages = open_host
            .as_ref()
            .and_then(|h| hosts.iter().find(|(x, _)| x == h))
            .map(|(_, p)| p.clone())
            .unwrap_or_default();

        let chips: Vec<AnyElement> = hosts
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(k, (host, pages))| {
                let active = open_host.as_deref() == Some(&host);
                let single = (pages.len() == 1).then(|| pages[0].clone());
                let h = host.clone();
                h_flex()
                    .id(SharedString::from(format!("src-{i}-{k}")))
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .border_1()
                    .rounded_md()
                    .border_color(if active { theme.primary } else { theme.border })
                    .text_base()
                    .text_color(if active {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                    .child(Ico::Globe.icon().xsmall())
                    .child(host)
                    .when(pages.len() > 1, |d| {
                        d.child(div().opacity(0.6).child(pages.len().to_string()))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| match &single {
                        Some(url) => cx.open_url(url),
                        None => this.toggle_source(i, h.clone(), cx),
                    }))
                    .into_any_element()
            })
            .collect();

        let list = (!pages.is_empty()).then(|| {
            v_flex()
                .mt_1()
                .pl_2()
                .border_l_1()
                .border_color(theme.border)
                .children(pages.into_iter().enumerate().map(|(k, url)| {
                    let open = url.clone();
                    div()
                        .id(SharedString::from(format!("page-{i}-{k}")))
                        .text_base()
                        .text_color(theme.link)
                        .truncate()
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .child(
                            url.trim_start_matches("https://")
                                .trim_start_matches("http://")
                                .to_string(),
                        )
                        .on_click(move |_, _, cx| cx.open_url(&open))
                }))
        });

        Some(
            v_flex()
                .child(h_flex().flex_wrap().gap_1().children(chips))
                .children(list)
                .into_any_element(),
        )
    }

    /// The background processes the agent started in this session, as a row of its own where the
    /// work began: running or finished, each with Stop (while it runs) and its latest output a
    /// click away.
    fn render_processes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.processes.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0., |d| d.as_secs_f64());
        let running = self.processes.iter().filter(|p| p.running).count();
        let done = self.processes.len() - running;
        let open = self.processes_open;
        let mut bits = vec![];
        if running > 0 {
            bits.push(format!("{running} running"));
        }
        if done > 0 {
            bits.push(format!("{done} finished"));
        }
        let rows: Vec<AnyElement> = self
            .processes
            .iter()
            .filter(|_| open)
            .enumerate()
            .map(|(k, p)| {
                let failed = !p.running && p.exit_code.is_some_and(|c| c != 0);
                let elapsed = p
                    .started
                    .map(|t| (p.exited.unwrap_or(now) - t).max(0.) as u64)
                    .map(clock);
                let state = if p.running {
                    elapsed.unwrap_or_default()
                } else {
                    let code = p
                        .exit_code
                        .map_or("ended".to_string(), |c| format!("exit {c}"));
                    // A reason that only says it ended adds nothing to "exit N".
                    let reason = p.reason.as_deref().filter(|r| {
                        !r.is_empty() && !matches!(*r, "exited" | "exit" | "completed")
                    });
                    match (reason, elapsed) {
                        (Some(r), Some(e)) => format!("{code} · {r} · {e}"),
                        (_, Some(e)) => format!("{code} · {e}"),
                        _ => code,
                    }
                };
                let shown = self.process_open.as_deref() == Some(p.id.as_str());
                let (id_toggle, id_kill) = (p.id.clone(), p.id.clone());
                // "cd <project> && cargo build": the folder is the session's, the rest is news.
                let shown_cmd = match p.command.strip_prefix("cd ") {
                    Some(rest) => rest
                        .split_once(" && ")
                        .map_or(p.command.as_str(), |(_, cmd)| cmd),
                    None => p.command.as_str(),
                }
                .to_string();
                let full_cmd = p.command.clone();
                let tail: String = {
                    let lines: Vec<&str> = p.tail.trim_end().lines().collect();
                    lines[lines.len().saturating_sub(60)..].join("\n")
                };
                v_flex()
                    // Rules between the processes, not under the head.
                    .when(k > 0, |d| d.border_t_1().border_color(theme.border))
                    .child(
                        h_flex()
                            .id(("proc", k))
                            .px_2()
                            .py_1()
                            .gap_2()
                            .items_center()
                            .text_base()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(if p.running {
                                Ico::Loader.live(false)
                            } else if failed {
                                Ico::CircleAlert
                                    .icon()
                                    .xsmall()
                                    .text_color(theme.danger)
                                    .into_any_element()
                            } else {
                                Ico::CircleCheck
                                    .icon()
                                    .xsmall()
                                    .text_color(muted)
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .id(("proc-cmd", k))
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(crate::theme::MONO)
                                    .child(shown_cmd)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(full_cmd.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(if failed { theme.danger } else { muted })
                                    .child(state),
                            )
                            .when(p.running, |d| {
                                d.child(
                                    div()
                                        .id(("proc-stop", k))
                                        .flex_none()
                                        .px_1p5()
                                        .text_color(muted)
                                        .hover(|s| {
                                            s.bg(theme.danger).text_color(theme.danger_foreground)
                                        })
                                        .child("Stop")
                                        .tooltip(|window, cx| {
                                            Tooltip::new("Stop this process").build(window, cx)
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.kill_process(id_kill.clone(), cx)
                                        })),
                                )
                            })
                            .child(
                                if shown {
                                    Ico::ChevronDown
                                } else {
                                    Ico::ChevronRight
                                }
                                .icon()
                                .xsmall()
                                .text_color(muted),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_process_output(id_toggle.clone(), cx)
                            })),
                    )
                    .when(shown, |d| {
                        d.child(
                            div()
                                .mx_2()
                                .mb_2()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .font_family(crate::theme::MONO)
                                .text_base()
                                .text_color(theme.foreground)
                                .child(if tail.trim().is_empty() {
                                    div()
                                        .text_color(muted)
                                        .child("No output yet")
                                        .into_any_element()
                                } else {
                                    plain_text(format!("proc-out-{k}"), &tail).into_any_element()
                                }),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let head = super::block_head(&theme, open)
            .id("procs-head")
            // As short as the thread's other small rows, not as tall as a block's head.
            .h(px(super::CHIP_H))
            .gap_1p5()
            .cursor_pointer()
            .hover(|s| s.text_color(theme.foreground))
            .child(
                if open {
                    Ico::ChevronDown
                } else {
                    Ico::ChevronRight
                }
                .icon()
                .xsmall(),
            )
            .child(Ico::Terminal.icon().xsmall())
            .child(
                div()
                    .flex_none()
                    .child(format!("BACKGROUND · {}", bits.join(" · "))),
            )
            .when(running > 0, |d| d.child(Ico::Loader.live(false)))
            .when(!open, |d| d.child(div().flex_1()))
            .on_click(cx.listener(|this, _, _, cx| {
                // Folding is the thread's own click: it must not pull the focus to the composer.
                this.click_claimed = true;
                this.processes_open = !this.processes_open;
                cx.notify();
            }));
        let frame = v_flex()
            .id("procs-block")
            .w_full()
            .min_w_0()
            .flex_none()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            .child(head)
            .when(open, |d| d.children(rows));
        // The agent's row, as its plan's: its avatar, the block in the text column.
        Some(
            h_flex()
                .w_full()
                .flex_none()
                .id("procs-row")
                .pl(px(super::CHAT_LEFT))
                .pr(px(super::CHAT_RIGHT))
                .py(px(super::TURN_Y))
                .gap(px(super::CHAT_TEXT - super::CHAT_LEFT - super::AVATAR))
                .items_start()
                .child(super::avatar(true, &theme))
                .child(div().flex_1().min_w_0().child(frame))
                .into_any_element(),
        )
    }

    /// After the last turn: every file this session edited, with +/- counts.
    fn render_session_files(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let files = self.session_files();
        if files.is_empty() || self.live.is_some() {
            return None;
        }
        let theme = cx.theme().clone();
        let open = self.files_open;
        let count = files.len();
        let (all_add, all_del) = files
            .iter()
            .flat_map(|(_, diffs)| diffs.iter().map(|d| crate::diff::stats(d)))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let rows: Vec<AnyElement> = files
            .into_iter()
            .filter(|_| open)
            .enumerate()
            .map(|(k, (path, diffs))| {
                let (add, del) = diffs
                    .iter()
                    .map(|d| crate::diff::stats(d))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
                let p = std::path::Path::new(&path);
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let dir = p
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let selected = false;
                let open = path.clone();
                h_flex()
                    .id(("file", k))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .items_center()
                    .text_base()
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .when(selected, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(name)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_base()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(dir),
                    )
                    .when(add > 0, |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.success)
                                .child(format!("+{add}")),
                        )
                    })
                    .when(del > 0, |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.danger)
                                .child(format!("−{del}")),
                        )
                    })
                    .when(diffs.is_empty(), |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.muted_foreground)
                                .child("written"),
                        )
                    })
                    // In Zed, the code editor beside Bench.
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let host = this.thread_host();
                        this.open_in_zed(&open, None, Some(host.as_str()), cx)
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                // Folded by default: one line with the totals; a click lists the files.
                .child(
                    h_flex()
                        .id("files-head")
                        // As tall as a source chip (px_1p5 py_0p5 around text_xs).
                        .px_1p5()
                        .py_0p5()
                        .gap_1p5()
                        .items_center()
                        .text_base()
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .hover(|s| s.text_color(theme.foreground))
                        .when(open, |d| d.border_b_1().border_color(theme.border))
                        .child(
                            if open {
                                Ico::ChevronDown
                            } else {
                                Ico::ChevronRight
                            }
                            .icon()
                            .xsmall(),
                        )
                        .child(format!(
                            "CHANGED FILES · {}",
                            plural(count, "file", "files", "files")
                        ))
                        .when(all_add > 0, |d| {
                            d.child(div().text_color(theme.success).child(format!("+{all_add}")))
                        })
                        .when(all_del > 0, |d| {
                            d.child(div().text_color(theme.danger).child(format!("−{all_del}")))
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.files_open = !this.files_open;
                            cx.notify();
                        })),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

pub(super) fn icon_button(
    id: impl Into<ElementId>,
    icon: Ico,
    tip: &'static str,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .p_1()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.live(false))
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// "21.10.2026 22:15" for a Unix timestamp, in local time.
fn format_when(ts: f64) -> String {
    let out = std::process::Command::new("date")
        .args(["-r", &(ts as i64).to_string(), "+%d.%m.%Y %H:%M"])
        .output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
