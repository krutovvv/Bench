//! Inserts as the thread draws them (the model: `crate::app::inserts`). Every kind is one
//! `InsertKind` in `KINDS`: what it shows, what its title switches between, its controls, and
//! what it tells the agent. The frame is the same for all, as Delta draws a terminal: a tinted
//! ground, the title on top (a menu when there is something to switch to), the kind's controls
//! and "remove" on the right, a click on the chevron folds it. It sits in the person's row (their
//! avatar): it is part of what they say.
//!
//! A new kind: a file here with a unit struct implementing `InsertKind`, a line in `KINDS`, its
//! word in `words()`, and a line in the agent's `inserted()` prompt (agent/src/prompt.ts).

mod browser;
mod files;
mod tasks;
mod terminal;
mod tools;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use serde_json::Value;

use crate::app::BenchApp;
use crate::icons::Ico;
use crate::model::{Insert, SlashCommand};

/// What the title's menu offers: show something else (a patch of the insert: its key, its
/// state), or start something (a form for a new server…).
pub struct Switch {
    pub label: String,
    pub detail: String,
    pub chosen: bool,
    pub to: SwitchTo,
}

pub enum SwitchTo {
    Patch(Value),
    Run(fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)),
}

pub trait InsertKind: Sync {
    /// Whether `key` is this kind's.
    fn owns(&self, key: &str) -> bool;
    /// Whether an insert already in the thread (`there`) is the one `new` would put in: then it
    /// takes the new state instead of a second one coming. Default: one per thread.
    fn same(&self, there: &str, new: &str) -> bool {
        self.owns(there) && self.owns(new)
    }
    fn title(&self, app: &BenchApp, ins: &Insert) -> String;
    /// The title's menu.
    fn switches(&self, _app: &BenchApp, _ins: &Insert) -> Vec<Switch> {
        vec![]
    }
    /// The head's own controls, left of "remove".
    fn controls(&self, _app: &mut BenchApp, _ins: &Insert, _window: &mut Window, _cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        vec![]
    }
    /// What it shows, under the head; None: the head alone.
    fn body(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement>;
    /// What it shows now, for the agent (sent with each message, never shown).
    fn context(&self, app: &BenchApp, ins: &Insert, cx: &App) -> String;
    /// Put in, or brought back: start what it needs (a terminal, a listing).
    fn opened(&self, _app: &mut BenchApp, _ins: &Insert, _window: &mut Window, _cx: &mut Context<BenchApp>) {}
    fn removed(&self, _app: &mut BenchApp, _ins: &Insert, _cx: &mut Context<BenchApp>) {}
}

static KINDS: &[&dyn InsertKind] = &[&terminal::Terminal, &files::Files, &tasks::Tasks, &tools::Tools, &browser::Browser];

pub fn kind_of(key: &str) -> Option<&'static dyn InsertKind> {
    KINDS.iter().copied().find(|k| k.owns(key))
}

/// The words of the bare "/" list: each puts its insert in at once (the insert itself offers
/// what to show: a server, a host, a list, a tool).
pub fn words() -> Vec<SlashCommand> {
    [
        ("/terminal", "This Mac's terminal", "ssh:local"),
        ("/ssh", "A server's terminal", "ssh"),
        ("/ftp", "A file host: folders and files, edit them", "files"),
        ("/tasks", "The task lists", "tasks"),
        ("/tools", "Everyday utilities: Base64, JSON, regex, cron…", "tools"),
        ("/browser", "The server's browser: its tab, live", "browser"),
    ]
    .into_iter()
    .map(|(n, d, k)| SlashCommand { name: n.into(), desc: d.into(), category: "Insert".into(), key: k.into() })
    .collect()
}

impl BenchApp {
    /// The inserts put after `after` of the person's messages, each in the person's row.
    pub(super) fn render_inserts_at(&mut self, after: usize, last: bool, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.thread_inserts()
            .into_iter()
            // Those past the end (a reply not here yet) show at the end.
            .filter(|i| i.after == after || (last && i.after > after))
            .filter_map(|ins| {
                let kind = kind_of(&ins.key)?;
                Some(self.render_insert(kind, &ins, window, cx))
            })
            .collect()
    }

