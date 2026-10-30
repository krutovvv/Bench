//! Sending: the composer's message, a new chat made into a thread, a slash command, copy, an
//! answer to what the run asks (the view's and the app's); the reasoning level and Stop (the
//! app's: the window's controls act on the open thread). The queue is `queued.rs`, drafts
//! `drafts.rs`.

use std::sync::Arc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use protocol::ThreadId;

use crate::thread::{ThreadRef, ThreadView};
use crate::app::model::{ProjectKey, Target};
use crate::app::{BenchApp, Live};
use crate::client::connection::Connection;
use crate::media::attach;

impl BenchApp {
    /// The reasoning levels offered anew (the presets'): the composer's choice follows, the
    /// level kept where it is still offered.
    pub fn set_efforts(
        &mut self,
        levels: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let current = self.defaults.efforts.get(self.thread(cx).effort).cloned();
        let default = self.defaults.efforts.get(self.defaults.effort).cloned();
        self.defaults.efforts = levels;
        let last = self.defaults.efforts.len().saturating_sub(1);
        let at = |v: Option<String>| v.and_then(|v| self.defaults.efforts.iter().position(|e| *e == v));
        let selected = at(current).unwrap_or(last);
        self.defaults.effort = at(default).unwrap_or(last);
        self.active_thread
            .update(cx, |v, _| v.thread.effort = selected);
    }

