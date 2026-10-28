//! The project's language servers (its lsp.json, as Crush's lsp section): after every edit the
//! agent gets the file's errors from them (`agent/src/lsp.ts`). A row per server with a sign of
//! how it is doing — running, not started yet (it starts with the first file it knows), failed
//! and why; its card is its own entry of lsp.json as JSON, as an MCP server's (`mcp.rs`). "+ Add"
//! offers the usual ones (each installs itself on first use, on the machine where the project's
//! files are), or one written by hand.

use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{
    Card, ExplorerParts, Item, Request, Section, add, code, new_card, remove, revert, save,
};
use super::{ProjectEnv, editor, enc, on_change};
use crate::app::BenchApp;
use crate::client::rest;

/// A server of lsp.json as its row says it.
#[derive(Clone, Debug)]
pub struct LspRow {
    pub name: String,
    pub extensions: Vec<String>,
    /// As the agent says it: "running …", "starting …", "failed: why"; empty: not started yet
    /// (no file of its kind touched).
    pub state: String,
    /// Its entry in lsp.json, as JSON text.
    pub spec: String,
}

/// How a server is doing, as its sign shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LspTone {
    Running,
    Starting,
    Failed,
    /// Not started yet: it starts with the first file it knows.
    Idle,
}

pub fn lsp_tone(s: &LspRow) -> LspTone {
    if s.state.starts_with("failed") {
        LspTone::Failed
    } else if s.state.starts_with("running") {
        LspTone::Running
    } else if s.state.starts_with("starting") {
        LspTone::Starting
    } else {
        LspTone::Idle
    }
}

/// Its state in words: the agent's, or what an empty one means.
pub fn lsp_state(s: &LspRow) -> String {
    if s.state.is_empty() {
        "starts with the first file it knows".into()
    } else {
        s.state.clone()
    }
}

/// The note after its name: the files it knows, how it is doing (a failed one: why).
pub fn lsp_note(s: &LspRow) -> String {
    format!("{} · {}", s.extensions.join(", "), lsp_state(s))
}

/// The project's language servers as the agent reads them, and the one open in the middle.
pub struct LspEnv {
    pub servers: Vec<LspRow>,
    /// The server opened, "new" for a new one's form.
    pub open: Option<String>,
    /// Its entry being edited.
    pub json: Entity<EditorState>,
    pub dirty: bool,
}

impl LspEnv {
    pub(super) fn new(id: &str, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let json = editor("json", window, cx);
        on_change(&json, id, cx, |e| e.lsp.dirty = true);
        Self {
            servers: vec![],
            open: None,
            json,
            dirty: false,
        }
    }
}

pub struct Lsp;

impl Section for Lsp {
    fn key(&self) -> &'static str {
        "lsp"
    }

    fn apply(&self, e: &mut ProjectEnv, v: &Value, _window: &mut Window, _cx: &mut App) {
        let l = &v["lsp"];
        let states = &l["states"];
        e.lsp.servers = l["servers"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(name, s)| LspRow {
                        name: name.clone(),
                        extensions: s["extensions"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect(),
                        state: states[name].as_str().unwrap_or_default().to_string(),
                        spec: serde_json::to_string_pretty(s).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        eid: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.envs.get(id)?;
        let mut name = e.lsp.open.clone()?;
        if name == "new" {
            name = e.new_name.read(cx).value().trim().to_string();
            if name.is_empty() {
                app.toast(NotificationType::Error, "Name it first", window, cx);
                return None;
            }
        }
        Some((
            "PUT",
            format!("/api/projects/{eid}/lsp/{}", enc(&name)),
            json!({"text": e.lsp.json.read(cx).value().to_string()}),
        ))
    }

    fn saved(&self, e: &mut ProjectEnv) {
        e.lsp.dirty = false;
    }

    /// Its saved entry back in the editor.
    fn revert(
        &self,
        app: &mut BenchApp,
        id: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> bool {
        let Some(e) = app.envs.get_mut(id) else {
            return false;
        };
        let Some(name) = e.lsp.open.clone().filter(|n| n != "new") else {
            return false;
        };
        app.env_open_lsp(id, name, window, cx);
        true
    }
}

impl ExplorerParts for Lsp {
    fn group(&self) -> &'static str {
        "Tools"
    }

    fn block(&self, _general: bool) -> &'static str {
        "Language servers"
    }

    fn head(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        Some(add("env-lsp-add", &e.id, "lsp", cx).into_any_element())
    }

    /// A row per server with its sign; each opens its own entry.
    fn items(&self, _app: &BenchApp, e: &ProjectEnv, cx: &mut Context<BenchApp>) -> Vec<Item> {
        let theme = cx.theme().clone();
        e.lsp
            .servers
            .iter()
            .map(|s| {
                let icon = match lsp_tone(s) {
                    LspTone::Running => (bench_ui::icons::Ico::CircleCheck, theme.success),
                    LspTone::Failed => (bench_ui::icons::Ico::CircleAlert, theme.danger),
                    LspTone::Starting => (bench_ui::icons::Ico::Loader, theme.muted_foreground),
                    LspTone::Idle => (bench_ui::icons::Ico::CircleEmpty, theme.muted_foreground),
                };
                Item {
                    name: s.name.clone(),
                    label: s.name.clone(),
                    note: lsp_note(s),
                    icon,
                }
            })
            .collect()
    }

    /// "new": the presets to add, or one written by hand; a server: its entry, its state.
    fn card(
        &self,
        app: &BenchApp,
        e: &ProjectEnv,
        name: &str,
        cx: &mut Context<BenchApp>,
    ) -> Option<Card> {
        let theme = cx.theme().clone();
        if name == "new" {
            return Some(new_lsp_card(app, e, cx));
        }
        let s = e.lsp.servers.iter().find(|s| s.name == name)?;
        let bad = lsp_tone(s) == LspTone::Failed;
        let actions = [save(&e.id, "lsp", e.lsp.dirty, cx)]
            .into_iter()
            .chain(revert(&e.id, "lsp", e.lsp.dirty, cx))
            .chain([remove(app, e, "lsp", name.to_string(), "Remove", cx)])
            .collect();
        Some(Card::new(
            format!("Language server · {name}"),
            v_flex()
                .child(code(&e.lsp.json, cx))
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .text_color(if bad {
                            theme.danger
                        } else {
                            theme.muted_foreground
                        })
                        .child(lsp_state(s)),
                )
                .into_any_element(),
            actions,
        ))
    }

    fn dirty(&self, e: &ProjectEnv) -> bool {
        e.lsp.open.is_some() && e.lsp.dirty
    }

    fn close(&self, e: &mut ProjectEnv) {
        e.lsp.open = None;
        e.lsp.dirty = false;
    }

    fn open(
        &self,
        app: &mut BenchApp,
        id: &str,
        name: String,
        w: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.env_open_lsp(id, name, w, cx);
    }

    fn new_one(&self, e: &mut ProjectEnv) -> Option<(Entity<EditorState>, &'static str)> {
        e.lsp.open = Some("new".into());
        Some((
            e.lsp.json.clone(),
            "{\n  \"command\": \"\",\n  \"args\": [\"--stdio\"],\n  \"extensions\": []\n}\n",
        ))
    }

    /// A new server, saved, is its name's part.
    fn made(&self, e: &ProjectEnv, cx: &App) -> Option<String> {
        Some(e.new_name.read(cx).value().trim().to_string())
    }

    fn remove_path(&self, eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/projects/{eid}/lsp/{}", enc(name)))
    }
}

