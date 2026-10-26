//! The agent's work between two messages: one line "N actions · time · Done", the steps under
//! it when opened. Plus what tool calls say about the outside world: the pages they touched.

use ::thread::Step;
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::message::speaker_row;
use crate::app::{BenchApp, now_secs, tool_label};
use crate::icons::Ico;
use crate::ui::media::plain_text;
use crate::ui::{clip, clock, plural};

impl BenchApp {
    pub(super) fn render_work(
        &mut self,
        key: &str,
        steps: &[Step],
        from: Option<f64>,
        to: Option<f64>,
        live: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let tools = steps.iter().filter(|s| s.is_tool()).count();
        let errors = steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { failed: true, .. }))
            .count();
        let open = self.open_work.contains(key);
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
                None => d.child(div().flex_none().child(if tools > 0 || errors > 0 {
                    "· Done"
                } else {
                    "Done"
                })),
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.click_claimed = true;
                if !this.open_work.remove(&work_key) {
                    this.open_work.insert(work_key.clone());
                }
                cx.notify();
            }));
        let list = open.then(|| {
            v_flex()
                .pl_3()
                .gap_1()
                .border_l_1()
                .border_color(theme.border)
                .children(
                    steps
                        .iter()
                        .enumerate()
                        .map(|(k, s)| self.render_step(key, k, s, cx)),
                )
        });
        speaker_row(
            Some(true),
            v_flex().gap_3().child(summary).children(list),
            cx,
        )
        .into_any_element()
    }

    /// One step; a click unfolds its details (reasoning, arguments, output).
    fn render_step(&self, key: &str, k: usize, step: &Step, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.open_steps.contains(&(key.to_string(), k));
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
                        this.click_claimed = true;
                        if !this.open_steps.remove(&step_key) {
                            this.open_steps.insert(step_key.clone());
                        }
                        cx.notify();
                    })),
            )
            .when(open && expandable, |d| {
                d.children(details.into_iter().map(|(title, body)| {
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
                                .font_family(crate::theme::MONO)
                                .text_color(theme.foreground)
                                .child(plain_text(format!("step-{key}-{k}-{title}"), &body)),
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