    /// Send's Stop, ⌘.: the open thread's run.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        let at = self.thread_ref(cx);
        if let Some(thread) = at.id.clone() {
            self.send_at(&at, protocol::StopThread { thread }, cx);
        }
        cx.notify();
    }

    /// A new chat becomes a thread of its agent, in its project (in the folder chosen for it, or
    /// the project's); what waited for it — its first message, its inserts — goes to it then.
    pub(crate) fn create_thread(&mut self, at: &ThreadRef, cx: &mut Context<Self>) {
        if self.threads.creating {
            return;
        }
        self.threads.creating = true;
        let host = self.thread_host(at);
        let key = ProjectKey::parse(&at.project);
        let project = (!key.is_host() && !key.id.is_empty()).then(|| key.id.clone());
        let request = protocol::CreateThread {
            project,
            cwd: self.new_project.folder.clone(),
        };
        let at = at.clone();
        self.request(&host.clone(), request, cx, move |this, r, window, cx| {
            this.threads.creating = false;
            match r {
                Ok(made) => this.thread_created(host, made.thread, &at, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("The thread wasn't created: {e:#}"),
                    window,
                    cx,
                ),
            }
        });
    }

    /// The new chat is a thread now: it shows in the sidebar at once, and what waited for it goes.
    fn thread_created(&mut self, host: String, id: ThreadId, at: &ThreadRef, cx: &mut Context<Self>) {
        if !self.threads.list.iter().any(|s| s.host == host && s.id == id) {
            let row = crate::thread::store::ThreadRow {
                id: id.clone(),
                title: "Untitled".into(),
                running: true,
                source: "local".into(),
                project: at.project.clone(),
                host: host.clone(),
                ..Default::default()
            };
            self.threads.list.insert(0, row);
        }
        let efforts = self.defaults.efforts.clone();
        let (queued, effort, made) = self.active_thread.update(cx, |v, cx| {
            v.thread.host = host.clone();
            v.thread.id = Some(id.clone());
            v.thread.loaded = true;
            let effort = v.thread.take_effort(&efforts);
            cx.notify();
            (v.thread.queued.take(), effort, v.thread.thread_ref())
        });
        self.inserts_created(&made, cx);
        self.save_place(cx);
        if let Some((request, text)) = queued {
            let context = self.inserts_context(&made, cx);
            self.say(&made, text, Some(request), context, effort, cx);
        }
    }

    /// The person's words to a thread that exists: a slash command runs (what it prints is a
    /// toast; what it becomes is sent), anything else is a message.
    pub(crate) fn say(
        &mut self,
        at: &ThreadRef,
        text: String,
        request: Option<String>,
        context: String,
        effort: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = at.id.clone() else {
            return;
        };
        let host = self.thread_host(at);
        if self.parse_command(&text).is_some() {
            let run = protocol::RunCommand {
                thread: thread.clone(),
                command: text,
            };
            self.request(&host.clone(), run, cx, move |this, r, window, cx| match r {
                Ok(protocol::CommandResult::Send(message)) => {
                    let at = ThreadRef {
                        host: host.clone(),
                        id: Some(thread),
                        ..Default::default()
                    };
                    this.say(&at, message, None, String::new(), None, cx);
                }
                Ok(protocol::CommandResult::Output(out)) => {
                    // A command that printed something: the run it would have been is not.
                    for view in this.views_of(&host, &thread, cx) {
                        view.update(cx, |v, _| v.thread.live = None);
                    }
                    if !out.trim().is_empty() {
                        this.toast(NotificationType::Info, bench_ui::clip(&out, 300), window, cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            });
            return;
        }
        let message = protocol::SendMessage {
            thread,
            text,
            context,
            request,
            queue: false,
            effort,
        };
        self.request(&host, message, cx, |this, r, window, cx| {
            if let Err(e) = r {
                // A send that failed: no run started, and the message never reached the journal.
                for view in this.thread_views() {
                    view.update(cx, |v, cx| {
                        if v.thread.live.is_some() && !v.thread.journal.running() {
                            v.thread.live = None;
                            v.thread.journal.clear_sent();
                            cx.notify();
                        }
                    });
                }
                this.toast(NotificationType::Error, format!("{e:#}"), window, cx);
            }
        });
    }
}

impl ThreadView {
    /// The work row's hold: stand the run at its next step boundary, or let it go on.
    pub fn set_paused(&mut self, paused: bool, cx: &mut Context<Self>) {
        if let Some(thread) = self.thread.id.clone() {
            self.tell(protocol::PauseThread { thread, paused }, cx);
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
            a.media.recorder.is_some() || a.media.transcribing
        };
        if dictating {
            app.update(cx, |app, cx| {
                app.media.send_after_transcribe = true;
                if app.media.recorder.is_some() {
                    app.toggle_mic(window, cx);
                }
                cx.notify();
            });
            return;
        }
        let typed = self.composer.read(cx).value().trim().to_string();
        // The code sent from the Explorer goes under the words.
        let typed = crate::thread::composer::attachments::with_references(typed, &self.thread_references(cx));
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
        crate::media::sound::Sound::MessageSent.play();
        // While the agent works, the message waits in the thread's queue (Pi's inbox) until the
        // answer is ready — with its files too: they are staged first (copied aside, uploaded
        // over SSH), then the same queue. The old refusal ("files go with the next message")
        // was ours, not Pi's: the inbox takes a follow-up with attachments like any other.
        if self.thread.live.is_some() {
            let Some(thread) = self.thread.id.clone() else {
                return;
            };
            let text = typed;
            self.take_composer(window, cx);
            let context = self.block_context(cx);
            // Bench's id for the message: its row shows now and is the queue's row later.
            let request = uuid::Uuid::new_v4().to_string();
            // It becomes the thread's next message: the inserts that waited with it go after it.
            let after = self.thread.journal.messages() + 1;
            if files.is_empty() {
                // The waiting row shows the frame it was pressed, not the frame the agent's
                // queue answers with: the composer must never empty onto nothing.
                self.thread
                    .journal
                    .queued_now(request.clone(), text.clone());
                self.follow = true;
                self.tell(
                    protocol::SendMessage {
                        thread,
                        text,
                        context,
                        request: Some(request),
                        queue: true,
                        effort: None,
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
            let keep = files.clone();
            cx.spawn_in(window, async move |this, cx| {
                let prepared = cx
                    .background_spawn(async move {
                        attach::prepare(&conn, &target, machine.as_deref(), &files)
                    })
                    .await;
                let _ = this.update_in(cx, |this, window, cx| match prepared {
                    Ok(prep) => {
                        // The thread it was sent from, even if another is open by now.
                        let same = this.thread.id.as_ref() == Some(&thread);
                        this.tell(
                            protocol::SendMessage {
                                thread,
                                text: with_attachments(&text, &prep.lines),
                                context,
                                request: Some(request),
                                queue: true,
                                effort: None,
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
        // Bench's id for the message: the row shown now is the journal's entry later, whatever
        // words the upload gives it.
        let request = uuid::Uuid::new_v4().to_string();
        self.thread.journal.sent(request.clone(), shown);
        // Inserts put while this message was in the composer sit after it, not before.
        let after = self.thread.journal.messages();
        self.anchor_draft_inserts(after, cx);
        self.follow = true;
        self.thread.speed.begin_answer();
        self.thread.speed.request();
        self.thread.live = Some(Live {
            label: if files.is_empty() {
                "Sending…"
            } else {
                "Uploading…"
            }
            .into(),
        });
        if files.is_empty() {
            self.submit(text, request, cx);
            return;
        }
        let Some((conn, target, machine)) = self.files_where(cx) else {
            return;
        };
        let keep = files.clone();
        cx.spawn_in(window, async move |this, cx| {
            let prepared = cx
                .background_spawn(async move {
                    attach::prepare(&conn, &target, machine.as_deref(), &files)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match prepared {
                Ok(prep) => {
                    if let Some(live) = this.thread.live.as_mut() {
                        live.label = "Sending…".into();
                    }
                    // Shown with the files where they were; sent with where they are now.
                    let full = with_attachments(&text, &prep.lines);
                    this.submit(full, request, cx);
                }
                Err(e) => {
                    this.thread.live = None;
                    this.thread.journal.unsent(&request);
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
            .update(cx, |app, _| app.threads.references.remove(&key))
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

    /// The message (with its files named in the text) to the thread, or — a new chat — to the
    /// thread made for it now.
    pub(crate) fn submit(&mut self, text: String, request: String, cx: &mut Context<Self>) {
        let at = self.thread_ref();
        if at.id.is_some() {
            let efforts = self.app(cx).map(|a| a.defaults.efforts.clone()).unwrap_or_default();
            let effort = self.thread.take_effort(&efforts);
            let context = self.block_context(cx);
            self.app
                .update(cx, |app, cx| app.say(&at, text, Some(request), context, effort, cx))
                .ok();
        } else {
            // The thread is made on the first message; the effort goes with it.
            self.thread.effort_pending = true;
            self.thread.queued = Some((request, text));
            self.app
                .update(cx, |app, cx| app.create_thread(&at, cx))
                .ok();
        }
        cx.notify();
    }

    /// A new thread that goes on from the answer in the row `key`: Pi forks the conversation
    /// through that answer's entry (the key's first part is the entry id), and it opens.
    pub fn fork_at(&mut self, key: &str, cx: &mut Context<Self>) {
        let (Some(thread), Some(entry)) = (
            self.thread.id.clone(),
            key.split('.').next().and_then(|s| s.parse::<u64>().ok()),
        ) else {
            return;
        };
        let host = self.thread.host.clone();
        self.app
            .update(cx, |app, cx| {
                let fork = protocol::ForkThread { thread, at: entry };
                app.request(&host.clone(), fork, cx, move |app, r, window, cx| match r {
                    // The fork opens where the person is: it is the thread they go on in.
                    Ok(made) => {
                        app.list_threads(&host, cx);
                        app.open_thread(host, made.thread, cx);
                    }
                    Err(e) => app.toast(NotificationType::Error, format!("Fork: {e:#}"), window, cx),
                });
            })
            .ok();
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

    /// The person's answer to what the run asks (`ask`: its id, the waiting call's).
    pub fn answer_ask(&mut self, ask: String, answer: String, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.id.clone() else {
            return;
        };
        if let Some(live) = self.thread.live.as_mut() {
            live.label = "Working…".into();
        }
        self.tell(protocol::AnswerAsk { thread, ask, answer }, cx);
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
