//! Inserts: the live tools a person puts into a thread with "/". Each kind is a folder here:
//!
//! ```text
//! inserts/<kind>/mod.rs    the kind: a unit struct implementing `InsertKind` (title, body, …)
//! inserts/<kind>/state.rs  what it keeps and does (methods on BenchApp: its state is app-wide)
//! inserts/<kind>/view.rs   how its body is drawn, when it is more than a few lines
//! ```
//!
//! This file has the trait, `KINDS`, the words of the "/" list and the block every insert sits
//! in (title, its menu, controls, fold, remove), drawn by the `ThreadView` it is in. `state.rs` here keeps the thread's inserts in
//! step with the agent (agent/src/runtime/inserts.ts).
//!
//! A new kind: a folder like `tools/` whose unit struct implements `InsertKind` (its "/" word
//! and what follows it are the trait's `word` and `choices`), a line in `KINDS`, its app-wide
//! state as a field of `state::InsertsState`, and a line in the agent's `inserted()` prompt
//! (agent/src/prompt.ts).

pub mod browser;
pub mod state;
pub mod slash;
pub mod terminal;
pub mod tools;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::state::Insert;
use crate::thread::composer::slash::SlashCommand;
use crate::thread::{ThreadRef, ThreadView};
use bench_ui::icons::Ico;

/// What the title's menu offers: show something else (`to`, a patch of the insert: its key, its
/// state).
pub struct Switch {
    pub label: String,
    pub detail: String,
    pub chosen: bool,
    pub to: protocol::InsertPatch,
}

/// A kind's "/" word: the word ("terminal", also its `aliases`), what it is, the key it puts in.
pub struct Word {
    pub word: &'static str,
    pub aliases: &'static [&'static str],
    pub desc: &'static str,
    pub key: &'static str,
}

impl Word {
    /// It as a row of the "/" list.
    pub fn command(&self, category: &str) -> SlashCommand {
        SlashCommand {
            name: format!("/{}", self.word),
            desc: self.desc.into(),
            category: category.into(),
            key: self.key.into(),
        }
    }

    pub fn is(&self, word: &str) -> bool {
        self.word == word || self.aliases.contains(&word)
    }
}

/// A kind of insert. Its app-wide state (the terminals' shells, the browser, the task list, the
/// tool) is `BenchApp`'s; the thread it sits in is `at` — any thread on screen, the open one or
/// the one beside it — so every method is told which (`ThreadRef`). Its block is drawn inside that
/// thread's `ThreadView` (`ThreadView::render_insert`); what the kind draws acts on the app.
pub trait InsertKind: Sync {
    /// Whether `key` is this kind's.
    fn owns(&self, key: &str) -> bool;
    /// Its "/" word: the bare "/" list offers it, and it puts the insert in at once.
    fn word(&self) -> Word;
    /// "/word …": what it can show, each a key to put in (a list, a tool); the bare word alone
    /// unless the kind has more.
    fn choices(&self, _app: &BenchApp, _at: &ThreadRef) -> Vec<SlashCommand> {
        vec![self.word().command("Tool")]
    }
    /// Kinds whose live state is one for the whole app (the task list, the tool): their
    /// name. Of several such inserts one is live — the last clicked, else the newest in the
    /// thread; the others offer to show it there.
    fn shared(&self) -> Option<&'static str> {
        None
    }
    fn title(&self, app: &BenchApp, at: &ThreadRef, ins: &Insert) -> String;
    /// The title's menu.
    fn switches(&self, _app: &BenchApp, _at: &ThreadRef, _ins: &Insert) -> Vec<Switch> {
        vec![]
    }
    /// The head's own controls, left of "remove".
    fn controls(
        &self,
        _app: &mut BenchApp,
        _at: &ThreadRef,
        _ins: &Insert,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> Vec<AnyElement> {
        vec![]
    }
    /// What it shows, under the head; None: the head alone.
    fn body(
        &self,
        app: &mut BenchApp,
        at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement>;
    /// What it opened, as the next row of the thread, as if the agent brought it (the open file,
    /// the open task): only while the insert is live and unfolded.
    fn follow(
        &self,
        _app: &mut BenchApp,
        _at: &ThreadRef,
        _ins: &Insert,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        None
    }
    /// What it shows now, for the agent (sent with each message, never shown).
    fn context(&self, app: &BenchApp, at: &ThreadRef, ins: &Insert, cx: &App) -> String;
    /// Put in, or brought back: start what it needs (a terminal, a listing).
    fn opened(
        &self,
        _app: &mut BenchApp,
        _at: &ThreadRef,
        _ins: &Insert,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) {
    }
    fn removed(
        &self,
        _app: &mut BenchApp,
        _at: &ThreadRef,
        _ins: &Insert,
        _cx: &mut Context<BenchApp>,
    ) {
    }
}

static KINDS: &[&dyn InsertKind] = &[
    &terminal::Terminal,
    &tools::Tools,
    &browser::Browser,
];

pub fn kind_of(key: &str) -> Option<&'static dyn InsertKind> {
    KINDS.iter().copied().find(|k| k.owns(key))
}

