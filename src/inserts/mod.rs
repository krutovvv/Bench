//! Inserts: the live tools a person puts into a thread with "/". Each kind is a folder here:
//!
//! ```text
//! inserts/<kind>/mod.rs    the kind: a unit struct implementing `InsertKind` (title, body, …)
//! inserts/<kind>/state.rs  what it keeps and does (methods on BenchApp)
//! inserts/<kind>/view.rs   how its body is drawn, when it is more than a few lines
//! ```
//!
//! This file has the trait, `KINDS`, the words of the "/" list and the block every insert sits
//! in (title, its menu, controls, fold, remove). `state.rs` here keeps the thread's inserts in
//! step with the agent (agent/src/inserts.ts).
//!
//! A new kind: a folder like `tools/`, a line in `KINDS`, its word in `words()`, and a line in
//! the agent's `inserted()` prompt (agent/src/prompt.ts).

pub mod browser;
pub mod state;
pub mod tasks;
pub mod terminal;
pub mod tools;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use serde_json::Value;

use crate::app::BenchApp;
use crate::icons::Ico;
use crate::model::{Insert, SlashCommand};

/// What the title's menu offers: show something else (`to`, a patch of the insert: its key, its
/// state).
pub struct Switch {
    pub label: String,
    pub detail: String,
    pub chosen: bool,
    pub to: Value,
}

pub trait InsertKind: Sync {
    /// Whether `key` is this kind's.
    fn owns(&self, key: &str) -> bool;
    /// Kinds whose live state is one for the whole app (the task list, the tool): their
    /// name. Of several such inserts one is live — the last clicked, else the newest in the
    /// thread; the others offer to show it there.
    fn shared(&self) -> Option<&'static str> {
        None
    }
    fn title(&self, app: &BenchApp, ins: &Insert) -> String;
    /// The title's menu.
    fn switches(&self, _app: &BenchApp, _ins: &Insert) -> Vec<Switch> {
        vec![]
    }
    /// The head's own controls, left of "remove".
    fn controls(
        &self,
        _app: &mut BenchApp,
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
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement>;
    /// What it opened, as the next row of the thread, as if the agent brought it (the open file,
    /// the open task): only while the insert is live and unfolded.
    fn follow(
        &self,
        _app: &mut BenchApp,
        _ins: &Insert,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        None
    }
    /// What it shows now, for the agent (sent with each message, never shown).
    fn context(&self, app: &BenchApp, ins: &Insert, cx: &App) -> String;
    /// Put in, or brought back: start what it needs (a terminal, a listing).
    fn opened(
        &self,
        _app: &mut BenchApp,
        _ins: &Insert,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) {
    }
    fn removed(&self, _app: &mut BenchApp, _ins: &Insert, _cx: &mut Context<BenchApp>) {}
}

static KINDS: &[&dyn InsertKind] = &[
    &terminal::Terminal,
    &tasks::Tasks,
    &tools::Tools,
    &browser::Browser,
];

pub fn kind_of(key: &str) -> Option<&'static dyn InsertKind> {
    KINDS.iter().copied().find(|k| k.owns(key))
}

/// The words of the bare "/" list: each puts its insert in at once (the insert itself offers
/// what to show: a list, a tool).
pub fn words() -> Vec<SlashCommand> {
    [
        ("/terminal", "This Mac's terminal", "ssh:local"),
        ("/tasks", "The task lists", "tasks"),
        (
            "/tools",
            "Everyday utilities: Base64, JSON, regex, cron…",
            "tools",
        ),
        ("/browser", "The server's browser: its tab, live", "browser"),
    ]
    .into_iter()
    .map(|(n, d, k)| SlashCommand {
        name: n.into(),
        desc: d.into(),
        category: "Insert".into(),
        key: k.into(),
    })
    .collect()
}

impl BenchApp {
    /// Whether `ins` shows its kind's app-wide state (see `InsertKind::shared`).
    fn insert_is_live(&self, kind: &dyn InsertKind, ins: &Insert) -> bool {
        let Some(name) = kind.shared() else {
            return true;
        };
        let same: Vec<String> = self
            .thread_inserts()
            .into_iter()
            .filter(|i| kind.owns(&i.key))
            .map(|i| i.id)
            .collect();
        match self.insert_live.get(name).filter(|id| same.contains(id)) {
            Some(id) => *id == ins.id,
            None => same.last() == Some(&ins.id),
        }
    }

    /// One insert where the journal puts it: its block, then what it opened.
    pub(crate) fn render_insert_by_id(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(ins) = self.thread_inserts().into_iter().find(|i| i.id == id) else {
            return vec![];
        };
        let Some(kind) = kind_of(&ins.key) else {
            return vec![];
        };
        let mut out = vec![self.render_insert(kind, &ins, window, cx)];
        if !ins.folded && self.insert_is_live(kind, &ins) {
            if let Some(f) = kind.follow(self, &ins, window, cx) {
                out.push(self.agent_row(f, cx));
            }
        }
        out
    }

