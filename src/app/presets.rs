//! The person's presets (`agent/src/builtin/presets.ts`): MCP and language servers a project
//! takes with one click, the compaction shares and reasoning levels its Model section offers.
//! This Mac's agent keeps them; the gear's Presets tab edits them; a project on a server gets the
//! finished spec from here.

use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::BenchApp;
use crate::rest;

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
                            (n.clone(), p["title"].as_str().unwrap_or(n).to_string(), p["spec"].clone())
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
                .filter_map(|c| Some((c["at"].as_f64()?, c["name"].as_str().unwrap_or_default().to_string())))
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
            move || rest::get_json(&conn, "/api/presets"),
            |this, r, window, cx| {
                if let Ok(v) = r {
                    this.set_presets(Presets::from(&v), window, cx);
                }
            },
        );
    }

    fn set_presets(&mut self, p: Presets, window: &mut Window, cx: &mut Context<Self>) {
        if !p.reasoning.is_empty() && p.reasoning != self.efforts {
            self.set_efforts(p.reasoning.clone(), window, cx);
        }
        self.presets = p;
        cx.notify();
    }

    /// A kind of presets opened to edit, as JSON.
    pub fn edit_presets(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if self.preset_editor.as_ref().is_some_and(|e| e.kind == kind) {
            self.preset_editor = None;
            cx.notify();
            return;
        }
        let text = serde_json::to_string_pretty(&self.presets.raw[kind]).unwrap_or_default();
        let editor = crate::env::editor("json", window, cx);
        editor.update(cx, |s, cx| s.set_value(text, window, cx));
        self.preset_editor = Some(PresetEditor { kind, text: editor });
        cx.notify();
    }

    /// The open kind saved (`reset`: back to the built-in one).
    pub fn save_presets(&mut self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(ed)) = (self.local_conn(), self.preset_editor.as_ref()) else {
            return;
        };
        let kind = ed.kind;
        let text = ed.text.read(cx).value().to_string();
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "PUT",
                    &format!("/api/presets/{kind}"),
                    &profile,
                    json!({"text": text, "reset": reset}),
                )
            },
            |this, r, window, cx| match r {
                Ok(v) => {
                    this.preset_editor = None;
                    this.set_presets(Presets::from(&v["presets"]), window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }
}
