//! Sending: the composer's message, the queue, the reasoning level, regenerate, copy, stop, an approval, drafts.

use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::attach;
use crate::connection::{self};
use crate::gateway::Cmd;

use super::*;

impl BenchApp {
    /// A waiting message (the thread's queue): sent now, into the running work, or taken back.
    pub fn queued(&mut self, id: String, send_now: bool, cx: &mut Context<Self>) {
        let Some(session) = self.runtime.clone() else {
            return;
        };
        self.cmd(if send_now {
            Cmd::QueueSend {
                session: session.clone(),
                id: id.clone(),
            }
        } else {
            Cmd::QueueWithdraw {
                session,
                id: id.clone(),
            }
        });
        cx.notify();
    }

    /// Fix the words of a waiting message. It has not gone anywhere, so a wrong word is cheap to
    /// change; one the agent has already taken up cannot (Pi settles it) — that row is gone by then.
    pub fn queue_edit_open(
        &mut self,
        id: String,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 100_000)
                .submit_on_enter(false)
        });
        field.update(cx, |s, cx| s.set_value(text, window, cx));
        cx.subscribe_in(&field, window, |this, _, ev: &InputEvent, window, cx| {
            // Enter is a new line, as in the composer; ⌘Enter keeps the change.
            if matches!(
                ev,
                InputEvent::PressEnter {
                    secondary: true,
                    ..
                }
            ) {
                this.queue_edit_save(window, cx);
            }
        })
        .detach();
        self.queue_edit = Some((id, field.clone()));
        // The click that opened this field is its own: the thread's bare-click rule (the composer
        // takes the focus) must not undo the focus this field just took.
        self.click_claimed = true;
        field.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn queue_edit_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, field)) = self.queue_edit.take() else {
            return;
        };
        let text = field.read(cx).value().trim().to_string();
        if let Some(session) = self.runtime.clone() {
            if !text.is_empty() {
                self.cmd(Cmd::QueueEdit { session, id, text });
            }
        }
        cx.notify();
    }

    pub fn queue_edit_cancel(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.queue_edit = None;
        cx.notify();
    }

    /// An open session takes the level at once (its next turn runs on it, the gateway confirms
    /// with session.info); a chat not created yet takes it with its first message.
    /// The reasoning levels offered anew (the presets'): the composer's choice follows, the
    /// level kept where it is still offered.
    pub fn set_efforts(
        &mut self,
        levels: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.efforts.get(self.effort).cloned();
        let profile = self.efforts.get(self.profile_effort).cloned();
        self.efforts = levels;
        let last = self.efforts.len().saturating_sub(1);
        let at = |v: Option<String>| v.and_then(|v| self.efforts.iter().position(|e| *e == v));
        self.effort = at(current).unwrap_or(last);
        self.profile_effort = at(profile).unwrap_or(last);
        let labels: Vec<SharedString> = self
            .efforts
            .iter()
            .map(|e| SharedString::from(e.clone()))
            .collect();
        let selected = self.effort;
        self.effort_choice.update(cx, |s, cx| {
            s.set_items(labels, window, cx);
            s.set_selected_index(Some(IndexPath::new(selected)), window, cx)
        });
    }

    pub fn set_effort(&mut self, i: usize, cx: &mut Context<Self>) {
        self.effort = i;
        self.effort_other = None;
        match self.runtime.clone() {
            Some(session) => {
                self.effort_pending = false;
                self.cmd(Cmd::SetEffort {
                    session,
                    effort: self.efforts[i].clone(),
                });
            }
            None => self.effort_pending = true,
        }
        cx.notify();
    }

    pub(crate) fn take_effort(&mut self) -> Option<String> {
        std::mem::take(&mut self.effort_pending)
            .then(|| self.efforts.get(self.effort).cloned())
            .flatten()
    }

    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // While dictating, Send stops the recording and sends what it recognizes.
        if self.recorder.is_some() || self.transcribing {
            self.send_after_transcribe = true;
            if self.recorder.is_some() {
                self.toggle_mic(window, cx);
            }
            cx.notify();
            return;
        }
        let typed = self.composer.read(cx).value().trim().to_string();
        let Some(profile) = self.profile.clone() else {
            return;
        };
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
        if self.live.is_some() {
            let Some(session) = self.runtime.clone() else {
                return;
            };
            let text = typed;
            self.composer
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.note_draft(cx);
            self.attachments.clear();
            let context = self.block_context(cx);
            // It becomes the thread's next message: the inserts that waited with it go after it.
            let after = self.thread.messages() + 1;
            if files.is_empty() {
                self.cmd(Cmd::Queue {
                    session,
                    text,
                    context,
                });
                self.anchor_draft_inserts(after, cx);
                cx.notify();
                return;
            }
            let Some(conn) = self.thread_conn() else {
                return;
            };
            let target = self.thread_target();
            let machine = self.files_machine();
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
                        let same = this.runtime.as_deref() == Some(session.as_str());
                        this.cmd(Cmd::Queue {
                            session,
                            text: with_attachments(&text, &prep.lines),
                            context,
                        });
                        if same {
                            this.anchor_draft_inserts(after, cx);
                        }
                    }
                    Err(e) => {
                        this.attachments = keep;
                        this.toast(
                            NotificationType::Error,
                            format!("Attach: {e:#}"),
                            window,
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
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.note_draft(cx);
        self.attachments.clear();
        let shown: Vec<String> = files
            .iter()
            .map(|f| format!("{}{}]", attach::PREFIX, f.display()))
            .collect();
        self.thread.sent(with_attachments(&text, &shown));
        // Inserts put while this message was in the composer sit after it, not before.
        let after = self.thread.messages();
        self.anchor_draft_inserts(after, cx);
        self.follow = true;
        self.speed.begin_answer();
        self.speed.request();
        self.live = Some(Live {
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
        let Some(conn) = self.thread_conn() else {
            return;
        };
        let target = self.thread_target();
        let machine = self.files_machine();
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
                    if let Some(live) = this.live.as_mut() {
                        live.label = "Sending…".into();
                    }
                    let full = with_attachments(&text, &prep.lines);
                    this.submit(profile, full, window, cx);
                }
                Err(e) => {
                    this.live = None;
                    // His files come back: the message stays his to send again.
                    this.attachments = keep;
                    this.toast(
                        NotificationType::Error,
                        format!("Attach: {e:#}"),
                        window,
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    /// The message (with its files named in the text) to the session, or to a new one.
    pub(crate) fn submit(
        &mut self,
        profile: String,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.runtime.clone() {
            Some(session) => match self.parse_command(&text) {
                Some((name, arg)) => self.cmd(Cmd::Dispatch {
                    profile,
                    session,
                    name,
                    arg,
                }),
                None => {
                    let effort = self.take_effort();
                    let context = self.block_context(cx);
                    self.cmd(Cmd::Send {
                        profile,
                        session,
                        text,
                        effort,
                        context,
                    });
                }
            },
            None => {
                // A new session is created on the first message; the effort goes with it.
                self.effort_pending = true;
                self.queued = Some(text.clone());
                let group = self.group_name.read(cx).value().trim().to_string();
                if self.new_project.is_none() && !group.is_empty() {
                    // A named group: its folder first (made where the gateway lives), then the chat.
                    self.group_name
                        .update(cx, |s, cx| s.set_value("", window, cx));
                    let target = self.target.clone();
                    self.background(
                        window,
                        cx,
                        move || connection::group_dir(&target, &group),
                        move |this, r, window, cx| match r {
                            Ok(dir) => this.cmd(this.create_cmd(profile, Some(dir))),
                            Err(e) => {
                                this.toast(
                                    NotificationType::Error,
                                    format!("Couldn't make the group's folder: {e}"),
                                    window,
                                    cx,
                                );
                                this.cmd(this.create_cmd(profile, None));
                            }
                        },
                    );
                    cx.notify();
                    return;
                }
                self.cmd(self.create_cmd(profile, self.new_project.clone()));
            }
        }
        cx.notify();
    }

    /// Ask the last question again: the gateway drops the last exchange, then the same text is sent.
    pub fn regenerate(&mut self, cx: &mut Context<Self>) {
        let (Some(profile), Some(session)) = (self.profile.clone(), self.runtime.clone()) else {
            return;
        };
        if self.thread.running() {
            return;
        }
        let Some(text) = self.tv.rows.iter().rev().find_map(|r| match r {
            ::thread::Row::User { text, .. } => Some(text.clone()),
            _ => None,
        }) else {
            return;
        };
        self.live = Some(Live {
            label: "Regenerating…".into(),
        });
        self.follow = true;
        self.cmd(Cmd::Regenerate {
            profile,
            session,
            text,
        });
        cx.notify();
    }

    /// Copy what the agent said in the row `key`.
    pub fn copy_answer(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.answer_text(key) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.toast(NotificationType::Success, "Copied", window, cx);
        }
    }

    pub(crate) fn answer_text(&self, key: &str) -> Option<String> {
        self.tv.rows.iter().find_map(|r| match r {
            ::thread::Row::Answer { key: k, text, .. } if k == key => Some(text.clone()),
            _ => None,
        })
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let (Some(profile), Some(session)) = (self.profile.clone(), self.runtime.clone()) {
            self.cmd(Cmd::Interrupt { profile, session });
        }
        cx.notify();
    }

    pub fn answer_approval(&mut self, choice: String, cx: &mut Context<Self>) {
        let (Some(req), Some(profile)) = (self.approval.take(), self.profile.clone()) else {
            return;
        };
        if let Some(live) = self.live.as_mut() {
            live.label = "Working…".into();
        }
        self.cmd(Cmd::Approve {
            profile,
            req,
            choice,
        });
        cx.notify();
    }

    /// The composer's text is the open chat's draft (called on every change).
    pub(crate) fn note_draft(&mut self, cx: &mut Context<Self>) {
        // A new chat keeps no draft: every new chat starts empty.
        let Some(key) = self.draft_key.clone().filter(|k| !k.is_empty()) else {
            return;
        };
        let text = self.composer.read(cx).value().to_string();
        let changed = if text.trim().is_empty() {
            self.drafts.remove(&key).is_some()
        } else {
            self.drafts.insert(key, text.clone()).as_deref() != Some(text.as_str())
        };
        if changed {
            save_drafts(&self.drafts);
        }
    }

    /// Another chat is open (or the new one): the composer shows its draft.
    pub(crate) fn sync_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.session.clone().unwrap_or_default();
        if self.draft_key.as_ref() == Some(&key) {
            return;
        }
        // The new chat just became this thread (an insert, a message): what is typed stays.
        let became = self.draft_key.as_deref() == Some("")
            && !key.is_empty()
            && !self.drafts.contains_key(&key);
        let text = if became {
            self.composer.read(cx).value().to_string()
        } else if key.is_empty() {
            String::new()
        } else {
            // A lone "/word" is a command half picked, not a draft.
            self.drafts
                .get(&key)
                .cloned()
                .filter(|t| !(t.starts_with('/') && !t.trim().contains(' ')))
                .unwrap_or_default()
        };
        self.draft_key = Some(key);
        if became {
            self.note_draft(cx);
            return;
        }
        self.composer
            .update(cx, |s, cx| s.set_value(text, window, cx));
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

pub(crate) fn drafts_path() -> std::path::PathBuf {
    crate::connection::state_dir().join("drafts.json")
}

pub(crate) fn load_drafts() -> std::collections::HashMap<String, String> {
    std::fs::read(drafts_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub(crate) fn save_drafts(drafts: &std::collections::HashMap<String, String>) {
    let _ = std::fs::create_dir_all(crate::connection::state_dir());
    let _ = std::fs::write(
        drafts_path(),
        serde_json::to_vec(drafts).unwrap_or_default(),
    );
}
