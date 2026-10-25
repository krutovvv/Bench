//! The conversation as one scrolling column: per turn the user message, folded steps
//! (each unfoldable), the answer, sources and hover actions; under the last answer, the
//! files this session changed.

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

/// Below the composer the thread can scroll on, as a code editor past its last line.
const SCROLL_PAST_END: f32 = 240.;
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
    pub(super) fn render_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // A thread with no turns yet still is a thread: "Loading history…" while it opens, then
        // the composer; a new chat is the composer alone.
        // A thread about something starts with it, live: a task list, a server's terminal, a tool.
        // @ in the composer offers the open thread's project's prompts.
        if !self.demo && self.conn.is_some() && self.prompts_for.as_deref() != Some(self.thread_project.as_str()) {
            self.prompts_for = Some(self.thread_project.clone());
            self.load_prompts(window, cx);
        }
        // A project's environment is the thread's document, at its top; everything else the
        // person puts in is an insert (crate::ui::inserts), where it was put.
        let subject = self.current_subject();
        let block = if let Some(id) = subject.strip_prefix("env:") {
            if self.project_env.as_ref().is_none_or(|e| e.id != id) {
                self.load_env(id, window, cx);
            }
            Some(self.render_env_block(window, cx))
        } else {
            None
        };
        // The block comes from the agent: its row, its avatar, like an answer.
        let theme = cx.theme().clone();
        // `BENCH_DEMO=new-project`: the new project's form, once (for screenshots).
        if self.demo && !self.subject_opened && std::env::var("BENCH_DEMO").as_deref() == Ok("new-project") {
            self.subject_opened = true;
            self.new_project_form(window, cx);
        }
        // A form (a new project; a new server or file host) is a block of the thread too: the new
        // project's at the top of its empty thread, the others with the tools, under the
        // conversation.
        // A form opened from an insert shows under that insert (`render_inserts_at`).
        let under_insert = self.form_after.as_ref().is_some_and(|id| self.thread_inserts().iter().any(|i| &i.id == id));
        let form: Option<(AnyElement, bool)> = if self.project_form.is_some() {
            Some((self.render_project_form(cx), true))
        } else if under_insert {
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
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, _| {
                            this.click_claimed = true;
                            this.follow = false;
                        }))
                        .child(content),
                )
                .into_any_element()
        });
        let mut subject_block = subject_block;
        let placeholder: Option<AnyElement> = if subject_block.is_some() || !self.thread_inserts().is_empty() {
            None
        } else if !self.turns.is_empty() {
            None
        } else if self.session.is_some() {
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
            // A new chat is a clean sheet, as in Delta: only the composer. Its folder is the
            // group's "+" or the sidebar's "Open folder…".
            None
        };

        // Follow the tail: stay pinned to the composer until the user scrolls up. "The bottom" is
        // the composer's, not the room the thread scrolls on past it (SCROLL_PAST_END), so a
        // short thread stays put and nothing calls for "Jump to latest".
        let max = (self.chat_scroll.max_offset().y - px(SCROLL_PAST_END)).max(px(0.));
        let at_bottom = -self.chat_scroll.offset().y >= max - px(4.);
        if self.follow && !at_bottom && max == self.last_max {
            self.follow = false; // the user scrolled away
        }
        if at_bottom {
            self.follow = true;
        }
        if self.follow && !at_bottom {
            self.chat_scroll.set_offset(point(px(0.), -max));
        }
        self.last_max = max;

        // The turns, and after each the inserts the person put there.
        let n = self.turns.len();
        let mut rows: Vec<AnyElement> = self.render_inserts_at(0, n == 0, window, cx);
        for i in 0..n {
            rows.push(self.render_turn(i, cx));
            rows.extend(self.render_inserts_at(i + 1, i + 1 == n, window, cx));
        }
        let approval = self.render_approval(cx);
        let clarify = self.render_clarify(cx);
        let rail = self.render_turn_rail(at_bottom, cx);
        // The environment's open row, as the agent's answer under it.
        let env_card = if subject.starts_with("env:") {
            self.render_env_card(cx).map(|c| self.agent_row(c, cx))
        } else {
            None
        };
        let queue = self.render_queue(cx);
        let composer = self.render_composer(cx);

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
                                (p.x - ev.position.x).abs() < px(4.) && (p.y - ev.position.y).abs() < px(4.)
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
                    .pb(px(SCROLL_PAST_END))
                    // The window's whole width from its left edge, as an editor's text (no cap,
                    // no centering).
                    .child(
                        v_flex()
                            .w_full()
                            .when(block_on_top, |d| d.children(subject_block.take()).children(env_card))
                            .children(placeholder)
                            .children(rows)
                            .children(approval)
                            .children(clarify)
                            .children(subject_block)
                            // What waits for the agent, over the next message.
                            .children(queue)
                            // The next message is the thread's last row, as in Delta: it scrolls
                            // away with the thread, "Jump to latest" brings it back.
                            .child(composer),
                    ),
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
                        .border_1().rounded_md()
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
    /// it answers: each can go now (into the running work) or be taken back.
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
                let (send_id, rm_id) = (id.clone(), id.clone());
                let steer = mode == "steer";
                let first = text.lines().next().unwrap_or_default().to_string();
                h_flex()
                    .id(("queued", k))
                    .gap_2()
                    .px_3()
                    .py_1()
                    .items_center()
                    .border_1()
                    .rounded_md()
                    .border_color(theme.border)
                    .child(div().flex_none().text_sm().text_color(muted).child(if steer { "Next" } else { "Queued" }))
                    .child(div().flex_1().min_w_0().truncate().child(first))
                    .when(!steer, |d| {
                        d.child(icon_button(("queued-send", k), Ico::Play, "Send now: into the running work", cx).on_click(cx.listener(move |this, _, _, cx| {
                            this.click_claimed = true;
                            this.queued(send_id.clone(), true, cx);
                        })))
                    })
                    .child(icon_button(("queued-rm", k), Ico::X, "Take it back", cx).on_click(cx.listener(move |this, _, _, cx| {
                        this.click_claimed = true;
                        this.queued(rm_id.clone(), false, cx);
                    })))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .w_full()
                .pl(px(super::CHAT_TEXT))
                .pr(px(super::CHAT_RIGHT))
                .pt(px(super::TURN_Y))
                .gap_1()
                .children(rows)
                .into_any_element(),
        )
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

        // One-line summary; only the error count is red.
        let summary = (has_steps || live.is_some()).then(|| {
            let mut tail: Vec<String> = vec![];
            if tools > 0 {
                tail.push(plural(tools, "action", "actions", "actions"));
            }
            if let Some((_, s)) = &live {
                tail.push(clock(*s));
            }
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
                .map(|d| match &live {
                    // One line: what the agent says it is doing, cut with "…" before the counters.
                    Some((label, _)) => d.child(
                        div()
                            .flex_shrink(1.)
                            .min_w_0()
                            .truncate()
                            .child(ShimmerText::new(label.clone()).text_color(muted)),
                    ),
                    None => d.child(div().flex_none().child("Done")),
                })
                .when(!tail.is_empty(), |d| {
                    d.child(div().flex_none().child(format!("· {}", tail.join(" · "))))
                })
                .when(errors > 0, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .text_color(theme.danger)
                            .child(format!("· {}", plural(errors, "error", "errors", "errors"))),
                    )
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
        let has_text = !user.is_empty();
        let mut attached: Vec<AnyElement> = vec![];
        attached.extend(
            attached_paths
                .iter()
                .enumerate()
                .map(|(k, p)| self.attachment_chip(format!("sent-{i}-{k}").into(), p, None, cx)),
        );

        let sources = self.render_sources(i, cx);
        // The session's edited files and background processes hang under the last answer: the
        // agent made them too.
        let (processes, files) = if i + 1 == self.turns.len() {
            (self.render_processes(cx), self.render_session_files(cx))
        } else {
            (None, None)
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

        // As in Zed's Delta: who speaks is an avatar in the left gutter, no band, no rules. The
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
            .child(
                h_flex()
                    .w_full()
                    .pl(px(CHAT_LEFT))
                    .pr(px(CHAT_RIGHT))
                    .py(px(super::TURN_Y))
                    .gap(px(CHAT_TEXT - CHAT_LEFT - AVATAR))
                    .items_start()
                    .child(avatar(false))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            // Selectable and copyable, like the answers (the same text view).
                            .when(!user.is_empty(), |d| {
                                d.child(plain_text(format!("user-{i}"), &user))
                            })
                            // The files that went with it.
                            .when(!attached.is_empty(), |d| {
                                d.child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap_1p5()
                                        .when(has_text, |d| d.mt_2())
                                        .children(attached),
                                )
                            }),
                    ),
            )
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
                                    .children(processes)
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

    /// The turns as ticks in the right gutter (no scrollbar): which one is on screen, a peek at
    /// each question on hover, a click to jump. Only with two turns or more.
    fn render_turn_rail(&mut self, at_bottom: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let n = self.turns.len();
        if n < 2 {
            return None;
        }
        let theme = cx.theme().clone();
        let current = if at_bottom {
            n - 1
        } else {
            self.chat_scroll.top_item().min(n - 1)
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
                        .w(px(8.))
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
                    this.follow = false;
                    this.chat_scroll.scroll_to_top_of_item(i);
                    cx.notify();
                }))
        });
        Some(
            v_flex()
                .absolute()
                .top_0()
                .bottom_0()
                .right(px(2.))
                .w(px(12.))
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
                .border_1().rounded_md()
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
                                .border_1().rounded_md()
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
                    .border_1().rounded_md()
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

    /// Under the last answer: the background processes the agent started in this session —
    /// running or finished, each with Stop (while it runs) and its latest output a click away.
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
        let mut head = vec![];
        if running > 0 {
            head.push(format!("{running} running"));
        }
        if done > 0 {
            head.push(format!("{done} finished"));
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
                    .border_t_1()
                    .border_color(theme.border)
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
                                .border_1().rounded_md()
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
        Some(
            v_flex()
                .border_1().rounded_md()
                .border_color(theme.border)
                .child(
                    h_flex()
                        .id("procs-head")
                        // As tall as a source chip (px_1p5 py_0p5 around text_xs).
                        .px_1p5()
                        .py_0p5()
                        .gap_1p5()
                        .items_center()
                        .text_base()
                        .text_color(muted)
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
                        .child(format!("BACKGROUND · {}", head.join(" · ")))
                        .when(running > 0, |d| d.child(Ico::Loader.live(false)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.processes_open = !this.processes_open;
                            cx.notify();
                        })),
                )
                .children(rows)
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
                .border_1().rounded_md()
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
