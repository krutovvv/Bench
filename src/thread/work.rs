//! The agent's work between two messages: one line "N actions · time · Done", the steps under
//! it when opened. Plus what tool calls say about the outside world: the pages they touched.

use ::thread::{AskCard, PlanStep, Step};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::ThreadView;
use super::message::speaker_row;
use crate::app::now_secs;
use bench_ui::icons::Ico;
use bench_ui::{clip, clock, plural};

impl ThreadView {
    pub(crate) fn render_work(
        &mut self,
        key: &str,
        steps: &[Step],
        from: Option<f64>,
        to: Option<f64>,
        live: bool,
        plan: &[(String, Vec<PlanStep>)],
        asks: &[AskCard],
        procs: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let tools = steps.iter().filter(|s| s.is_tool()).count();
        let errors = steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { failed: true, .. }))
            .count();
        let open = self.thread.open_work.contains(key);
        // A live piece of work counts on; a finished one keeps the time its steps took.
        let secs = match (from, to, live) {
            (Some(a), _, true) => Some((now_secs() * 1000. - a).max(0.) / 1000.),
            (Some(a), Some(b), false) => Some((b - a).max(0.) / 1000.),
            _ => None,
        };
        let mut parts: Vec<(String, Hsla)> = vec![];
        if tools > 0 {
            parts.push((plural(tools, "action", "actions", "actions"), muted));
        }
        if errors > 0 {
            parts.push((plural(errors, "error", "errors", "errors"), theme.danger));
        }
        if let Some(s) = secs.filter(|s| *s >= 1.) {
            parts.push((clock(s as u64), muted));
        }
        let has_parts = !parts.is_empty();
        // What the agent is doing now: its last step, said in a line.
        let doing = live.then(|| match steps.last() {
            Some(Step::Tool { name, preview, .. }) => tool_label(name, preview),
            Some(Step::Note(t)) => clip(t, 160),
            _ => "Thinking…".to_string(),
        });
        let work_key = key.to_string();
        let summary = h_flex()
            .id(SharedString::from(format!("work-{key}")))
            .gap_1p5()
            .items_center()
            .min_w_0()
            .text_color(muted)
            .cursor_pointer()
            .hover(|s| s.text_color(theme.foreground))
            .children(parts.into_iter().enumerate().map(|(k, (t, c))| {
                div()
                    .flex_none()
                    .text_color(c)
                    .child(if k == 0 { t } else { format!("· {t}") })
            }))
            .map(|d| match doing {
                Some(label) => d.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(ShimmerText::new(label).text_base().text_color(muted)),
                ),
                // A piece of work without tool calls was the model thinking before it answered.
                None => d.child(div().flex_none().child(
                    match (has_parts, tools > 0 || errors > 0) {
                        (true, true) => "· Done",
                        (false, true) => "Done",
                        (true, false) => "· Thought",
                        (false, false) => "Thought",
                    },
                )),
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.claim_click(cx);
                if !this.thread.open_work.remove(&work_key) {
                    this.thread.open_work.insert(work_key.clone());
                }
                cx.notify();
            }));
        let items: Vec<AnyElement> = if open {
            steps
                .iter()
                .enumerate()
                .map(|(k, s)| self.render_step(key, k, s, cx))
                .collect()
        } else {
            vec![]
        };
        let list = open.then(|| {
            v_flex()
                .pl_3()
                .gap_1()
                .border_l_1()
                .border_color(theme.border)
                .children(items)
        });
        // The plan and the questions this work wrote, under its steps; the live work also
        // carries the background processes it started.
        let mut artifacts = self.render_work_artifacts(plan, asks, cx);
        if procs {
            artifacts.extend(self.processes_frame(cx));
        }
        // Work that only asked or only planned has no "N actions" line to open: the cards are it.
        let no_steps = steps.is_empty() && !live;
        speaker_row(
            Some(true),
            v_flex()
                .gap_3()
                .when(!no_steps, |d| d.child(summary))
                .children(list)
                .children(artifacts),
            cx,
        )
        .into_any_element()
    }

    /// One step; a click unfolds its details (reasoning, arguments, output).
    fn render_step(
        &mut self,
        key: &str,
        k: usize,
        step: &Step,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.thread.open_steps.contains(&(key.to_string(), k));
        let (icon, color, label, details): (Ico, Hsla, String, Vec<(&str, String)>) = match step {
            Step::Tool {
                name,
                preview,
                args,
                output,
                failed,
                ..
            } => (
                if *failed {
                    Ico::CircleAlert
                } else {
                    Ico::Terminal
                },
                if *failed { theme.danger } else { muted },
                clip(&tool_label(name, preview), 220),
                [
                    ("Arguments", pretty(args)),
                    ("Output", output.clone().unwrap_or_default()),
                ]
                .into_iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .collect(),
            ),
            Step::Note(text) => (
                Ico::ChevronRight,
                theme.foreground.opacity(0.75),
                clip(text, 400),
                vec![],
            ),
            Step::Thought(text) => (
                Ico::Brain,
                muted,
                clip(text, 160),
                vec![("Reasoning", text.clone())],
            ),
        };
        let expandable = !details.is_empty();
        let step_key = (key.to_string(), k);
        let bodies: Vec<(&str, AnyElement)> = if open {
            details
                .into_iter()
                .map(|(title, body)| {
                    let text =
                        self.thread
                            .texts
                            .plain(&format!("step-{key}-{k}-{title}"), &body, cx);
                    (title, text.into_any_element())
                })
                .collect()
        } else {
            vec![]
        };
        v_flex()
            .child(
                h_flex()
                    .id(SharedString::from(format!("step-{key}-{k}")))
                    .gap_2()
                    .items_start()
                    .text_color(color)
                    .when(expandable, |d| {
                        d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
                    })
                    .child(icon.icon().xsmall().mt(px(3.)))
                    .child(div().flex_1().min_w_0().child(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.claim_click(cx);
                        if !this.thread.open_steps.remove(&step_key) {
                            this.thread.open_steps.insert(step_key.clone());
                        }
                        cx.notify();
                    })),
            )
            .when(open && expandable, |d| {
                d.children(bodies.into_iter().map(|(title, body)| {
                    v_flex()
                        .ml_5()
                        .mt_1()
                        .mb_1()
                        .child(div().text_color(muted).child(title))
                        .child(
                            div()
                                .mt_0p5()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .font_family(bench_ui::theme::MONO)
                                .text_color(theme.foreground)
                                .child(body),
                        )
                }))
            })
            .into_any_element()
    }
}