/// A pane that scrolls inside the thread: the wheel over it moves the pane, and the thread only
/// gets the wheel when the pane cannot go further (the mask's own rule, CSS
/// `overscroll-behavior: auto`). The mask is a sibling of the scrolled element, not its child:
/// a child is painted with the scroll offset applied, so a mask inside the pane would slide off
/// the viewport as the pane scrolls and leave its bottom to the thread.
pub(crate) fn scroll_pane(
    id: impl Into<ElementId>,
    handle: &ScrollHandle,
    body: impl IntoElement,
) -> AnyElement {
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id(id)
                .absolute()
                .inset_0()
                .overflow_y_scroll()
                .track_scroll(handle)
                .child(body),
        )
        .child(gpui_base::ScrollableMask::new(Axis::Vertical, handle))
        .into_any_element()
}

/// The words of the bare "/" list: each puts its insert in at once (the insert itself offers
/// what to show: a list, a tool).
pub fn words() -> Vec<SlashCommand> {
    KINDS.iter().map(|k| k.word().command("Insert")).collect()
}

/// The kind whose "/" word this is.
pub fn kind_of_word(word: &str) -> Option<&'static dyn InsertKind> {
    KINDS.iter().copied().find(|k| k.word().is(word))
}

impl BenchApp {
    /// Whether `ins` shows its kind's app-wide state (see `InsertKind::shared`).
    pub(crate) fn insert_is_live(
        &self,
        at: &ThreadRef,
        kind: &dyn InsertKind,
        ins: &Insert,
    ) -> bool {
        let Some(name) = kind.shared() else {
            return true;
        };
        let same: Vec<String> = self
            .inserts_of(at)
            .into_iter()
            .filter(|i| kind.owns(&i.key))
            .map(|i| i.id)
            .collect();
        match self.inserts.live.get(name).filter(|id| same.contains(id)) {
            Some(id) => *id == ins.id,
            None => same.last() == Some(&ins.id),
        }
    }

    /// `ins` shown in its thread's block (a click on "show it here", a pick in its title's menu).
    fn insert_opened(
        &mut self,
        at: &ThreadRef,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ins) = self.inserts_of(at).into_iter().find(|i| i.id == id) {
            if let Some(k) = kind_of(&ins.key) {
                k.opened(self, at, &ins, window, cx);
            }
        }
        cx.notify();
    }
}

impl ThreadView {
    /// One insert where the journal puts it: its block, then what it opened.
    pub(crate) fn render_insert_by_id(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let at = self.thread_ref();
        let Some(app) = self.app.upgrade() else {
            return vec![];
        };
        let Some(ins) = app
            .read(cx)
            .inserts_of(&at)
            .into_iter()
            .find(|i| i.id == id)
        else {
            return vec![];
        };
        let Some(kind) = kind_of(&ins.key) else {
            return vec![];
        };
        let mut out = vec![self.render_insert(&app, &at, kind, &ins, window, cx)];
        if !ins.folded && app.read(cx).insert_is_live(&at, kind, &ins) {
            if let Some(f) = app.update(cx, |app, cx| kind.follow(app, &at, &ins, window, cx)) {
                out.push(self.agent_row(f, cx));
            }
        }
        out
    }