    /// A block in the agent's row (its avatar), at the thread's column: what an insert opened.
    pub(crate) fn agent_row(&mut self, content: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        h_flex()
            .w_full()
            .pl(px(crate::ui::CHAT_LEFT))
            .pr(px(crate::ui::CHAT_RIGHT))
            .py(px(crate::ui::TURN_Y))
            .gap(px(crate::ui::CHAT_TEXT
                - crate::ui::CHAT_LEFT
                - crate::ui::AVATAR))
            .items_start()
            .child(crate::ui::avatar(true, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
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
    }

    fn render_insert(
        &mut self,
        kind: &'static dyn InsertKind,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let title = kind.title(self, ins);
        let switches = kind.switches(self, ins);
        let live = self.insert_is_live(kind, ins);
        let controls = if live {
            kind.controls(self, ins, window, cx)
        } else {
            vec![]
        };
        let body = if ins.folded {
            None
        } else if live {
            kind.body(self, ins, window, cx)
        } else {
            let (claim_id, name) = (ins.id.clone(), kind.shared().unwrap_or_default());
            Some(
                div()
                    .id(SharedString::from(format!("{}-claim", ins.id)))
                    .text_color(muted)
                    .cursor_pointer()
                    .hover(|s| s.text_color(theme.foreground))
                    .child("Shown in another insert of this thread — click to show it here")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.insert_live.insert(name.to_string(), claim_id.clone());
                        if let Some(ins) =
                            this.thread_inserts().into_iter().find(|i| i.id == claim_id)
                        {
                            if let Some(k) = kind_of(&ins.key) {
                                k.opened(this, &ins, window, cx);
                            }
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        };
        let id = SharedString::from(ins.id.clone());
        let (fold_id, rm_id) = (ins.id.clone(), ins.id.clone());
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
            .on_click(cx.listener(move |this, _, _, cx| {
                this.click_claimed = true;
                this.patch_insert(&fold_id, serde_json::json!({ "folded": !folded }), cx);
            }));
        let title_el: AnyElement = if switches.is_empty() {
            div().px_1().child(title).into_any_element()
        } else {
            let view = cx.entity();
            let ins_id = ins.id.clone();
            let items: Vec<(String, String, bool, Value)> = switches
                .into_iter()
                .map(|s| (s.label, s.detail, s.chosen, s.to))
                .collect();
            Button::new(SharedString::from(format!("{id}-switch")))
                .ghost()
                .small()
                .label(format!("{title}  ▾"))
                .dropdown_menu(move |mut menu, _, _| {
                    for (label, detail, chosen, to) in items.iter().cloned() {
                        let (view, ins_id) = (view.clone(), ins_id.clone());
                        let text = if detail.is_empty() {
                            label
                        } else {
                            format!("{label}  ·  {detail}")
                        };
                        menu = menu.item(PopupMenuItem::new(text).checked(chosen).on_click(
                            move |_, window, cx| {
                                let to = to.clone();
                                let ins_id = ins_id.clone();
                                view.update(cx, |this, cx| {
                                    this.patch_insert(&ins_id, to, cx);
                                    if let Some(ins) =
                                        this.thread_inserts().into_iter().find(|i| i.id == ins_id)
                                    {
                                        if let Some(k) = kind_of(&ins.key) {
                                            k.opened(this, &ins, window, cx);
                                        }
                                    }
                                })
                            },
                        ));
                    }
                    menu
                })
                .into_any_element()
        };
        let head = crate::ui::block_head(&theme, body.is_some())
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.click_claimed = true;
                    this.remove_insert(&rm_id, cx);
                })),
            );
        // Light, as the page: a thin border, round corners, the head on a faint strip.
        let frame = v_flex()
            .id(id)
            .w_full()
            .min_w_0()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            .child(head)
            .children(body.map(|b| div().px_3().py_2().child(b)));
        h_flex()
            .w_full()
            .pl(px(crate::ui::CHAT_LEFT))
            .pr(px(crate::ui::CHAT_RIGHT))
            .py(px(crate::ui::TURN_Y))
            .gap(px(crate::ui::CHAT_TEXT
                - crate::ui::CHAT_LEFT
                - crate::ui::AVATAR))
            .items_start()
            .child(crate::ui::avatar(false, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    // Working in an insert keeps the focus there, and doesn't pull the thread
                    // to its end when it grows.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| {
                            this.click_claimed = true;
                            this.follow = false;
                        }),
                    )
                    .child(frame),
            )
            .into_any_element()
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

/// A block an insert opened (a file, a task): a border, round corners, `block_head` on top with
/// its title and buttons, the content under it at its full height.
pub(crate) fn card(head: Div, body: AnyElement, theme: &gpui_kit::component::Theme) -> AnyElement {
    v_flex()
        .w_full()
        .min_w_0()
        .border_1()
        .rounded_md()
        .border_color(theme.border)
        .overflow_hidden()
        .bg(theme.background)
        .child(head)
        .child(body)
        .into_any_element()
}
