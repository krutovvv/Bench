//! The project's language servers (its lsp.json, as Crush's lsp section): after every edit the
//! agent gets the file's errors from them (`agent/src/lsp.ts`). A row per server — the files it
//! knows, how it is doing; "+ Add" offers the usual ones (each installs itself on first use, on
//! the machine where the project's files are); any row opens lsp.json to edit or paste from
//! another project.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::EditorState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Card, Request, Section, Tab, add, code, empty, remove, revert, row, save};
use super::{ProjectEnv, editor, on_change};
use crate::app::BenchApp;
use crate::rest;

/// A server of lsp.json as its row says it.
#[derive(Clone, Debug)]
pub struct LspRow {
    pub name: String,
    pub extensions: Vec<String>,
    /// running, starting, failed: why; empty: not started yet (no file of its kind touched).
    pub state: String,
}

pub struct LspEnv {
    pub servers: Vec<LspRow>,
    /// lsp.json, as its card edits it.
    pub text: Entity<EditorState>,
    pub dirty: bool,
}

impl LspEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let text = editor("json", window, cx);
        on_change(&text, cx, |e| e.lsp.dirty = true);
        Self {
            servers: vec![],
            text,
            dirty: false,
        }
    }
}

pub struct Lsp;

impl Section for Lsp {
    fn key(&self) -> &'static str {
        "lsp"
    }
    fn tab(&self) -> Tab {
        Tab::Tools
    }
    fn first(&self, e: &ProjectEnv) -> Option<String> {
        e.lsp.servers.first().map(|s| s.name.clone())
    }
    fn block(&self, _general: bool) -> &'static str {
        "Language servers"
    }
    fn head(&self, _app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        Some(add("env-lsp-add", "lsp", cx).into_any_element())
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        if e.lsp.servers.is_empty() {
            return vec![empty(
                "None: + Add takes the usual ones (TypeScript, PHP and WordPress, CSS, Tailwind…).",
                cx,
            )];
        }
        e.lsp
            .servers
            .iter()
            .map(|s| {
                let state = if s.state.is_empty() {
                    "starts with the first file it knows".to_string()
                } else {
                    s.state.clone()
                };
                let bad = s.state.starts_with("failed");
                row(
                    e,
                    "lsp",
                    s.name.clone(),
                    s.name.clone(),
                    format!("{} · {state}", s.extensions.join(", ")),
                    bad,
                    cx,
                )
            })
            .collect()
    }
    /// "new": the presets to add; any server: lsp.json to edit.
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let theme = cx.theme().clone();
        if name == "new" {
            // The person's presets (the gear's Presets tab).
            let have: Vec<&str> = e.lsp.servers.iter().map(|s| s.name.as_str()).collect();
            let items = app.presets.lsp.iter().map(|(key, title, spec)| {
                let added = have.contains(&key.as_str());
                let exts: Vec<String> = spec["extensions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect();
                let (key, spec) = (key.clone(), spec.clone());
                h_flex()
                    .id(SharedString::from(format!("env-lsp-preset-{key}")))
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .when(!added, |d| {
                        d.cursor_pointer().hover(|s| s.bg(theme.list_hover))
                    })
                    .child(div().flex_none().child(title.clone()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(if added {
                                "added".to_string()
                            } else {
                                exts.join(", ")
                            }),
                    )
                    .when(!added, |d| {
                        d.on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            cx.stop_propagation();
                            this.env_add_lsp(key.clone(), spec.clone(), window, cx)
                        }))
                    })
                    .into_any_element()
            });
            return Some(Card::new(
                "Add a language server",
                v_flex()
                    .w_full()
                    .py_1()
                    .children(items)
                    .child(
                        div()
                            .px_3()
                            .pt_2()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Each installs itself on first use, into Bench's folder on the machine where the project's files are (~/.bench/lsp): it needs npm there. The list is the gear's Presets; another server for this project alone: open any row and write it into lsp.json."),
                    )
                    .into_any_element(),
                vec![],
            ));
        }
        let mut actions = vec![save("lsp", e.lsp.dirty, cx)];
        actions.extend(revert("lsp", e.lsp.dirty, cx));
        // Opened from its row: that server can go (asked once).
        if e.lsp.servers.iter().any(|s| s.name == name) {
            actions.push(remove(e, "lsp", name.to_string(), "Remove this server", cx));
        }
        Some(Card::new(
            "lsp.json: command, args, extensions; npm and fetch install into ${LSP}",
            code(&e.lsp.text, cx),
            actions,
        ))
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
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
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !e.lsp.dirty {
            let text = serde_json::to_string_pretty(&json!({"servers": l["servers"].clone()}))
                .unwrap_or_default();
            e.lsp.text.update(cx, |s, cx| s.set_value(text, window, cx));
        }
    }
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        Some((
            "PUT",
            format!("/api/projects/{eid}/lsp"),
            json!({"text": e.lsp.text.read(cx).value().to_string()}),
        ))
    }
    fn saved(&self, e: &mut ProjectEnv) {
        e.lsp.dirty = false;
    }
    fn remove_path(&self, eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/projects/{eid}/lsp/{}", super::enc(name)))
    }
    fn revert(&self, app: &mut BenchApp, _: &mut Window, _: &mut Context<BenchApp>) -> bool {
        if let Some(e) = app.project_env.as_mut() {
            e.lsp.dirty = false;
        }
        false
    }
}

impl BenchApp {
    /// A preset's server into the project's lsp.json (wherever the project lives), then the
    /// environment read again.
    pub fn env_add_lsp(
        &mut self,
        key: String,
        spec: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.project_env.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "PUT",
                    &format!("/api/projects/{eid}/lsp"),
                    &profile,
                    json!({"name": key, "spec": spec}),
                )
            },
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.card = None;
                    }
                    this.load_env(&id, window, cx);
                }
                Err(err) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.error = Some(format!("{err:#}"));
                    }
                    cx.notify();
                }
            },
        );
    }
}
