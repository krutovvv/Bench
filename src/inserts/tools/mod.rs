//! `/tools`: the everyday utilities in the thread, one insert for all of them — its title
//! switches between them (Base64, JSON, regex, cron…), its head has the tool's modes, the tool
//! under it. Its state: {tool}.

pub mod state;
pub mod view;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::inserts::state::Insert;
use crate::inserts::slash::tool_key;
use crate::inserts::{InsertKind, Switch, clip};
use crate::thread::ThreadRef;
use devtools::{GROUPS, Tool};

/// The tool's least height in the thread (it grows with its text).
const MIN_H: f32 = 200.;

pub struct Tools;

fn tool_of(ins: &Insert) -> Tool {
    let name = ins.get("tool");
    GROUPS
        .iter()
        .flat_map(|g| g.tools.iter().copied())
        .find(|t| tool_key(*t) == name)
        .unwrap_or(GROUPS[0].tools[0])
}

impl InsertKind for Tools {
    fn owns(&self, key: &str) -> bool {
        key == "tools"
    }

    fn word(&self) -> crate::inserts::Word {
        crate::inserts::Word {
            word: "tools",
            aliases: &["tool"],
            desc: "Everyday utilities: Base64, JSON, regex, cron…",
            key: "tools",
        }
    }

    /// "tools:<tool>" (or "tool:<tool>"): the utility chosen.
    fn spec(&self, slash_key: &str) -> Option<(String, serde_json::Map<String, serde_json::Value>)> {
        match slash_key.split_once(':') {
            Some(("tools" | "tool", v)) if !v.is_empty() => {
                let mut state = serde_json::Map::new();
                state.insert("tool".into(), json!(v));
                Some(("tools".into(), state))
            }
            _ => self.owns(slash_key).then(|| (slash_key.to_string(), Default::default())),
        }
    }

    fn welcome(&self) -> Option<crate::inserts::WelcomeChip> {
        Some(crate::inserts::WelcomeChip {
            icon: bench_ui::icons::Ico::Wrench,
            label: "Utilities",
            compose: true,
        })
    }

    /// One row per utility, under its group's name.
    fn choices(&self, _app: &BenchApp, _at: &ThreadRef) -> Vec<crate::thread::composer::slash::SlashCommand> {
        GROUPS
            .iter()
            .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
            .map(|(t, g)| crate::thread::composer::slash::SlashCommand {
                name: format!("/tools {}", t.name()),
                desc: g.to_string(),
                category: "Tool".into(),
                key: format!("tools:{}", tool_key(t)),
            })
            .collect()
    }

    /// Each utility its own word in the bare "/" list ("/base64", "/json-yaml"), its name and
    /// group beside it; "/tools …" still lists them all.
    fn words(&self) -> Vec<crate::thread::composer::slash::SlashCommand> {
        GROUPS
            .iter()
            .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
            .map(|(t, g)| crate::thread::composer::slash::SlashCommand {
                name: format!("/{}", tool_key(t)),
                desc: format!("{} · {g}", t.name()),
                category: "Tool".into(),
                key: format!("tools:{}", tool_key(t)),
            })
            .collect()
    }

    fn shared(&self) -> Option<&'static str> {
        Some("tools")
    }

    fn title(&self, _app: &BenchApp, _at: &ThreadRef, ins: &Insert) -> String {
        tool_of(ins).name().to_string()
    }

    fn switches(&self, _app: &BenchApp, _at: &ThreadRef, ins: &Insert) -> Vec<Switch> {
        let now = tool_of(ins);
        GROUPS
            .iter()
            .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
            .map(|(t, g)| Switch {
                label: t.name().to_string(),
                detail: g.to_string(),
                chosen: t == now,
                to: crate::inserts::state::new_state(json!({ "tool": tool_key(t) })),
            })
            .collect()
    }

    fn controls(
        &self,
        app: &mut BenchApp,
        _at: &ThreadRef,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Vec<AnyElement> {
        let tool = tool_of(ins);
        let modes = tool.modes();
        let mut out = vec![
            div()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(tool.hint())
                .into_any_element(),
        ];
        if !modes.is_empty() {
            out.push(crate::inserts::tools::view::segmented(
                "insert-tool-mode",
                modes,
                app.inserts.tools.mode(tool),
                cx,
                |this, k, window, cx| this.set_tool_mode(k, window, cx),
            ));
        }
        out
    }

    fn body(
        &self,
        app: &mut BenchApp,
        _at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let tool = tool_of(ins);
        if app.inserts.tools.tool != tool {
            app.select_tool(tool, window, cx);
        }
        // As tall as its longer side (input or output): the thread scrolls, not the tool. But
        // never taller than half the window — a pasted page would otherwise blow the row up to
        // thousands of pixels; what doesn't fit, the editor scrolls inside its pane.
        let lines = |e: &Entity<gpui_kit::component::input::EditorState>| {
            e.read(cx).value().lines().count()
        };
        let n = lines(&app.inserts.tools.input).max(lines(&app.inserts.tools.output)) + 2;
        let want = px(n as f32 * bench_ui::editor_line(cx) + bench_ui::BLOCK_HEAD_H + 24.);
        let cap = window.bounds().size.height * 0.5;
        let h = want.max(px(MIN_H)).min(cap.max(px(MIN_H)));
        Some(
            div()
                .h(h)
                .mx(px(-12.))
                .my(px(-8.))
                .child(app.render_tools_body(cx))
                .into_any_element(),
        )
    }

    fn context(&self, app: &BenchApp, _at: &ThreadRef, ins: &Insert, cx: &App) -> String {
        let tool = tool_of(ins);
        if app.inserts.tools.tool != tool {
            return String::new();
        }
        let input = app.inserts.tools.input.read(cx).value().to_string();
        let output = app.inserts.tools.output.read(cx).value().to_string();
        format!(
            "The tools insert is on {} ({}).\nIts input:\n```\n{}\n```\nIts output:\n```\n{}\n```",
            tool.name(),
            tool.hint(),
            clip(&input, 6000),
            clip(&output, 6000)
        )
    }

    fn opened(
        &self,
        app: &mut BenchApp,
        _at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.select_tool(tool_of(ins), window, cx);
    }
}
