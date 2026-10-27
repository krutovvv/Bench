//! The project's language servers (its lsp.json, as Crush's lsp section): after every edit the
//! agent gets the file's errors from them (`agent/src/lsp.ts`). A row per server — the files it
//! knows, how it is doing; "+ Add" offers the usual ones (each installs itself on first use, on
//! the machine where the project's files are); any row opens lsp.json — a file of the project's
//! agent dir (`<dir>/lsp.json`) — as a document, to edit or paste from another project.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::ProjectEnv;
use super::section::{Card, Home, Item, Section, add};
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

/// The project's language servers as the agent reads them; lsp.json itself is a file of the
/// project's agent dir, edited as any file (`lsp_file`).
#[derive(Default)]
pub struct LspEnv {
    pub servers: Vec<LspRow>,
}

/// The project's lsp.json, as the Explorer opens it (None: its agent dir is not read yet).
pub fn lsp_file(e: &ProjectEnv) -> Option<crate::explorer::FileRef> {
    (!e.dir.is_empty()).then(|| crate::explorer::FileRef {
        project: e.id.clone(),
        machine: None,
        path: format!("{}/lsp.json", e.dir.trim_end_matches('/')),
    })
}

pub struct Lsp;

impl Section for Lsp {
    fn key(&self) -> &'static str {
        "lsp"
    }
    fn home(&self) -> Home {
        Home::Explorer("Tools")
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
    /// A row per server; each opens lsp.json.
    fn items(&self, _app: &BenchApp, e: &ProjectEnv, cx: &mut Context<BenchApp>) -> Vec<Item> {
        let theme = cx.theme().clone();
        let file = lsp_file(e);
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
                Item {
                    name: s.name.clone(),
                    label: s.name.clone(),
                    note: format!("{} · {state}", s.extensions.join(", ")),
                    icon: (
                        crate::icons::Ico::FileCog,
                        if bad {
                            theme.danger
                        } else {
                            theme.muted_foreground
                        },
                    ),
                    file: file.clone(),
                }
            })
            .collect()
    }
    /// "new": the presets to add (a server's own row opens lsp.json, a file).
    fn card(
        &self,
        app: &BenchApp,
        e: &ProjectEnv,
        name: &str,
        cx: &mut Context<BenchApp>,
    ) -> Option<Card> {
        let theme = cx.theme().clone();
        if name != "new" {
            return None;
        }
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
            let (key, spec, id) = (key.clone(), spec.clone(), e.id.clone());
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
                        this.env_add_lsp(id.clone(), key.clone(), spec.clone(), window, cx)
                    }))
                })
                .into_any_element()
        });
        Some(Card::new(
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
        ))
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
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
}

impl BenchApp {
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
