//! Sending: the composer's message, copy, an approval (the view's); the reasoning
//! level and Stop (the app's: the status bar's controls act on the open thread). The queue is
//! `queued.rs`, drafts `drafts.rs`.

use std::sync::Arc;

use gpui_kit::component::IndexPath;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::ThreadView;
use crate::app::model::Target;
use crate::app::{BenchApp, Live};
use crate::client::connection::{self, Connection};
use crate::client::rpc::Cmd;
use crate::media::attach;

impl BenchApp {
    /// The reasoning levels offered anew (the presets'): the composer's choice follows, the
    /// level kept where it is still offered.
    pub fn set_efforts(
        &mut self,
        levels: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.efforts.get(self.thread(cx).effort).cloned();
        let profile = self.efforts.get(self.profile_effort).cloned();
        self.efforts = levels;
        let last = self.efforts.len().saturating_sub(1);
        let at = |v: Option<String>| v.and_then(|v| self.efforts.iter().position(|e| *e == v));
        let selected = at(current).unwrap_or(last);
        self.profile_effort = at(profile).unwrap_or(last);
        self.active_thread
            .update(cx, |v, _| v.thread.effort = selected);
        let labels: Vec<SharedString> = self
            .efforts
            .iter()
            .map(|e| SharedString::from(e.clone()))
            .collect();
        self.effort_choice.update(cx, |s, cx| {
            s.set_items(labels, window, cx);
            s.set_selected_index(Some(IndexPath::new(selected)), window, cx)
        });
    }

    /// An open session takes the level at once (its next turn runs on it, the gateway confirms
    /// with session.info); a chat not created yet takes it with its first message.
    pub fn set_effort(&mut self, i: usize, cx: &mut Context<Self>) {
        let runtime = self.active_thread.update(cx, |v, _| {
            v.thread.effort = i;
            v.thread.effort_other = None;
            v.thread.effort_pending = v.thread.runtime.is_none();
            v.thread.runtime.clone()
        });
        if let Some(session) = runtime {
            let at = self.thread_ref(cx);
            self.cmd_at(
                &at,
                Cmd::SetEffort {
                    session,
                    effort: self.efforts[i].clone(),
                },
            );
        }
        cx.notify();
    }

    /// The status bar's Stop, ⌘.: the open thread's run.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        let at = self.thread_ref(cx);
        if let (Some(profile), Some(session)) = (self.profile.clone(), at.runtime.clone()) {
            self.cmd_at(&at, Cmd::Interrupt { profile, session });
        }
        cx.notify();
    }

    /// A new thread's session, on its first message: in a group's folder when one was named (made
    /// where the gateway lives), else in the new chat's project folder.
    pub(crate) fn create_session(
        &mut self,
        profile: String,
        at: &crate::thread::ThreadRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let group = self.group_name.read(cx).value().trim().to_string();
        if self.new_project.is_none() && !group.is_empty() {
            self.group_name
                .update(cx, |s, cx| s.set_value("", window, cx));
            let target = self.target.clone();
            let at = at.clone();
            self.background(
                window,
                cx,
                move || connection::group_dir(&target, &group),
                move |this, r, window, cx| match r {
                    Ok(dir) => this.cmd_at(&at, this.create_cmd(profile, Some(dir), &at.project)),
                    Err(e) => {
                        this.toast(
                            NotificationType::Error,
                            format!("Couldn't make the group's folder: {e}"),
                            window,
                            cx,
                        );
                        this.cmd_at(&at, this.create_cmd(profile, None, &at.project));
                    }
                },
            );
            cx.notify();
            return;
        }
        self.cmd_at(
            at,
            self.create_cmd(profile, self.new_project.clone(), &at.project),
        );
        cx.notify();
    }
}

