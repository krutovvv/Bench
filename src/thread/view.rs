//! A thread on screen, as its own entity (Zed's `AcpThreadView`): its state (`ActiveThread`),
//! its composer and the composer's extras, drawn by itself (`mod.rs` and the files beside it). The
//! center shows one (`BenchApp::active_thread`), the right dock a second beside it
//! (`BenchApp::beside_thread`); each has its own composer, run, queue and events.
//!
//! It reaches the window's app through `app`, a weak handle. Two rules keep the two entities from
//! ever being borrowed twice:
//! - what a view asks of the app takes the thread as a `ThreadRef` and never reads a view — the
//!   view asking may be drawing at that moment;
//! - what the app asks of a view never reads the app. When a view's own click needs the app to
//!   come back to it (an insert put, a `!command`), the call is deferred (`window.defer`).

use std::path::PathBuf;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::*;

use super::{ActiveThread, ThreadRef};
use crate::app::BenchApp;
use crate::client::rpc::{Cmd, Evt};

pub struct ThreadView {
    /// The window's app: what is one for the whole window (the connection, the sessions, the
    /// inserts' shared state, media).
    pub app: WeakEntity<BenchApp>,
    /// The thread it shows: its session, journal, list, run, what waits and what it asks.
    pub thread: ActiveThread,
    /// The thread's next message.
    pub composer: Entity<TextareaState>,
    /// The question card's own answer.
    pub clarify_input: Entity<InputState>,
    /// Files waiting in the composer to go with the next message.
    pub attachments: Vec<PathBuf>,
    /// The waiting message whose words the person is fixing, and the field they fix them in.
    pub queue_edit: Option<(String, Entity<TextareaState>)>,
    /// The thread whose draft the composer shows ("" = a new chat): `drafts.rs`.
    pub draft_key: Option<String>,
    /// Inserts put while a message still waited in the composer: they belong after that message,
    /// whose place in the thread is not there yet. Their ids; `send` fixes the place as it sends.
    pub insert_draft: Vec<String>,
    /// The next change of the thread brings the composer into view (a message sent, an insert put).
    pub follow: bool,
    /// The `/` list: its highlighted row; Esc closed it for this text; its scroll.
    pub slash_sel: usize,
    pub slash_closed: bool,
    pub slash_scroll: ScrollHandle,
    /// Where the composer is in the window, and the window's height (last frame): the `/` list
    /// opens above the line when there is no room under it.
    pub composer_y: Pixels,
    /// The composer's measured height, from the same canvas as `composer_y`: how much of the
    /// window the thread's last row has to share with it (`past_end`).
    pub composer_h: f32,
    pub viewport_h: Pixels,
    /// The composer shows a new chat — what render last set.
    pub(super) composer_fresh: bool,
    /// `BENCH_DEMO_DRAFT` was put into the composer; `BENCH_DEMO_SCROLL` was done.
    pub(super) demo_draft_done: bool,
    pub(super) demo_scrolled: bool,
    _subscriptions: Vec<Subscription>,
}

