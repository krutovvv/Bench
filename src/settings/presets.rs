//! The person's presets (`agent/src/builtin/presets.ts`): MCP and language servers a project
//! takes with one click, the compaction shares and reasoning levels its Model section offers.
//! This Mac's agent keeps them; the gear's Presets tab edits them; a project on a server gets the
//! finished spec from here.

use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::BenchApp;
use crate::client::rest;

/// The presets as Bench uses them; `raw` is each kind as the agent gave it (the editors' text).
#[derive(Clone, Debug, Default)]
pub struct Presets {
    /// (name, title, spec)
    pub mcp: Vec<(String, String, Value)>,
    pub lsp: Vec<(String, String, Value)>,
    /// (share of the window, name)
    pub compaction: Vec<(f64, String)>,
    pub reasoning: Vec<String>,
    pub raw: Value,
}

impl Presets {
    fn from(v: &Value) -> Self {
        let named = |k: &str| -> Vec<(String, String, Value)> {
            v[k].as_object()
                .map(|o| {
                    o.iter()
                        .map(|(n, p)| {
                            (
                                n.clone(),
                                p["title"].as_str().unwrap_or(n).to_string(),
                                p["spec"].clone(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            mcp: named("mcp"),
            lsp: named("lsp"),
            compaction: v["compaction"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| {
                    Some((
                        c["at"].as_f64()?,
                        c["name"].as_str().unwrap_or_default().to_string(),
                    ))
                })
                .collect(),
            reasoning: v["reasoning"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| l.as_str().map(str::to_string))
                .collect(),
            raw: v.clone(),
        }
    }
}

/// The gear's Presets tab: one editor per kind, the kind open, whether it changed.
pub struct PresetEditor {
    pub kind: &'static str,
    pub text: Entity<EditorState>,
}

pub const KINDS: [(&str, &str); 4] = [
    ("mcp", "MCP servers"),
    ("lsp", "Language servers"),
    ("compaction", "Compaction"),
    ("reasoning", "Reasoning levels"),
];

impl BenchApp {
    /// The presets from this Mac's agent; the reasoning levels follow them.
    pub fn load_presets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        self.background(
            window,
            cx,
            move || rest::get(&conn, "/api/presets"),
            |this, r, _window, cx| {
                if let Ok(v) = r {
                    this.set_presets(Presets::from(&v), cx);
                }
            },
        );
    }

    fn set_presets(&mut self, p: Presets, cx: &mut Context<Self>) {
        if !p.reasoning.is_empty() && p.reasoning != self.defaults.efforts {
            self.set_efforts(p.reasoning.clone(), cx);
        }
        self.gear.presets = p;
        cx.notify();
    }

    /// A kind of presets opened to edit, as JSON.
    pub fn edit_presets(
        &mut self,
        kind: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.gear.preset_editor.as_ref().is_some_and(|e| e.kind == kind) {
            self.gear.preset_editor = None;
            cx.notify();
            return;
        }
        let text = serde_json::to_string_pretty(&self.gear.presets.raw[kind]).unwrap_or_default();
        let editor = crate::env::editor("json", window, cx);
        editor.update(cx, |s, cx| s.set_value(text, window, cx));
        self.gear.preset_editor = Some(PresetEditor { kind, text: editor });
        cx.notify();
    }

    /// The open kind saved (`reset`: back to the built-in one).
    pub fn save_presets(&mut self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(ed)) = (self.local_conn(), self.gear.preset_editor.as_ref()) else {
            return;
        };
        let kind = ed.kind;
        let text = ed.text.read(cx).value().to_string();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "PUT",
                    &format!("/api/presets/{kind}"),
                    json!({"text": text, "reset": reset}),
                )
            },
            |this, r, window, cx| match r {
                Ok(v) => {
                    this.gear.preset_editor = None;
                    this.set_presets(Presets::from(&v["presets"]), cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }
}

impl BenchApp {
    /// The Presets tab: each kind as a few lines, Edit opens its JSON (Save, Built-in, Cancel).
    pub(super) fn render_presets(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let p = self.gear.presets.clone();
        let mut out = vec![div()
            .pt_4()
            .text_sm()
            .text_color(muted)
            .child("What a project takes with one click (+ Add in its environment), and the choices of its Model section. Kept by the local agent; a server's projects take them too. Keys can be written right into a preset.")
            .into_any_element()];
        for (kind, title) in crate::settings::presets::KINDS {
            let lines: Vec<(String, String)> = match kind {
                "mcp" => p
                    .mcp
                    .iter()
                    .map(|(n, t, _)| (n.clone(), t.clone()))
                    .collect(),
                "lsp" => p
                    .lsp
                    .iter()
                    .map(|(n, t, _)| (n.clone(), t.clone()))
                    .collect(),
                "compaction" => p
                    .compaction
                    .iter()
                    .map(|(at, n)| (format!("{}%", (at * 100.).round()), n.clone()))
                    .collect(),
                _ => vec![(
                    p.reasoning.join(", "),
                    "offered in a project's Model section and the composer".into(),
                )],
            };
            out.push(
                bench_ui::settings_heading(title, vec![], cx).into_any_element(),
            );
            for (name, note) in lines {
                out.push(
                    h_flex()
                        .gap_2()
                        .py_1p5()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(div().flex_none().child(name))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(muted)
                                .child(note),
                        )
                        .into_any_element(),
                );
            }
            let open = self
                .gear.preset_editor
                .as_ref()
                .filter(|e| e.kind == kind)
                .map(|e| e.text.clone());
            match open {
                Some(text) => {
                    out.push(
                        div()
                            .pt_2()
                            .child(crate::env::section::code(&text, cx))
                            .into_any_element(),
                    );
                    out.push(
                        h_flex()
                            .py_2()
                            .gap_2()
                            .child(
                                bench_ui::primary_button(
                                    SharedString::from(format!("preset-save-{kind}")),
                                    "Save",
                                )
                                .small()
                                .on_click(
                                    cx.listener(|this, _, w, cx| this.save_presets(false, w, cx)),
                                )
                                .into_any_element(),
                            )
                            .child(
                                bench_ui::secondary_button(
                                    SharedString::from(format!("preset-reset-{kind}")),
                                    "Built-in",
                                )
                                .small()
                                .on_click(
                                    cx.listener(|this, _, w, cx| this.save_presets(true, w, cx)),
                                )
                                .into_any_element(),
                            )
                            .child(
                                bench_ui::secondary_button(
                                    SharedString::from(format!("preset-cancel-{kind}")),
                                    "Cancel",
                                )
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.gear.preset_editor = None;
                                    cx.notify();
                                }))
                                .into_any_element(),
                            )
                            .into_any_element(),
                    );
                }
                None => out.push(
                    h_flex()
                        .py_2()
                        .child(
                            bench_ui::secondary_button(
                                SharedString::from(format!("preset-edit-{kind}")),
                                "Edit",
                            )
                            .small()
                            .on_click(
                                cx.listener(move |this, _, w, cx| this.edit_presets(kind, w, cx)),
                            )
                            .into_any_element(),
                        )
                        .into_any_element(),
                ),
            }
        }
        out
    }
}
