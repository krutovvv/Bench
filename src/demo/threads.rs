//! The demo's threads: one per subject, so every sidebar group opens to something, each one turn
//! and done; the landing page's dev server still running.

use super::journal::*;
use crate::app::BenchApp;
use protocol::ThreadItem;

/// A thread about something (`ssh:local`, `tool:json`…): one turn, done.
fn thread(app: &mut BenchApp, id: &str, title: &str, subject: &str, journal: Journal) {
    app.threads.list.push(row(id, title, false, "desktop"));
    let mut t = journal.thread();
    // A tool: an insert before the first message.
    if !subject.is_empty() {
        let spec = match subject.split_once(':') {
            Some(("tool", t)) => format!("tools:{t}"),
            Some(("browser", _)) => "browser".into(),
            Some(("ssh", "")) => "ssh:local".into(),
            _ => subject.to_string(),
        };
        let (key, mut state) = BenchApp::insert_spec(&spec);
        if key == "browser" {
            state.insert("tab".into(), serde_json::json!("demo-tab"));
            state.insert(
                "url".into(),
                serde_json::json!("https://blog.example.com/admin/posts/new"),
            );
        }
        // The insert the thread is about stands first, before the first message.
        let insert = format!("i{id}");
        t.apply(protocol::ThreadEvent::Append(vec![ThreadItem::Insert {
            key: "0".into(),
            seq: 0,
            id: insert.clone(),
            insert_key: key.clone(),
        }]));
        let state = protocol::InsertState {
            key,
            state,
            ..Default::default()
        };
        t.apply(protocol::ThreadEvent::Inserts([(insert, state)].into()));
        let list = t
            .insert_list()
            .into_iter()
            .map(|(id, s)| crate::inserts::state::Insert::new(id, s))
            .collect();
        app.inserts.by_thread.insert(id.into(), list);
    }
    app.demo_state.threads.insert(id.into(), t);
}

/// One thread per subject, so every sidebar group opens to something.
pub(super) fn subject_threads(app: &mut BenchApp, now: f64) {
    let done = |user: &str, calls: Vec<Call>, answer: &str, secs: f64| {
        let mut j = Journal::default();
        j.turn(user, None, calls, answer, now - 600.0, secs);
        j
    };
    thread(
        app,
        "d-web1-tls",
        "Renew the TLS certificates",
        "ssh:local",
        done(
            "Renew the certificates before they expire on Friday.",
            vec![
                tool("ssh_run", "certbot renew"),
                tool("ssh_run", "systemctl reload nginx"),
            ],
            "Renewed both certificates (until **January 22**) and reloaded nginx; the site answers over \
the new ones.",
            31.0,
        ),
    );
    thread(
        app,
        "d-web1",
        "Why is the site slow",
        "ssh:local",
        done(
            "The site got slow this morning. What's going on?",
            vec![
                tool("ssh_run", "uptime"),
                tool("ssh_run", "docker stats --no-stream"),
                tool("ssh_run", "journalctl -u nginx --since 09:00 | tail -50"),
            ],
            "The `search-indexer` container has been at **188% CPU** since 09:12: last night's reindex \
started late and is still running. Nginx is fine — no errors, cached pages answer in 140 ms.\n\n\
To cap it at one core and restart it:\n\n\
```bash\ndocker update --cpus 1 search-indexer && docker restart search-indexer\n```\n\n\
Say so and I'll run it here.",
            21.0,
        ),
    );
    thread(
        app,
        "d-site",
        "Publish the release notes",
        "browser:d1",
        done(
            "Open the blog's admin, make a draft from release-notes.md and stop before publishing.",
            vec![
                tool("browser_tabs", "new"),
                tool(
                    "browser_navigate",
                    "https://blog.example.com/admin/posts/new",
                ),
                tool("browser_fill_form", "title, body, tags"),
                tool("browser_snapshot", ""),
            ],
            "The draft **Release 2.4: faster search** is in the editor with the text from \
`release-notes.md`, the tags *releases* and *search*, and a cover. It is saved as a draft; nothing \
is published.\n\nThe page is right here — click and type in it if something needs a touch. Say \
*publish* and I'll press the button.",
            48.0,
        ),
    );
    app.inserts.sites.demo_frame =
        crate::inserts::browser::gateway::image_of(include_bytes!("../../assets/demo/site.jpg"));
    thread(
        app,
        "d-tool",
        "Cron for work hours",
        &match std::env::var("BENCH_DEMO_TOOL").as_deref() {
            Ok("html") => "tool:rich-text-to-markdown",
            Ok("preview") => "tool:markdown-preview",
            Ok("long") => "tool:base64",
            _ => "tool:cron-parser",
        }
        .to_string(),
        done(
            "Is this right for every 15 minutes during work hours on weekdays?",
            vec![],
            "Yes: `*/15 9-18 * * 1-5` fires at :00, :15, :30 and :45 from 09:00 to 18:45, Monday to \
Friday — the next runs are listed above. If 18:45 is too late, make the hours `9-17`.",
            4.0,
        ),
    );
    app.inserts.tools
        .inputs
        .insert(devtools::Tool::Cron, "*/15 9-18 * * 1-5".into());
    // `BENCH_DEMO_TOOL=html|preview`: the insert shows the pane that scrolls inside the thread
    // (the clipboard's rich text, a Markdown preview), with lines enough to scroll.
    match std::env::var("BENCH_DEMO_TOOL").as_deref() {
        Ok("long") => {
            // A pasted page: 120 lines of markup, the tallest thing a pane is asked to hold.
            let one = (0..120)
                .map(|n| format!("<div class=\"post\">{n} <!DOCTYPE html><h1>413 Request Entity Too Large</h1> openresty/1.15.8.2</div>"))
                .collect::<Vec<_>>()
                .join("\n");
            app.inserts.tools.inputs.insert(devtools::Tool::Base64, one.clone());
            app.inserts.tools.preview = one;
        }
        Ok("html") | Ok("preview") => {
            let lines = (1..=40)
                .map(|n| format!("{n}. A line of the page, long enough to fill the pane's width."))
                .collect::<Vec<_>>()
                .join("\n");
            let tool = if std::env::var("BENCH_DEMO_TOOL").unwrap() == "html" {
                devtools::Tool::HtmlToMarkdown
            } else {
                devtools::Tool::MarkdownPreview
            };
            app.inserts.tools.inputs.insert(tool, lines.clone());
            app.inserts.tools.preview = lines;
        }
        _ => {}
    }
}

/// The landing page thread's dev server, still running.
/// The dev server the landing thread's agent left running (`process start`).
pub(super) fn processes(now: f64) -> Vec<::thread::Process> {
    vec![::thread::Process {
        id: "41".into(),
        command: "npm run dev -- --port 5173".into(),
        state: ::thread::ProcessState::Running,
        started: Some((now - 250.0) * 1000.),
        tail: "VITE v6.2  ready in 412 ms\n  ➜  Local:   http://localhost:5173/".into(),
        ..Default::default()
    }]
}