    /// A block in the agent's row (its avatar), at the thread's column: what an insert opened.
    pub(crate) fn agent_row(&mut self, content: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let body = div()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.claim_click(cx);
                    this.follow = false;
                }),
            )
            .child(content);
        bench_ui::turn_row(Some(true), body, cx).into_any_element()
    }

    /// The block an insert sits in — its head (fold, title and its menu, controls, remove), its
    /// body — drawn here, in this thread; what the kind draws in it is the app's (`InsertKind`).
    fn render_insert(
        &mut self,
        app: &Entity<BenchApp>,
        at: &ThreadRef,
        kind: &'static dyn InsertKind,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let (title, switches, live) = {
            let a = app.read(cx);
            (
                kind.title(a, at, ins),
                kind.switches(a, at, ins),
                a.insert_is_live(at, kind, ins),
            )
        };
        let controls = if live {
            app.update(cx, |a, cx| kind.controls(a, at, ins, window, cx))
        } else {
            vec![]
        };
        let body = if ins.folded {
            None
        } else if live {
            app.update(cx, |a, cx| kind.body(a, at, ins, window, cx))
        } else {
            let (claim_id, name) = (ins.id.clone(), kind.shared().unwrap_or_default());
            let at = at.clone();
            Some(
                div()
                    .id(SharedString::from(format!("{}-claim", ins.id)))
                    .text_color(muted)
                    .cursor_pointer()
                    .hover(|s| s.text_color(theme.foreground))
                    .child("Shown in another insert of this thread — click to show it here")
                    .on_click(self.app_listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.inserts.live.insert(name.to_string(), claim_id.clone());
                        this.insert_opened(&at, &claim_id, window, cx);
                    }))
                    .into_any_element(),
            )
        };
        let id = SharedString::from(ins.id.clone());
        let (fold_id, rm_id) = (ins.id.clone(), ins.id.clone());
        let (fold_at, rm_at) = (at.clone(), at.clone());
        let folded = ins.folded;
        let fold = div()
            .id(SharedString::from(format!("{id}-fold")))
            .p_0p5()
            .cursor_pointer()
            .text_color(muted)
            .hover(|s| s.text_color(theme.foreground))
            .child(
                if folded {
                    Ico::ChevronRight
                } else {
                    Ico::ChevronDown
                }
                .icon()
                .xsmall(),
            )
            .on_click(self.app_listener(move |this, _, _, cx| {
                this.click_claimed = true;
                let patch = protocol::InsertPatch {
                    folded: Some(!folded),
                    ..Default::default()
                };
                this.patch_insert(&fold_at, &fold_id, patch, cx);
            }));
        let title_el: AnyElement = if switches.is_empty() {
            div().px_1().child(title).into_any_element()
        } else {
            let app = self.app.clone();
            let (ins_id, at) = (ins.id.clone(), at.clone());
            let items: Vec<(String, String, bool, protocol::InsertPatch)> = switches
                .into_iter()
                .map(|s| (s.label, s.detail, s.chosen, s.to))
                .collect();
            Button::new(SharedString::from(format!("{id}-switch")))
                .ghost()
                .small()
                .label(format!("{title}  ▾"))
                .dropdown_menu(move |mut menu, _, _| {
                    for (label, detail, chosen, to) in items.iter().cloned() {
                        let (app, ins_id, at) = (app.clone(), ins_id.clone(), at.clone());
                        let text = if detail.is_empty() {
                            label
                        } else {
                            format!("{label}  ·  {detail}")
                        };
                        menu = menu.item(PopupMenuItem::new(text).checked(chosen).on_click(
                            move |_, window, cx| {
                                let to = to.clone();
                                app.update(cx, |this, cx| {
                                    this.patch_insert(&at, &ins_id, to, cx);
                                    this.insert_opened(&at, &ins_id, window, cx);
                                })
                                .ok();
                            },
                        ));
                    }
                    menu
                })
                .into_any_element()
        };
        let head = bench_ui::block_head(&theme, body.is_some())
            .gap_1()
            .child(fold)
            .child(title_el)
            .child(div().flex_1())
            .children(controls)
            .child(
                bench_ui::head_icon_button(
                    &format!("{id}-rm"),
                    Ico::X,
                    "Remove from the thread",
                    cx,
                )
                .on_click(self.app_listener(move |this, _, _, cx| {
                    this.click_claimed = true;
                    this.remove_insert(&rm_at, &rm_id, cx);
                })),
            );
        // Light, as the page: a thin border, round corners, the head on a faint strip.
        let frame = bench_ui::block_frame(cx)
            .id(id)
            .child(head)
            .children(body.map(|b| div().px_3().py_2().child(b)));
        // Working in an insert keeps the focus there, and doesn't pull the thread to its end
        // when it grows.
        let body = div()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.claim_click(cx);
                    this.follow = false;
                }),
            )
            .child(frame);
        bench_ui::turn_row(Some(false), body, cx).into_any_element()
    }
}

/// Shorten text for the agent.
pub(crate) fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}… [cut]", s.chars().take(n).collect::<String>())
    }
}