/// "+ Add": the person's presets (the gear's Presets tab) as chips, each added with a click —
/// the same shape the MCP card uses; under them a server written by hand — its name and its entry.
fn new_lsp_card(app: &BenchApp, e: &ProjectEnv, cx: &mut Context<BenchApp>) -> Card {
    let theme = cx.theme().clone();
    let have: Vec<&str> = e.lsp.servers.iter().map(|s| s.name.as_str()).collect();
    let chips: Vec<AnyElement> = app
        .gear.presets
        .lsp
        .iter()
        .filter(|(key, _, _)| !have.contains(&key.as_str()))
        .map(|(key, title, spec)| {
            let exts: Vec<String> = spec["extensions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            let (key, spec, id) = (key.clone(), spec.clone(), e.id.clone());
            div()
                .id(SharedString::from(format!("env-lsp-preset-{key}")))
                .px_2()
                .py_0p5()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    h_flex()
                        .gap_1()
                        .child(title.clone())
                        .when(!exts.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .child(exts.join(", ")),
                            )
                        }),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.env_add_lsp(id.clone(), key.clone(), spec.clone(), window, cx)
                }))
                .into_any_element()
        })
        .collect();
    let mut card = new_card(e, "lsp", "Add a language server", &e.lsp.json, true, cx);
    let form = std::mem::replace(&mut card.body, div().into_any_element());
    card.body = v_flex()
        .when(!chips.is_empty(), |d| {
            d.child(h_flex().flex_wrap().gap_1p5().px_3().pt_2().children(chips))
        })
        .child(
            div()
                .px_3()
                .pt_2()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Each installs itself on first use, into Bench's folder on the machine where the project's files are (~/.bench/lsp): it needs npm there. The list is the gear's Presets; another server for this project alone: its name and its entry below (a command, its args, the extensions it knows), then Save."),
        )
        .child(form)
        .into_any_element();
    card
}

impl BenchApp {
    /// A server's row in the project `id`: its saved entry in the editor.
    pub fn env_open_lsp(
        &mut self,
        id: &str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.envs.get_mut(id) else {
            return;
        };
        let spec = e
            .lsp
            .servers
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.spec.clone())
            .unwrap_or_default();
        e.lsp.open = Some(name);
        e.lsp.dirty = false;
        e.lsp.json.update(cx, |s, cx| s.set_value(spec, window, cx));
        cx.notify();
    }

    /// A preset's server into the project's lsp.json (wherever the project lives), then the
    /// environment read again.
    pub fn env_add_lsp(
        &mut self,
        id: String,
        key: String,
        spec: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "PUT",
                    &format!("/api/projects/{eid}/lsp/{}", enc(&key)),
                    json!({"spec": spec}),
                )
            },
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.envs.get_mut(&id) {
                        e.card = None;
                    }
                    this.load_env(&id, window, cx);
                }
                Err(err) => {
                    if let Some(e) = this.envs.get_mut(&id) {
                        e.error = Some(format!("{err:#}"));
                    }
                    cx.notify();
                }
            },
        );
    }
}