    fn render_insert(&mut self, kind: &'static dyn InsertKind, ins: &Insert, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let title = kind.title(self, ins);
        let switches = kind.switches(self, ins);
        let controls = kind.controls(self, ins, window, cx);
        let body = if ins.folded { None } else { kind.body(self, ins, window, cx) };
        let id = SharedString::from(ins.id.clone());
        let (fold_id, rm_id) = (ins.id.clone(), ins.id.clone());
        let folded = ins.folded;
        let fold = div()
            .id(SharedString::from(format!("{id}-fold")))
            .p_0p5()
            .cursor_pointer()
            .text_color(muted)
            .hover(|s| s.text_color(theme.foreground))
            .child(if folded { Ico::ChevronRight } else { Ico::ChevronDown }.icon().xsmall())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.click_claimed = true;
                this.patch_insert(&fold_id, serde_json::json!({ "folded": !folded }), cx);
            }));
        let title_el: AnyElement = if switches.is_empty() {
            div().px_1().italic().child(title).into_any_element()
        } else {
            let view = cx.entity();
            let ins_id = ins.id.clone();
            let items: Vec<(String, String, bool, std::rc::Rc<SwitchTo>)> =
                switches.into_iter().map(|s| (s.label, s.detail, s.chosen, std::rc::Rc::new(s.to))).collect();
            Button::new(SharedString::from(format!("{id}-switch")))
                .ghost()
                .small()
                .label(format!("{title}  ▾"))
                .dropdown_menu(move |mut menu, _, _| {
                    for (label, detail, chosen, to) in items.iter().cloned() {
                        let (view, ins_id) = (view.clone(), ins_id.clone());
                        let text = if detail.is_empty() { label } else { format!("{label}  ·  {detail}") };
                        menu = menu.item(PopupMenuItem::new(text).checked(chosen).on_click(move |_, window, cx| {
                            let to = to.clone();
                            let ins_id = ins_id.clone();
                            view.update(cx, |this, cx| match &*to {
                                SwitchTo::Patch(p) => {
                                    this.patch_insert(&ins_id, p.clone(), cx);
                                    if let Some(ins) = this.thread_inserts().into_iter().find(|i| i.id == ins_id) {
                                        if let Some(k) = kind_of(&ins.key) {
                                            k.opened(this, &ins, window, cx);
                                        }
                                    }
                                }
                                SwitchTo::Run(f) => f(this, window, cx),
                            })
                        }));
                    }
                    menu
                })
                .into_any_element()
        };
        let head = super::block_head(&theme, body.is_some())
            .gap_1()
            .child(fold)
            .child(title_el)
            .child(div().flex_1())
            .children(controls)
            .child(icon_button(&format!("{id}-rm"), Ico::X, "Remove from the thread", &theme).on_click(cx.listener(move |this, _, _, cx| {
                this.click_claimed = true;
                this.remove_insert(&rm_id, cx);
            })));
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
            .pl(px(super::CHAT_LEFT))
            .pr(px(super::CHAT_RIGHT))
            .py(px(super::TURN_Y))
            .gap(px(super::CHAT_TEXT - super::CHAT_LEFT - super::AVATAR))
            .items_start()
            .child(super::avatar(false, &theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    // Working in an insert keeps the focus there, and doesn't pull the thread
                    // to its end when it grows.
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, _| {
                        this.click_claimed = true;
                        this.follow = false;
                    }))
                    .child(frame),
            )
            .into_any_element()
    }
}

/// A small icon in an insert's head.
pub(super) fn icon_button(id: &str, ico: Ico, tip: &'static str, theme: &gpui_kit::component::Theme) -> Stateful<Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .p_0p5()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|s| s.text_color(theme.foreground).bg(theme.list_hover))
        .child(ico.icon().small())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// A row of a picker inside an insert (a server, a host): a click picks it.
pub(super) fn pick_row(
    id: String,
    ico: Ico,
    label: String,
    detail: String,
    cx: &mut Context<BenchApp>,
    on: impl Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>) + 'static,
) -> AnyElement {
    let theme = cx.theme().clone();
    h_flex()
        .id(SharedString::from(id))
        .px_2()
        .py_1()
        .gap_2()
        .items_center()
        .text_base()
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(ico.icon().small().text_color(theme.muted_foreground))
        .child(div().flex_none().child(label))
        .child(div().flex_1().min_w_0().truncate().text_sm().text_color(theme.muted_foreground).child(detail))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.click_claimed = true;
            on(this, window, cx)
        }))
        .into_any_element()
}

/// Shorten text for the agent.
pub(super) fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}… [cut]", s.chars().take(n).collect::<String>())
    }
}
