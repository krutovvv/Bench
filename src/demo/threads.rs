//! The demo's threads: one per subject, so every sidebar group opens to something, each one turn
//! and done; the landing page's dev server still running.


use crate::app::BenchApp;
use crate::app::model::BgProcess;
use protocol::ThreadItem;
use serde_json::json;
use super::journal::*;

/// A thread about something (`tasks:today`, `ssh:local`…): one turn, done.
fn thread(app: &mut BenchApp, id: &str, title: &str, subject: &str, journal: Journal) {
    let row = session(id, title, 2, false, "desktop");
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
        app.inserts.insert(
            id.into(),
            vec![crate::app::model::Insert {
                id: format!("i{id}"),
                key,
                state,
                folded: false,
                after: 0,
            }],
        );
    }
    app.sessions.push(row);
    // The insert the thread is about stands first, before the first message.
    let mut items = vec![];
    if app.inserts.contains_key(id) {
        items.push(ThreadItem::Insert {
            key: "0".into(),
            seq: 0,
            id: format!("i{id}"),
            insert_key: String::new(),
        });
    }
    let mut t = journal.thread();
    let inserts = app.inserts.get(id).map(|list| {
        list.iter()
            .map(|i| (i.id.clone(), json!({ "key": i.key })))
            .collect::<serde_json::Map<_, _>>()
    });
    if !items.is_empty() {
        t.apply(protocol::ThreadEvent::Append(items));
    }
    if let Some(m) = inserts {
        t.apply(protocol::ThreadEvent::Inserts(m));
    }
    app.demo_threads.insert(id.into(), t);
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
        "d-today",
        "What's left for today",
        "tasks:today",
        done(
            "What's left for today? Move the RFC to tomorrow and log 40 minutes on the API task.",
            vec![
                tool("tasks", "list today"),
                tool("tasks", "update: Read the RFC on rate limits → tomorrow"),
                tool("tasks", "update: Rate limiting for the public API, +40 min"),
            ],
            "Three left for today:\n\n\
- **Send the quarterly report** (flagged)\n\
- **Rate limiting for the public API** — 2 h 15 min logged now\n\
- **Review the landing page copy**\n\n\
The RFC moved to tomorrow.",
            9.0,
        ),
    );
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
    app.sites.demo_frame =
        crate::inserts::browser::gateway::image_of(include_bytes!("../../assets/demo/site.jpg"));
    thread(
        app,
        "d-tool",
        "Cron for work hours",
        "tool:cron-parser",
        done(
            "Is this right for every 15 minutes during work hours on weekdays?",
            vec![],
            "Yes: `*/15 9-18 * * 1-5` fires at :00, :15, :30 and :45 from 09:00 to 18:45, Monday to \
Friday — the next runs are listed above. If 18:45 is too late, make the hours `9-17`.",
            4.0,
        ),
    );
    app.tools
        .inputs
        .insert(devtools::Tool::Cron, "*/15 9-18 * * 1-5".into());
}

/// The landing page thread's dev server, still running.
pub fn processes(now: f64) -> Vec<BgProcess> {
    vec![BgProcess {
        id: "p-1".into(),
        command: "npm run dev -- --port 5173".into(),
        running: true,
        exit_code: None,
        started: Some(now - 250.0),
        exited: None,
        reason: None,
        tail: "VITE v6.2  ready in 412 ms\n  ➜  Local:   http://localhost:5173/".into(),
    }]
}
