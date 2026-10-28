//! `Section`: one part of a project's environment (its name, its MCP servers, …) and how it
//! saves and reverts; each is a `SettingsForm` (fields on the Settings page: what the project is
//! set to) or `ExplorerParts` (parts in the Explorer's tree, opened in the middle: what it
//! contains that is not a file). The drivers in `mod.rs`, `page.rs` and `src/explorer/` reach
//! them through `FORMS` and `PARTS`; the pieces below are what sections are made of.

use gpui_kit::component::input::EditorState;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::ProjectEnv;
use crate::app::BenchApp;
use bench_ui::icons::Ico;

/// A save's REST call: method, path, JSON body.
pub type Request = (&'static str, String, Value);

/// A part of a section shown in the Explorer's tree: its name (what `card` is called with), what
/// the row says, a muted note after it, its icon.
pub struct Item {
    pub name: String,
    pub label: String,
    pub note: String,
    pub icon: (Ico, Hsla),
}

/// A part opened in the middle, as its `card` gives it: the head's title and actions, the body.
pub struct Card {
    pub title: String,
    pub body: AnyElement,
    pub actions: Vec<AnyElement>,
}

impl Card {
    pub fn new(title: impl Into<String>, body: AnyElement, actions: Vec<AnyElement>) -> Self {
        Self {
            title: title.into(),
            body,
            actions,
        }
    }
}

/// What every section is: its key, its part of the project's environment, and how it saves and
/// reverts. Each is also one of the two below — a `SettingsForm` or `ExplorerParts`.
pub trait Section: Sync {
    /// What `env_save`, `env_revert`, `env_remove` and `env_card` are called with: "name",
    /// "place", "telegram", "model", "project", "mcp", "lsp", "cron".
    fn key(&self) -> &'static str;
    /// Whether it is shown for the project of `e`.
    fn shown(&self, _app: &BenchApp, _e: &ProjectEnv) -> bool {
        true
    }
    /// Its part of the agent's answer (`/api/projects/<id>/env`); fields being typed in keep
    /// their text.
    fn apply(&self, _e: &mut ProjectEnv, _v: &Value, _window: &mut Window, _cx: &mut App) {}
    /// Its Save's call for the project `id` (`app.envs[id]`); None: nothing to send (it may say
    /// why in a toast, or have done it itself). `eid` is the project as its agent knows it.
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
    fn reload(
        &self,
        _app: &mut BenchApp,
        _id: &str,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) {
    }
    /// Revert in the project `id`: the saved text back. True when it opened its text again
    /// itself; false: the environment is read again.
    fn revert(
        &self,
        _app: &mut BenchApp,
        _id: &str,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> bool {
        false
    }
}

/// A section of what the project is set to: fields on its Settings page (`page.rs`), under a
/// heading it may share with its neighbours in `FORMS`.
pub trait SettingsForm: Section {
    /// The heading it is drawn under.
    fn block(&self, general: bool) -> &'static str;
    /// Its fields (`settings_field` rows, then `form_actions` once something changed); None:
    /// nothing to show here.
    fn form(
        &self,
        _app: &BenchApp,
        _e: &ProjectEnv,
        _cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        None
    }
    /// Its Remove, done here rather than by a DELETE to the agent (the project itself). True:
    /// done.
    fn remove_here(
        &self,
        _app: &mut BenchApp,
        _id: &str,
        _window: &mut Window,
        _cx: &mut Context<BenchApp>,
    ) -> bool {
        false
    }
}

/// A section of what the project contains that is not a file: parts in the Explorer's tree
/// (`src/explorer/parts.rs`), each opened in the middle as its `card` (`DocKey::Part`).
pub trait ExplorerParts: Section {
    /// The Explorer's group it goes under ("Tools", "Schedule").
    fn group(&self) -> &'static str;
    /// Its node in that group.
    fn block(&self, general: bool) -> &'static str;
    /// What it adds to its node, on the right ("+ Add", "Check").
    fn head(
        &self,
        _app: &BenchApp,
        _e: &ProjectEnv,
        _cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        None
    }
    /// Its parts.
    fn items(&self, _app: &BenchApp, _e: &ProjectEnv, _cx: &mut Context<BenchApp>) -> Vec<Item> {
        vec![]
    }
    /// What the part `name` opens in the middle: its fields, its actions (None: nothing to show).
    fn card(
        &self,
        _app: &BenchApp,
        _e: &ProjectEnv,
        _name: &str,
        _cx: &mut Context<BenchApp>,
    ) -> Option<Card> {
        None
    }
    /// Its open part has changes not saved (its tab's dot; closing it or opening another part
    /// of the project asks first).
    fn dirty(&self, _e: &ProjectEnv) -> bool {
        false
    }
    /// Another card opens, or its own closes: what it has open shuts, its draft with it.
    fn close(&self, _e: &mut ProjectEnv) {}
    /// Its row's card opens in the project `id`: fill it.
    fn open(
        &self,
        _app: &mut BenchApp,
        _id: &str,
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
    /// A new one saved: the name of the part it is now, whose tab takes the new one's form's
    /// place (None: the form stays as it is).
    fn made(&self, _e: &ProjectEnv, _cx: &App) -> Option<String> {
        None
    }
    /// The DELETE path of its part `name` (None: it has no Remove).
    fn remove_path(&self, _eid: &str, _name: &str) -> Option<String> {
        None
    }
}

/// A quiet text action in a heading: muted, the foreground on hover.
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

/// "+ Add": a new one's form for the project `project`, opened in the middle.
pub fn add(
    id: &'static str,
    project: &str,
    kind: &'static str,
    cx: &mut Context<BenchApp>,
) -> Stateful<Div> {
    let project = project.to_string();
    head_act(id.into(), "+ Add", cx).on_click(cx.listener(move |this, _, window, cx| {
        cx.stop_propagation();
        this.open_part(project.clone(), kind, "new".into(), window, cx)
    }))
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

/// A card's Save in the project `project`: live once something changed.
pub fn save(
    project: &str,
    what: &'static str,
    dirty: bool,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let project = project.to_string();
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
                .on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.env_save(&project, what, window, cx)
                    }),
                )
        })
        .when(!dirty, |d| d.text_color(theme.muted_foreground))
        .child("Save")
        .into_any_element()
}

