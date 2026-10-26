//! `Section`: one part of a project's environment (its soul, its MCP servers, …) — its rows in
//! the environment's blocks, the card under them, and how that card saves, reverts and removes.
//! The drivers in `mod.rs` and `view.rs` call it through `SECTIONS`; the pieces below are what
//! every section's rows and card are made of.

use gpui_kit::component::input::EditorState;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::ProjectEnv;
use crate::app::BenchApp;

/// A save's REST call: method, path, JSON body.
pub type Request = (&'static str, String, Value);

/// A section's card, as its `card` gives it: the head's title and actions (Close is added), the
/// body, and the cards that follow it (a skill's open file).
pub struct Card {
    pub title: String,
    pub body: AnyElement,
    pub actions: Vec<AnyElement>,
    pub follow: Vec<AnyElement>,
}

impl Card {
    pub fn new(title: impl Into<String>, body: AnyElement, actions: Vec<AnyElement>) -> Self {
        Self {
            title: title.into(),
            body,
            actions,
            follow: vec![],
        }
    }
}

pub trait Section: Sync {
    /// Its card's kind, and what `env_save` / `env_revert` / `env_remove` are called with:
    /// "name", "soul", "telegram", "model", "mcp", "skill", "prompt", "cron", "host".
    fn key(&self) -> &'static str;
    /// The block its rows sit in (sections next to each other in `SECTIONS` may share one).
    fn block(&self, general: bool) -> &'static str;
    /// Whether it is shown for this project.
    fn shown(&self, _app: &BenchApp) -> bool {
        true
    }
    /// What it adds to its block's head after the title (one section of a block has it).
    fn head(&self, _app: &BenchApp, _cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        None
    }
    /// Its rows in the block.
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement>;
    /// The card of the row `name` (None: nothing to show).
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card>;
    /// Its part of the agent's answer (`/api/projects/<id>/env`); fields being typed in keep
    /// their text.
    fn apply(&self, _e: &mut ProjectEnv, _v: &Value, _window: &mut Window, _cx: &mut App) {}
    /// Another card opens: what it has open shuts.
    fn close(&self, _e: &mut ProjectEnv) {}
    /// Its row's card opens: fill it.
    fn open(
        &self,
        _app: &mut BenchApp,
        _name: String,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) {
    }
    /// "+ Add": marks a new one open and gives its editor and the text it starts with (None: it
    /// opens as any other card).
    fn new_one(&self, _e: &mut ProjectEnv) -> Option<(Entity<EditorState>, &'static str)> {
        None
    }
    /// Its Save's call; None: nothing to save (it may say why in a toast). `id` is the project,
    /// `eid` the project as its agent knows it.
    fn save(
        &self,
        _app: &mut BenchApp,
        _id: &str,
        _eid: &str,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        None
    }
    /// After a save went through: its dirty flag goes, a new one's form closes.
    fn saved(&self, _e: &mut ProjectEnv) {}
    /// After a save, besides the environment: what else to read again.
    fn reload(&self, _app: &mut BenchApp, _window: &mut Window, _cx: &mut Context<BenchApp>) {}
    /// Revert: the saved text back. True when it opened its card again itself; false: the
    /// environment is read again.
    fn revert(
        &self,
        _app: &mut BenchApp,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> bool {
        false
    }
    /// The DELETE path of its row `name` (None: it has no Remove).
    fn remove_path(&self, _eid: &str, _name: &str) -> Option<String> {
        None
    }
}

/// The block of the project's own rows (its name, soul, Telegram).
pub fn project_block(general: bool) -> &'static str {
    if general { "Host" } else { "Project" }
}

/// A quiet text action in a block's head: muted, the foreground on hover.
pub fn head_act(id: SharedString, label: &'static str, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let (muted, fg) = (theme.muted_foreground, theme.foreground);
    div()
        .id(id)
        .flex_none()
        .cursor_pointer()
        .text_color(muted)
        .hover(move |s| s.text_color(fg))
        .child(label)
}

/// "+ Add": a new one's card (paste it, or ask the agent from there); with no `kind`, the request
/// `ask` starts in the composer (accesses are the agent's to add).
pub fn add(
    id: &'static str,
    kind: &'static str,
    ask: &'static str,
    cx: &mut Context<BenchApp>,
) -> Stateful<Div> {
    head_act(id.into(), "+ Add", cx).on_click(cx.listener(move |this, _, window, cx| {
        this.click_claimed = true;
        cx.stop_propagation();
        if kind.is_empty() {
            this.ask_in_composer(ask, window, cx)
        } else {
            this.env_card(kind, "new".into(), window, cx)
        }
    }))
}

/// A row: its name, a one-line note, its actions on hover; lit when its card is open. `mark` is
/// a status sign before the name (MCP servers have one).
#[allow(clippy::too_many_arguments)]
pub fn row_with(
    e: &ProjectEnv,
    kind: &'static str,
    name: String,
    mark: Option<AnyElement>,
    label: String,
    note: String,
    bad: bool,
    first: bool,
    actions: Vec<AnyElement>,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
    let on = e
        .card
        .as_ref()
        .is_some_and(|(k, n)| *k == kind && *n == name);
    let group = SharedString::from(format!("env-g-{kind}-{name}"));
    h_flex()
        .id(SharedString::from(format!("env-{kind}-{name}")))
        .group(group.clone())
        .px_3()
        .py_1p5()
        .gap_2()
        .cursor_pointer()
        .when(!first, |d| {
            d.border_t_1().border_color(theme.border.opacity(0.5))
        })
        .when(on, |d| d.bg(theme.list_active))
        .hover(|s| s.bg(theme.list_hover))
        .children(mark)
        .child(div().flex_none().child(label))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if bad { theme.danger } else { muted })
                .child(note),
        )
        .children(actions.into_iter().map(|a| {
            div()
                .flex_none()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                .child(a)
        }))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.click_claimed = true;
            this.env_card(kind, name.clone(), window, cx)
        }))
        .into_any_element()
}

