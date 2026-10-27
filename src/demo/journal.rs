//! The made-up pieces of a thread: rows of the sidebar, steps of the agent's work, a journal as Pi
//! gives it, the answer's media.

use crate::app::model::SessionRow;
use ::thread::Thread;
use protocol::{ThreadItem, ThreadOpen};
use serde_json::{Value, json};

pub(super) fn demo_file(name: &str) -> String {
    format!("{}/assets/demo/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub(super) fn session(
    id: &str,
    title: &str,
    messages: u64,
    running: bool,
    source: &str,
) -> SessionRow {
    SessionRow {
        id: id.into(),
        title: title.into(),
        messages,
        running,
        paused: false,
        source: source.into(),
        subject: String::new(),
        project: String::new(),
        host: String::new(),
    }
}

/// A made-up step of the agent's work: a tool, what it acted on, its arguments, an edit's diff.
#[derive(Clone)]
pub(super) struct Call {
    name: &'static str,
    preview: String,
    args: Value,
    diff: Option<String>,
}

pub(super) fn tool(name: &'static str, detail: &str) -> Call {
    let args = match name {
        "edit" | "read" | "write" => json!({ "path": detail }),
        "web_fetch" | "browser_navigate" => json!({ "url": detail }),
        _ => json!({ "command": detail }),
    };
    Call {
        name,
        preview: detail.into(),
        args,
        diff: None,
    }
}

pub(super) fn edit(path: &str, diff: &str) -> Call {
    Call {
        diff: Some(diff.into()),
        ..tool("edit", path)
    }
}

/// A made-up journal, entry ids in order as Pi gives them; times in Unix milliseconds.
#[derive(Default)]
pub(super) struct Journal {
    items: Vec<ThreadItem>,
    seq: u64,
}

impl Journal {
    pub(super) fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// One exchange: the person's words, the work, the answer, `secs` after `at` (seconds).
    pub(super) fn turn(
        &mut self,
        user: &str,
        thought: Option<&str>,
        calls: Vec<Call>,
        answer: &str,
        at: f64,
        secs: f64,
    ) {
        let ms = |t: f64| Some(t * 1000.);
        let seq = self.next();
        self.items.push(ThreadItem::User {
            key: format!("{seq}.0"),
            seq,
            text: user.into(),
            at: ms(at),
        });
        let n = calls.len().max(1) as f64;
        for (k, c) in calls.into_iter().enumerate() {
            let t = at + secs * (k as f64 + 1.) / (n + 1.);
            let seq = self.next();
            if k == 0 {
                if let Some(th) = thought {
                    self.items.push(ThreadItem::Thought {
                        key: format!("{seq}.0"),
                        seq,
                        text: th.into(),
                        at: ms(t),
                    });
                }
            }
            let call_id = format!("c{seq}");
            self.items.push(ThreadItem::Tool {
                key: format!("{seq}.1"),
                seq,
                call_id: call_id.clone(),
                name: c.name.into(),
                args: c.args,
                preview: c.preview,
                at: ms(t),
            });
            let seq = self.next();
            self.items.push(ThreadItem::Result {
                key: format!("{seq}.0"),
                seq,
                call_id,
                name: c.name.into(),
                output: "ok".into(),
                failed: false,
                diff: c.diff,
                at: ms(t + 1.),
            });
        }
        let seq = self.next();
        self.items.push(ThreadItem::Text {
            key: format!("{seq}.0"),
            seq,
            text: answer.into(),
            at: ms(at + secs),
            error: None,
            narration: Some(false),
        });
    }

    pub(super) fn thread(self) -> Thread {
        Thread::open(ThreadOpen {
            items: self.items,
            ..Default::default()
        })
    }
}

/// The answer's media. On the desktop: files the agent delivered (`MEDIA:` lines), a Mermaid
/// diagram drawn here, a video and a voice note that play inline. In the browser there is no
/// disk and no ffmpeg: the chart and the (pre-drawn) diagram come by URL next to the page, and
/// the clips are named instead.
#[cfg(not(target_family = "wasm"))]
pub(super) fn media() -> (String, String, String) {
    let video = demo_file("preview.mp4");
    crate::media::warm(&video);
    (
        format!("MEDIA:{}", demo_file("chart.png")),
        "```mermaid\ngraph LR\n  A[Visit] --> B[Hero]\n  B --> C[Quotes]\n  C --> D[Pricing]\n  D --> E[Signup]\n```".into(),
        format!(
            "A 4-second preview of the hero animation, and the summary as a voice note:\n\nMEDIA:{video}\n\nMEDIA:{}\n",
            demo_file("summary.m4a")
        ),
    )
}

#[cfg(target_family = "wasm")]
pub(super) fn media() -> (String, String, String) {
    // The page's folder: …/bench/ on GitHub Pages, / locally.
    let href = web_sys::window()
        .and_then(|w| w.location().href().ok())
        .unwrap_or_default();
    let base = match href.split(['?', '#']).next().unwrap_or("").rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/"),
        None => String::new(),
    };
    (
        format!("![Weekly signups]({base}assets/demo/chart.png)"),
        format!("![The flow]({base}assets/demo/flow.png)"),
        "In the desktop app a 4-second video preview and the summary as a voice note play right \
here; the browser demo leaves them out.\n"
            .into(),
    )
}
