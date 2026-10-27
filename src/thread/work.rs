//! The agent's work between two messages: one line "N actions · time · Done", the steps under
//! it when opened. Plus what tool calls say about the outside world: the pages they touched.

use ::thread::{AskCard, Hold, PlanStep, Step, worked_ms};
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
        // The person's holds of the live run: its clock skips them; the last open one means
        // it stands held, and the hold's own button shows it.
        holds: &[Hold],
        plan: Option<&(String, Vec<PlanStep>)>,
        asks: &[AskCard],
        procs: bool,
        // The answer this work leads to, when the run has ended: its read-aloud and copy
        // ride this line's far edge, the exact place Pause and Stop held while the run
        // lived — standing there from the start, shown on hover.
        answer: Option<(String, String)>,
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
        let paused = live && holds.last().is_some_and(|h| h.to.is_none());
        // A live piece of work counts on — but not while held: it stands, then goes on from
        // there; a finished one keeps the time its steps took.
        let secs = match (from, to, live) {
            (Some(a), _, true) => Some(worked_ms(a, now_secs() * 1000., holds) / 1000.),
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
        // What the agent is doing now, in a line: the words it wrote — the narrative of what
        // it is after — and, when a call is the newest thing, a short word for its kind after
        // a dot: the story and the concrete step at once. Commands and paths stay where they
        // belong — in the opened step list. Before any word: the neutral one. Held: no
        // shimmer, and the hold's button says why.
        let last_live = steps.iter().rev().find(|s| !matches!(s, Step::Thought(_)));
        let narrative = steps.iter().rev().find_map(|s| match s {
            Step::Note(t) => t
                .lines()
                .rev()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| clip(l, 120)),
            _ => None,
        });
        let doing = live.then(|| match last_live {
            // Held: nothing is being done, and the line says so in the step's place.
            _ if paused => match &narrative {
                Some(n) => format!("{n} · Paused"),
                None => "Paused".to_string(),
            },
            Some(Step::Tool { name, preview, .. }) => {
                let w = activity_word(name, preview);
                match narrative.clone() {
                    Some(n) => format!("{n} · {w}"),
                    None => w,
                }
            }
            Some(Step::Note(_)) => narrative.unwrap_or_else(|| "Thinking…".to_string()),
            _ => "Thinking…".to_string(),
        });
        let work_key = key.to_string();
        // The tail's two, in the transport's seat: same buttons' size, same edge, same gap.
        let tail = answer.filter(|_| !live);
        let tgroup: SharedString = format!("worktail-{key}").into();
        let (tplay, tspeak) = tail
            .as_ref()
            .map_or((false, false), |(ak, _)| match self.app(cx) {
                Some(a) => (
                    a.playing.as_ref().is_some_and(|(j, _)| j == ak),
                    a.speaking.as_deref() == Some(ak),
                ),
                None => (false, false),
            });
        let summary = h_flex()
            .id(SharedString::from(format!("work-{key}")))
            .group(tgroup.clone())
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
                Some(label) => {
                    // The narrative continues the counts; with no counts it stands alone.
                    let label = if has_parts {
                        format!("· {label}")
                    } else {
                        label
                    };
                    d.child(div().min_w_0().truncate().child(if paused {
                        div().text_color(muted).child(label).into_any_element()
                    } else {
                        ShimmerText::new(label)
                            .text_base()
                            .text_color(muted)
                            .into_any_element()
                    }))
                }
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
            // The hold and the stop, at the far edge while the run is live: the actions are
            // where the minutes go, so the answers to them ride the actions' own line, always
            // in sight, and never make the line jump. A finished run has nothing to hold.
            .when(live, |d| {
                let held = paused;
                let stop_view = self.thread_ref();
                d.child(
                    h_flex()
                        .flex_none()
                        .ml_auto()
                        .gap_1()
                        .child(
                            bench_ui::head_icon_button(
                                "work-hold",
                                if held { Ico::Play } else { Ico::Pause },
                                if held { "Resume" } else { "Pause" },
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.claim_click(cx);
                                    this.set_paused(!held, cx);
                                },
                            )),
                        )
                        .child(
                            bench_ui::head_icon_button("work-stop", Ico::Square, "Stop", cx)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.claim_click(cx);
                                    if let (Some(profile), Some(session)) =
                                        (this.profile(cx), stop_view.runtime.clone())
                                    {
                                        this.cmd(
                                            crate::client::rpc::Cmd::Interrupt { profile, session },
                                            cx,
                                        );
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
            // The answer's own two, in the same seat the hold and the stop sat in: read
            // aloud, and copy the agent's markdown. Hidden until the line is hovered —
            // unless that answer is playing or being spoken right now.
            .when_some(tail, |d, (ak, atext)| {
                let copy_id = format!("work-copy-{key}");
                let speak_id = format!("work-speak-{key}");
                let ak_copy = ak.clone();
                d.child(
                    h_flex()
                        .flex_none()
                        .ml_auto()
                        .gap_1()
                        .items_center()
                        .when(
                            !(tplay || tspeak) && std::env::var("BENCH_TOOLS_SHOWN").is_err(),
                            |s| {
                                let shown = self.reveal_tail.as_deref() == Some(ak.as_str());
                                s.when(!shown, |s| {
                                    s.opacity(0.).group_hover(tgroup, |g| g.opacity(1.))
                                })
                            },
                        )
                        .child(
                            bench_ui::head_icon_button(&copy_id, Ico::Copy, "Copy", cx).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.claim_click(cx);
                                    this.copy_answer(&ak_copy, window, cx);
                                }),
                            ),
                        )
                        .child(
                            bench_ui::head_icon_button(
                                &speak_id,
                                if tplay {
                                    Ico::Square
                                } else if tspeak {
                                    Ico::Loader
                                } else {
                                    Ico::Volume
                                },
                                if tplay { "Stop" } else { "Read aloud" },
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.claim_click(cx);
                                    this.app
                                        .update(cx, |app, cx| {
                                            app.toggle_speak(&ak, atext.clone(), window, cx)
                                        })
                                        .ok();
                                },
                            )),
                        ),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.claim_click(cx);
                // The click takes the auto-opened line over: from now the person decides
                // whether it stays open, also when the run ends.
                if this.thread.auto_work.as_deref() == Some(work_key.as_str()) {
                    this.thread.auto_work = None;
                }
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
        // A call of this work waiting for the person's approval: its card joins the questions.
        // The request is live state (the call is the journal's): it is asked again after a restart.
        let approval = self.thread.approval.as_ref().filter(|a| {
            steps
                .iter()
                .any(|s| matches!(s, Step::Tool { call_id, .. } if *call_id == a.prompt_id))
        });
        let asks: Vec<AskCard> = asks
            .iter()
            .cloned()
            .chain(approval.map(|a| AskCard {
                id: a.prompt_id.clone(),
                ask_kind: "approve".into(),
                question: if a.description.is_empty() {
                    "The agent asks to run this".into()
                } else {
                    format!("The agent asks to run this: {}", a.description)
                },
                detail: Some(a.command.clone()),
                choices: a.choices.clone(),
                multi: false,
                answer: None,
            }))
            .collect();
        let mut artifacts = self.render_work_artifacts(plan, &asks, cx);
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

/// The one word for what a kind of tool is doing, for the live line: a person reads words
/// faster than command lines, and the opened step list still holds the exact command.
fn activity_word(name: &str, preview: &str) -> String {
    // A file's own name, not its path: the line is about the thing, not where it lives.
    let target = || {
        preview
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim())
            .map(|l| {
                std::path::Path::new(l)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| clip(l, 40))
            })
            .unwrap_or_default()
    };
    let with = |verb: &str| {
        let t = target();
        if t.is_empty() {
            format!("{verb}…")
        } else {
            format!("{verb} {t}…")
        }
    };
    if name.starts_with("mcp__browser") {
        return "browsing…".to_string();
    }
    match name {
        "bash" | "ssh_run" => "running a command…".to_string(),
        "read" => with("reading"),
        "edit" | "write" | "search_replace" => with("editing"),
        "grep" | "glob" | "ls" | "session_search" | "tool_search" => "searching…".to_string(),
        "web_fetch" | "web_search" => "checking the web…".to_string(),
        "delegate" => "delegating…".to_string(),
        "todo" => "planning…".to_string(),
        "memory" => "taking notes…".to_string(),
        "tasks" => "the task list…".to_string(),
        "cron" => "the schedule…".to_string(),
        "clarify" => "asking you…".to_string(),
        "skill_view" | "skill_manage" => "a skill…".to_string(),
        _ if name.starts_with("mcp__") => {
            let tool = name.rsplit("__").next().unwrap_or(name).replace('_', " ");
            format!("using {tool}…")
        }
        _ => format!("using {}…", name.replace('_', " ")),
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