/// A card's Revert in the project `project`, while something changed.
pub fn revert(
    project: &str,
    what: &'static str,
    dirty: bool,
    cx: &mut Context<BenchApp>,
) -> Option<AnyElement> {
    let muted = cx.theme().muted_foreground;
    let project = project.to_string();
    dirty.then(|| {
        card_act(format!("env-card-revert-{what}"), "Revert".into(), muted)
            .on_click(
                cx.listener(move |this, _, window, cx| this.env_revert(&project, what, window, cx)),
            )
            .into_any_element()
    })
}

/// A Remove: the first click arms it ("Remove?"), the second does it.
pub fn remove(
    app: &BenchApp,
    e: &ProjectEnv,
    what: &'static str,
    target: String,
    label: &'static str,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let on = app.armed.is(&format!("env:{}:{what}:{target}", e.id));
    let project = e.id.clone();
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
        this.env_remove(&project, what, target.clone(), window, cx)
    }))
    .into_any_element()
}

/// A card's body of fields: padded, spaced.
pub fn pad(d: Div) -> AnyElement {
    d.px_3().py_2().gap_2().into_any_element()
}

/// A text: a code editor as tall as its wrapped text (the page scrolls, not it).
pub fn code(state: &Entity<EditorState>, cx: &App) -> AnyElement {
    // As tall as the lines it shows: a folded block's lines do not count.
    let s = state.read(cx);
    let text = s.value().to_string();
    let lines: usize = text
        .lines()
        .enumerate()
        .filter(|(k, _)| !s.is_line_folded(*k))
        .map(|(_, l)| l.chars().count() / 100 + 1)
        .sum::<usize>()
        .max(1);
    div()
        .h(px(bench_ui::editor_height(lines, cx)))
        .child(
            gpui_kit::component::input::Editor::new(state)
                .appearance(false)
                .h_full(),
        )
        .into_any_element()
}

/// A new MCP server's or language server's card: its name (when `named`), its text, Save.
pub fn new_card(
    e: &ProjectEnv,
    kind: &'static str,
    title: &'static str,
    editor: &Entity<EditorState>,
    named: bool,
    cx: &mut Context<BenchApp>,
) -> Card {
    let actions = vec![save(&e.id, kind, true, cx)];
    let body = v_flex()
        .when(named, |d| {
            d.child(
                // The ring's clearance under the name, or its 3 px slide under the editor's first
                // line (the ring is drawn outside the box).
                div()
                    .px_3()
                    .pt_2()
                    .pb(px(bench_ui::RING_CLEAR))
                    .child(gpui_kit::component::input::Input::new(&e.new_name)),
            )
        })
        .child(code(editor, cx))
        .into_any_element();
    Card::new(title, body, actions)
}

/// A form's Revert and Save in the project `project`, under its fields, while something in it
/// changed.
pub fn form_actions(
    project: &str,
    what: &'static str,
    dirty: bool,
    cx: &mut Context<BenchApp>,
) -> Option<AnyElement> {
    let (p1, p2) = (project.to_string(), project.to_string());
    dirty.then(|| {
        h_flex()
            .w_full()
            .py_2()
            .gap_2()
            .justify_end()
            .child(
                bench_ui::form_action(
                    SharedString::from(format!("env-form-revert-{what}")),
                    "Revert",
                    false,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.env_revert(&p1, what, window, cx)),
                ),
            )
            .child(
                bench_ui::form_action(
                    SharedString::from(format!("env-form-save-{what}")),
                    "Save",
                    true,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.env_save(&p2, what, window, cx)),
                ),
            )
            .into_any_element()
    })
}

/// A setting that is only read: its text (in the danger color when `bad`).
pub fn text(t: impl Into<SharedString>, bad: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .when(bad, |d| d.text_color(theme.danger))
        .child(t.into())
        .into_any_element()
}

/// Pick one of a few, shown as a list (each a line of its own, the current one checked); a
/// click calls `pick` with its index.
pub fn choices(
    id: &'static str,
    items: Vec<(String, bool)>,
    pick: std::rc::Rc<dyn Fn(&mut BenchApp, usize, &mut Window, &mut Context<BenchApp>)>,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let theme = cx.theme().clone();
    v_flex()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        // A field's text: the choices read as the fields around them.
        .text_sm()
        .children(items.into_iter().enumerate().map(|(k, (label, on))| {
            let pick = pick.clone();
            h_flex()
                .id((id, k))
                .h(px(bench_ui::FIELD_H))
                .px_3()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .when(k > 0, |d| {
                    d.border_t_1().border_color(theme.border.opacity(0.5))
                })
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(div().flex_1().min_w_0().truncate().child(label))
                .when(on, |d| {
                    d.child(Ico::Check.icon().xsmall().text_color(theme.foreground))
                })
                .on_click(cx.listener(move |this, _, window, cx| pick(this, k, window, cx)))
        }))
        .into_any_element()
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