impl ThreadView {
    /// The work row's hold: stand the run at its next step boundary, or let it go on.
    pub fn set_paused(&mut self, paused: bool, cx: &mut Context<Self>) {
        if let Some(session) = self.thread_ref().runtime {
            self.cmd(Cmd::SetPaused { session, paused }, cx);
        }
        cx.notify();
    }

    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        // While dictating, Send stops the recording and sends what it recognizes.
        let dictating = {
            let a = app.read(cx);
            a.recorder.is_some() || a.transcribing
        };
        if dictating {
            app.update(cx, |app, cx| {
                app.send_after_transcribe = true;
                if app.recorder.is_some() {
                    app.toggle_mic(window, cx);
                }
                cx.notify();
            });
            return;
        }
        let typed = self.composer.read(cx).value().trim().to_string();
        let Some(profile) = self.profile(cx) else {
            return;
        };
        // The code sent from the Explorer goes under the words.
        let typed = super::attachments::with_references(typed, &self.thread_references(cx));
        let files = self.attachments.clone();
        if typed.is_empty() && files.is_empty() {
            return;
        }
        // Enter in the command list picks the command instead of sending.
        if files.is_empty() && self.complete_slash(window, cx) {
            return;
        }
        // "!command": run it in this thread's terminal (opened under the thread).
        if let Some(command) = typed.strip_prefix('!') {
            let command = command.trim().to_string();
            self.composer
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.run_bang(command, window, cx);
            return;
        }
        // While the agent works, the message waits in the thread's queue (Pi's inbox) until the
        // answer is ready — with its files too: they are staged first (copied aside, uploaded
        // over SSH), then the same queue. The old refusal ("files go with the next message")
        // was ours, not Pi's: the inbox takes a follow-up with attachments like any other.
        if self.thread.live.is_some() {
            let Some(session) = self.thread.runtime.clone() else {
                return;
            };
            let text = typed;
            self.take_composer(window, cx);
            let context = self.block_context(cx);
            // It becomes the thread's next message: the inserts that waited with it go after it.
            let after = self.thread.journal.messages() + 1;
            if files.is_empty() {
                // The waiting row shows the frame it was pressed, not the frame the agent's
                // queue answers with: the composer must never empty onto nothing.
                self.thread.journal.queued_now(text.clone());
                self.follow = true;
                self.cmd(
                    Cmd::Queue {
                        session,
                        text,
                        context,
                    },
                    cx,
                );
                self.anchor_draft_inserts(after, cx);
                cx.notify();
                return;
            }
            let Some((conn, target, machine)) = self.files_where(cx) else {
                return;
            };
            let profile2 = profile.clone();
            let keep = files.clone();
            cx.spawn_in(window, async move |this, cx| {
                let prepared = cx
                    .background_spawn(async move {
                        attach::prepare(&conn, &target, machine.as_deref(), &profile2, &files)
                    })
                    .await;
                let _ = this.update_in(cx, |this, window, cx| match prepared {
                    Ok(prep) => {
                        // The thread he sent from, even if another is open by now.
                        let same = this.thread.runtime.as_deref() == Some(session.as_str());
                        this.cmd(
                            Cmd::Queue {
                                session,
                                text: with_attachments(&text, &prep.lines),
                                context,
                            },
                            cx,
                        );
                        if same {
                            this.anchor_draft_inserts(after, cx);
                        }
                    }
                    Err(e) => {
                        this.attachments = keep;
                        window.push_notification(
                            (
                                NotificationType::Error,
                                SharedString::from(format!("Attach: {e:#}")),
                            ),
                            cx,
                        );
                    }
                });
            })
            .detach();
            return;
        }
        // Code from the IDE follows the words, each block naming its file and lines.
        let text = typed;
        self.take_composer(window, cx);
        let shown: Vec<String> = files
            .iter()
            .map(|f| format!("{}{}]", attach::PREFIX, f.display()))
            .collect();
        let shown = with_attachments(&text, &shown);
        self.thread.journal.sent(shown.clone());
        // Inserts put while this message was in the composer sit after it, not before.
        let after = self.thread.journal.messages();
        self.anchor_draft_inserts(after, cx);
        self.follow = true;
        // The status bar's speed is the open thread's.
        if self.is_open_thread(cx) {
            app.update(cx, |app, _| {
                app.speed.begin_answer();
                app.speed.request();
            });
        }
        self.thread.live = Some(Live {
            label: if files.is_empty() {
                "Sending…"
            } else {
                "Uploading…"
            }
            .into(),
        });
        if files.is_empty() {
            self.submit(profile, text, window, cx);
            return;
        }
        let Some((conn, target, machine)) = self.files_where(cx) else {
            return;
        };
        let keep = files.clone();
        cx.spawn_in(window, async move |this, cx| {
            let p = profile.clone();
            let prepared = cx
                .background_spawn(async move {
                    attach::prepare(&conn, &target, machine.as_deref(), &p, &files)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match prepared {
                Ok(prep) => {
                    if let Some(live) = this.thread.live.as_mut() {
                        live.label = "Sending…".into();
                    }
                    let full = with_attachments(&text, &prep.lines);
                    // Shown with the files where they were; sent with where they are now.
                    this.thread.journal.resent(&shown, full.clone());
                    this.submit(profile, full, window, cx);
                }
                Err(e) => {
                    this.thread.live = None;
                    this.thread.journal.unsent(&shown);
                    // His files come back: the message stays his to send again.
                    this.attachments = keep;
                    window.push_notification(
                        (
                            NotificationType::Error,
                            SharedString::from(format!("Attach: {e:#}")),
                        ),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    /// What was in the composer has gone: its text, its draft, its files and references.
    fn take_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.note_draft(cx);
        self.attachments.clear();
        let key = self.thread_ref().key();
        self.app
            .update(cx, |app, _| app.references.remove(&key))
            .ok();
    }

    /// Where this thread's files go to: its agent, the machine its files are on.
    fn files_where(&self, cx: &App) -> Option<(Arc<Connection>, Target, Option<String>)> {
        let app = self.app(cx)?;
        let at = self.thread_ref();
        Some((
            app.thread_conn(&at)?,
            app.thread_target(&at),
            app.files_machine(&at),
        ))
    }

    /// What the thread's inserts show right now, for the agent (sent with each message).
    pub(crate) fn block_context(&self, cx: &App) -> String {
        self.app(cx)
            .map(|a| a.inserts_context(&self.thread_ref(), cx))
            .unwrap_or_default()
    }

    /// The message (with its files named in the text) to the session, or to a new one.
    pub(crate) fn submit(
        &mut self,
        profile: String,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.thread.runtime.clone() {
            Some(session) => {
                let command = self.app(cx).and_then(|a| a.parse_command(&text));
                match command {
                    Some((name, arg)) => self.cmd(
                        Cmd::Dispatch {
                            profile,
                            session,
                            name,
                            arg,
                        },
                        cx,
                    ),
                    None => {
                        let efforts = self.app(cx).map(|a| a.efforts.clone()).unwrap_or_default();
                        let effort = self.thread.take_effort(&efforts);
                        let context = self.block_context(cx);
                        self.cmd(
                            Cmd::Send {
                                profile,
                                session,
                                text,
                                effort,
                                context,
                            },
                            cx,
                        );
                    }
                }
            }
            None => {
                // A new session is created on the first message; the effort goes with it.
                self.thread.effort_pending = true;
                self.thread.queued = Some(text);
                let at = self.thread_ref();
                self.app
                    .update(cx, |app, cx| app.create_session(profile, &at, window, cx))
                    .ok();
            }
        }
        cx.notify();
    }

    /// Copy what the agent said in the row `key`.
    pub fn copy_answer(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.answer_text(key) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            window.push_notification(
                (NotificationType::Success, SharedString::from("Copied")),
                cx,
            );
        }
    }

    pub(crate) fn answer_text(&self, key: &str) -> Option<String> {
        self.thread.tv.rows.iter().find_map(|r| match r {
            ::thread::Row::Answer { key: k, text, .. } if k == key => Some(text.clone()),
            _ => None,
        })
    }

    pub fn answer_approval(&mut self, choice: String, cx: &mut Context<Self>) {
        let (Some(req), Some(profile)) = (self.thread.approval.take(), self.profile(cx)) else {
            return;
        };
        if let Some(live) = self.thread.live.as_mut() {
            live.label = "Working…".into();
        }
        self.cmd(
            Cmd::Approve {
                profile,
                req,
                choice,
            },
            cx,
        );
        cx.notify();
    }
}

/// The message's text with the files' `[attached: …]` lines under it.
pub(crate) fn with_attachments(text: &str, lines: &[String]) -> String {
    std::iter::once(text.to_string())
        .filter(|t| !t.is_empty())
        .chain(lines.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n\n")
}