impl ThreadView {
    pub fn new(app: WeakEntity<BenchApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 100_000)
                .submit_on_enter(false)
        });
        let clarify_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Your own answer — Enter to send"));
        let subscriptions = vec![
            cx.subscribe_in(&composer, window, |this, _, ev: &InputEvent, window, cx| {
                match ev {
                    // Enter is a new line; ⌘Enter sends.
                    InputEvent::PressEnter {
                        secondary: true, ..
                    } => this.send(window, cx),
                    // New text: the command list starts over at its first row.
                    InputEvent::Change => {
                        this.note_draft(cx);
                        this.slash_sel = 0;
                        // Esc hides the list for the current word; a new word may open it again.
                        let text = this.composer.read(cx).value();
                        if text.is_empty() || text.ends_with(char::is_whitespace) {
                            this.slash_closed = false;
                        }
                        cx.notify();
                    }
                    _ => {}
                }
            }),
            // Enter in the question card's field sends that text as the answer.
            cx.subscribe_in(
                &clarify_input,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let text = state.read(cx).value().trim().to_string();
                        if !text.is_empty() {
                            this.answer_clarify(text, window, cx);
                        }
                    }
                },
            ),
        ];
        Self {
            app,
            thread: ActiveThread::default(),
            composer,
            clarify_input,
            attachments: vec![],
            queue_edit: None,
            draft_key: None,
            insert_draft: vec![],
            follow: false,
            slash_sel: 0,
            slash_closed: false,
            slash_scroll: ScrollHandle::new(),
            composer_y: px(0.),
            composer_h: 0.,
            viewport_h: px(0.),
            composer_fresh: false,
            demo_draft_done: false,
            demo_scrolled: false,
            _subscriptions: subscriptions,
        }
    }

    /// Which thread this is, for what it asks of the app.
    pub fn thread_ref(&self) -> ThreadRef {
        self.thread.thread_ref()
    }

    /// The window's app, to read. None while the window closes.
    pub(crate) fn app<'a>(&self, cx: &'a App) -> Option<&'a BenchApp> {
        Some(self.app.upgrade()?.read(cx))
    }

    /// The profile the app talks to the agent as.
    pub(crate) fn profile(&self, cx: &App) -> Option<String> {
        self.app(cx)?.profile.clone()
    }

    /// A command of this thread to the agent keeping it.
    pub(crate) fn cmd(&self, cmd: Cmd, cx: &App) {
        if let Some(app) = self.app(cx) {
            app.cmd_at(&self.thread_ref(), cmd);
        }
    }

    /// Whether this is the center's view (the open thread), not the one beside it: what a thread
    /// does to the window (the status bar's speed) is the open thread's only.
    pub(crate) fn is_open_thread(&self, cx: &Context<Self>) -> bool {
        self.app(cx)
            .is_some_and(|a| a.active_thread.entity_id() == cx.entity_id())
    }

    /// A listener that acts on the app, not on this thread (`cx.listener` is this view's): the
    /// window's things — playback, documents, the sidebar.
    pub(crate) fn app_listener<E: ?Sized>(
        &self,
        f: impl Fn(&mut BenchApp, &E, &mut Window, &mut Context<BenchApp>) + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let app = self.app.clone();
        move |e, window, cx| {
            app.update(cx, |app, cx| f(app, e, window, cx)).ok();
        }
    }

    /// Back to a thread not yet shown (another one opened here, or a new chat): what the view
    /// keeps of its last one goes; `effort` is where a new chat starts. The composer's text is
    /// the drafts' (`sync_draft`).
    pub fn reset(&mut self, effort: usize, cx: &mut Context<Self>) {
        let project = std::mem::take(&mut self.thread.thread_project);
        self.thread = ActiveThread {
            effort,
            thread_project: project,
            ..Default::default()
        };
        self.queue_edit = None;
        cx.notify();
    }

    /// An event of this thread: its state, and what the view keeps around it.
    pub fn on_event(&mut self, evt: &Evt, window: &mut Window, cx: &mut Context<Self>) {
        self.thread.on_event(evt);
        match evt {
            Evt::Thread(_, ::thread::ThreadEvent::Queue(items)) => {
                // A message that left the queue (sent, taken back) has nothing left to fix.
                if self
                    .queue_edit
                    .as_ref()
                    .is_some_and(|(id, _)| !items.iter().any(|q| &q.sub == id))
                {
                    self.queue_edit = None;
                }
            }
            // The free answer's field starts empty for a new question.
            Evt::Clarify(_) => {
                self.clarify_input
                    .update(cx, |s, cx| s.set_value("", window, cx));
            }
            _ => {}
        }
        cx.notify();
    }
}

impl Render for ThreadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The thread's unsent text in the composer (`drafts.rs`).
        self.sync_draft(window, cx);
        // A new chat is a code editor's empty file: the caret alone (no avatars yet), focused.
        // The composer never has a placeholder: it is the thread's next text, not a form field.
        let fresh = self.thread.journal.is_empty() && self.thread.session.is_none();
        if fresh != self.composer_fresh {
            self.composer_fresh = fresh;
            if fresh {
                self.composer.update(cx, |s, cx| s.focus(window, cx));
            }
        }
        self.render_chat(window, cx)
    }
}