/// `row_with` with no sign and no actions.
#[allow(clippy::too_many_arguments)]
pub fn row(
    e: &ProjectEnv,
    kind: &'static str,
    name: String,
    label: String,
    note: String,
    bad: bool,
    first: bool,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    row_with(e, kind, name, None, label, note, bad, first, vec![], cx)
}

/// A block with nothing in it: one muted line.
pub fn empty(text: &'static str, cx: &App) -> AnyElement {
    div()
        .px_3()
        .py_2()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

/// A text action in a card's head.
pub fn card_act(id: String, label: String, color: Hsla) -> Stateful<Div> {
    div()
        .id(SharedString::from(id))
        .flex_none()
        .cursor_pointer()
        .text_color(color)
        .hover(|s| s.opacity(0.8))
        .child(label)
}

/// A card's Save: live once something changed.
pub fn save(what: &'static str, dirty: bool, cx: &mut Context<BenchApp>) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .id(SharedString::from(format!("env-card-save-{what}")))
        .flex_none()
        .px_2()
        .rounded_md()
        .when(dirty, |d| {
            d.cursor_pointer()
                .bg(theme.primary)
                .text_color(theme.primary_foreground)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_save(what, window, cx)
                }))
        })
        .when(!dirty, |d| d.text_color(theme.muted_foreground))
        .child("Save")
        .into_any_element()
}

/// A card's Revert, while something changed.
pub fn revert(what: &'static str, dirty: bool, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
    let muted = cx.theme().muted_foreground;
    dirty.then(|| {
        card_act(format!("env-card-revert-{what}"), "Revert".into(), muted)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.click_claimed = true;
                this.env_revert(what, window, cx)
            }))
            .into_any_element()
    })
}

/// A card's Remove: the first click arms it ("Remove?"), the second does it.
pub fn remove(
    e: &ProjectEnv,
    what: &'static str,
    target: String,
    label: &'static str,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let on = e.armed.as_deref() == Some(format!("{what}:{target}").as_str());
    card_act(
        format!("env-card-rm-{what}"),
        if on {
            format!("{label}?")
        } else {
            label.to_string()
        },
        cx.theme().danger,
    )
    .on_click(cx.listener(move |this, _, window, cx| {
        this.click_claimed = true;
        this.env_remove(what, target.clone(), window, cx)
    }))
    .into_any_element()
}

/// A card's body of fields: padded, spaced.
pub fn pad(d: Div) -> AnyElement {
    d.px_3().py_2().gap_2().into_any_element()
}

/// A text: a code editor as tall as its wrapped text (the thread scrolls, not it).
pub fn code(state: &Entity<EditorState>, cx: &App) -> AnyElement {
    let text = state.read(cx).value().to_string();
    let lines: usize = text
        .lines()
        .map(|l| l.chars().count() / 100 + 1)
        .sum::<usize>()
        .max(1);
    div()
        .h(px(crate::ui::editor_height(lines, cx)))
        .child(
            gpui_kit::component::input::Editor::new(state)
                .appearance(false)
                .h_full(),
        )
        .into_any_element()
}

/// A new MCP server's, skill's or prompt's card: its name (when `named`), its text, Save, and
/// "Ask the agent instead" (`ask` starts in the composer).
pub fn new_card(
    e: &ProjectEnv,
    kind: &'static str,
    title: &'static str,
    ask: &'static str,
    editor: &Entity<EditorState>,
    named: bool,
    cx: &mut Context<BenchApp>,
) -> Card {
    let muted = cx.theme().muted_foreground;
    let ask = card_act("env-card-ask".into(), "Ask the agent instead".into(), muted)
        .on_click(cx.listener(move |this, _, window, cx| {
            this.click_claimed = true;
            this.ask_in_composer(ask, window, cx)
        }))
        .into_any_element();
    let actions = vec![ask, save(kind, true, cx)];
    let body = v_flex()
        .when(named, |d| {
            d.child(
                // The ring's clearance under the name, or its 3 px slide under the editor's first
                // line (the ring is drawn outside the box).
                div()
                    .px_3()
                    .pt_2()
                    .pb(px(crate::ui::RING_CLEAR))
                    .child(gpui_kit::component::input::Input::new(&e.new_name)),
            )
        })
        .child(code(editor, cx))
        .into_any_element();
    Card::new(title, body, actions)
}

/// A run's time as read: "today 18:00", "tomorrow 09:00", "Mon 2 Nov 09:00".
pub fn when(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return iso.to_string();
    };
    let t = t.with_timezone(&chrono::Local);
    let today = chrono::Local::now().date_naive();
    match (t.date_naive() - today).num_days() {
        0 => t.format("today %H:%M").to_string(),
        1 => t.format("tomorrow %H:%M").to_string(),
        -1 => t.format("yesterday %H:%M").to_string(),
        _ => t.format("%a %-d %b %H:%M").to_string(),
    }
}