/// JSON as readable text, capped so a huge value cannot stall the UI.
pub fn pretty(v: &Value) -> String {
    let text = match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    if text.chars().count() > 12_000 {
        format!(
            "{}\n… (truncated)",
            text.chars().take(12_000).collect::<String>()
        )
    } else {
        text
    }
}

/// The external pages a tool call touched: http(s) URLs in its arguments. File tools and code
/// carry URLs that are not pages visited, so they are skipped.
pub fn tool_urls(name: &str, args: &Value, out: &mut Vec<String>) {
    let name = name.to_lowercase();
    if ["write", "patch", "edit", "create", "memory", "skill"]
        .iter()
        .any(|w| name.contains(w))
    {
        return;
    }
    const CODE: &[&str] = &[
        "content",
        "code",
        "text",
        "patch",
        "new_string",
        "old_string",
        "script",
        "html",
        "body",
        "expression",
        "function",
        "js",
        "javascript",
        "source",
    ];
    match args {
        Value::Object(m) => m
            .iter()
            .filter(|(k, _)| !CODE.contains(&k.as_str()))
            .for_each(|(_, v)| urls_in(v, out)),
        other => urls_in(other, out),
    }
}

fn urls_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            for word in s.split(|c: char| c.is_whitespace() || "\"'<>`()[]".contains(c)) {
                let Some(start) = word.find("http") else {
                    continue;
                };
                let url = word[start..].trim_end_matches(['.', ',', ';', ':', '!', '?']);
                if (url.starts_with("http://") || url.starts_with("https://"))
                    && !is_local(&host_of(url))
                    && !url.contains("w3.org/")
                    && !url.contains('{')
                    && !out.iter().any(|u| u == url)
                {
                    out.push(url.to_string());
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| urls_in(x, out)),
        Value::Object(m) => m.values().for_each(|x| urls_in(x, out)),
        _ => {}
    }
}

pub fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    host.trim_start_matches("www.").to_lowercase()
}

fn is_local(host: &str) -> bool {
    host.is_empty()
        || host == "localhost"
        || host.ends_with(".local")
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("0.0.0.0")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (16..=31).contains(&n)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn pages_a_tool_touched() {
        let mut out = vec![];
        super::tool_urls(
            "web_fetch",
            &json!({"url": "https://www.example.com/a", "code": "https://x.dev"}),
            &mut out,
        );
        super::tool_urls(
            "bash",
            &json!({"command": "curl http://localhost:3000 https://docs.rs/gpui."}),
            &mut out,
        );
        super::tool_urls("edit", &json!({"path": "https://nope.io"}), &mut out);
        assert_eq!(out, ["https://www.example.com/a", "https://docs.rs/gpui"]);
        assert_eq!(
            super::host_of("https://www.example.com:8080/a"),
            "example.com"
        );
    }
}

///  ("session search"; an MCP tool
/// `mcp__<server>__<tool>` as "<server> · <tool>"), then what it worked on.
pub fn tool_label(name: &str, detail: &str) -> String {
    let name = match name.strip_prefix("mcp__").and_then(|n| n.split_once("__")) {
        Some((server, tool)) => format!("{server} · {}", tool.replace('_', " ")),
        None => name.replace('_', " "),
    };
    // One line: a command with a heredoc, a patch, a quoted page — the running line is one row
    // tall whatever the tool was handed, or the thread jumps in height as the work changes.
    let detail = clip_line(detail);
    if detail.is_empty() {
        name
    } else {
        format!("{name}: {detail}")
    }
}

/// One line of at most 160 characters.
fn clip_line(s: &str) -> String {
    let line = s
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .trim();
    if line.chars().count() > 160 {
        format!("{}…", line.chars().take(159).collect::<String>())
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tool_label_tests {
    #[test]
    fn labels() {
        assert_eq!(super::tool_label("bash", "ls"), "bash: ls");
        assert_eq!(super::tool_label("session_search", ""), "session search");
        assert_eq!(
            super::tool_label("bash", "python3 - <<'PY'\nimport re\np='src/app.rs'"),
            "bash: python3 - <<'PY'",
            "a multiline command stays one line"
        );
        assert_eq!(super::tool_label("bash", "\n\n   \n"), "bash");
        assert_eq!(
            super::tool_label("mcp__reminders__list_items", "Today"),
            "reminders · list items: Today"
        );
    }
}
