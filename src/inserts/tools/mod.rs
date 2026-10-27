//! `/tools`: the everyday utilities in the thread, one insert for all of them — its title
//! switches between them (Base64, JSON, regex, cron…), its head has the tool's modes, the tool
//! under it. Its state: {tool}.

pub mod state;
pub mod view;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::app::model::Insert;
use crate::inserts::subjects::tool_key;
use crate::inserts::{InsertKind, Switch, clip};
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

    fn shared(&self) -> Option<&'static str> {
        Some("tools")
    }

    fn title(&self, _app: &BenchApp, ins: &Insert) -> String {
        tool_of(ins).name().to_string()
    }

    fn switches(&self, _app: &BenchApp, ins: &Insert) -> Vec<Switch> {
        let now = tool_of(ins);
        GROUPS
            .iter()
            .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
            .map(|(t, g)| Switch {
                label: t.name().to_string(),
                detail: g.to_string(),
                chosen: t == now,
                to: json!({ "state": { "tool": tool_key(t) } }),
            })
            .collect()
    }

    fn controls(
        &self,
        app: &mut BenchApp,
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
                app.tools.mode(tool),
                cx,
                |this, k, window, cx| this.set_tool_mode(k, window, cx),
            ));
        }
        out
    }

    fn body(
        &self,
        app: &mut BenchApp,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let tool = tool_of(ins);
        if app.tools.tool != tool {
            app.select_tool(tool, window, cx);
        }
        // As tall as its longer side (input or output): the thread scrolls, not the tool.
        let lines = |e: &Entity<gpui_kit::component::input::EditorState>| {
            e.read(cx).value().lines().count()
        };
        let n = lines(&app.tools.input).max(lines(&app.tools.output)) + 2;
        let h = (n as f32 * bench_ui::editor_line(cx) + bench_ui::BLOCK_HEAD_H + 24.).max(MIN_H);
        Some(
            div()
                .h(px(h))
                .mx(px(-12.))
                .my(px(-8.))
                .child(app.render_tools_body(cx))
                .into_any_element(),
        )
    }

    fn context(&self, app: &BenchApp, ins: &Insert, cx: &App) -> String {
        let tool = tool_of(ins);
        if app.tools.tool != tool {
            return String::new();
        }
        let input = app.tools.input.read(cx).value().to_string();
        let output = app.tools.output.read(cx).value().to_string();
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
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.select_tool(tool_of(ins), window, cx);
    }
}
