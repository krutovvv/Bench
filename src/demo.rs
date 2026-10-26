//! A made-up session for screenshots and the web demo: `BENCH_DEMO=1 ./target/release/bench`
//! opens the browser thread (the centrepiece); `BENCH_DEMO=tasks|server|tool` another subject's
//! thread; `BENCH_DEMO=landing` the landing-page chat at the end of its answer, `BENCH_DEMO=top`
//! at its beginning with the agent's steps unfolded.
//!
//! No gateway, nothing of the person's: invented chats in the sidebar and one
//! open session that shows what the agent's client can do — steps and a subagent, an answer with
//! a code block, a chart, a diagram, a formula, a video and a voice reply, its sources, the
//! files it changed, a process still running. The media live in `assets/demo/`.

use gpui_kit::*;

use crate::app::BenchApp;
use crate::model::{BgProcess, Place, Profile, SessionRow, Target, Usage};
use ::thread::Thread;
use protocol::{ThreadItem, ThreadOpen};
use serde_json::{Value, json};

fn demo_file(name: &str) -> String {
    format!("{}/assets/demo/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn session(id: &str, title: &str, messages: u64, running: bool, source: &str) -> SessionRow {
    SessionRow {
        id: id.into(),
        title: title.into(),
        messages,
        running,
        source: source.into(),
        subject: String::new(),
        project: String::new(),
        host: String::new(),
    }
}

/// A made-up step of the agent's work: a tool, what it acted on, its arguments, an edit's diff.
#[derive(Clone)]
struct Call {
    name: &'static str,
    preview: String,
    args: Value,
    diff: Option<String>,
}

fn tool(name: &'static str, detail: &str) -> Call {
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

fn edit(path: &str, diff: &str) -> Call {
    Call {
        diff: Some(diff.into()),
        ..tool("edit", path)
    }
}

/// A made-up journal, entry ids in order as Pi gives them; times in Unix milliseconds.
#[derive(Default)]
struct Journal {
    items: Vec<ThreadItem>,
    seq: u64,
}

impl Journal {
    fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// One exchange: the person's words, the work, the answer, `secs` after `at` (seconds).
    fn turn(
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
        });
    }

    fn thread(self) -> Thread {
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
fn media() -> (String, String, String) {
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
fn media() -> (String, String, String) {
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

fn server(alias: &str, hostname: &str, folder: &str) -> crate::sshconfig::Server {
    crate::sshconfig::Server {
        alias: alias.into(),
        hostname: hostname.into(),
        identity: String::new(),
        user: "deploy".into(),
        port: String::new(),
        folder: folder.into(),
        path: String::new(),
        shared: false,
    }
}

fn task(id: &str, title: &str, tags: &[&str], day: Option<i64>) -> crate::tasks::Task {
    crate::tasks::Task {
        id: id.into(),
        title: title.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        day: day.map(|d| crate::tasks::today() + chrono::Duration::days(d)),
        created: crate::tasks::now(),
        project: "p-me".into(),
        ..Default::default()
    }
}

/// Made-up servers (SSH mode) and tasks (Tasks mode), so every mode has something to show.
fn other_modes(app: &mut BenchApp) {
    app.ssh.servers = vec![
        server("web-1", "203.0.113.10", "Production"),
        server("web-2", "203.0.113.11", "Production"),
        server("db-main", "203.0.113.20", "Production"),
        server("staging", "198.51.100.5", "Staging"),
        server("ci-runner", "198.51.100.9", "Staging"),
        server("homelab", "192.168.1.50", ""),
    ];
    let today = crate::tasks::today();
    let mut done = task("t-done", "Renew the TLS certificates", &["ops"], Some(-1));
    done.completed = Some(today - chrono::Duration::days(1));
    done.spent = 25 * 60;
    let mut flagged = task("t-tax", "Send the quarterly report", &["finance"], Some(0));
    flagged.important = true;
    let evening = task(
        "t-read",
        "Read the RFC on rate limits",
        &["reading"],
        Some(1),
    );
    let mut timed = task(
        "t-api",
        "Rate limiting for the public API",
        &["storefront"],
        Some(0),
    );
    timed.spent = 135 * 60;
    timed.notes = "Token bucket per API key; 429 with Retry-After.".into();
    app.tasks.items = vec![
        flagged,
        timed,
        task(
            "t-copy",
            "Review the landing page copy",
            &["storefront", "marketing"],
            Some(0),
        ),
        evening,
        task(
            "t-demo",
            "Prepare the demo for Friday",
            &["storefront"],
            Some(2),
        ),
        task("t-1on1", "One-on-ones: agenda", &["team"], Some(3)),
        task("t-trip", "Book the conference trip", &["travel"], Some(9)),
        task(
            "t-idea",
            "Try a local model for code review",
            &["ideas"],
            None,
        ),
        done,
    ];
    app.tasks.loaded = true;
    // `BENCH_DEMO=tasks-open`: the task card open, as when it is being edited.
    if std::env::var("BENCH_DEMO").as_deref() == Ok("tasks-open") {
        app.tasks.selected = Some("t-api".into());
    }
}

/// The demo server's terminal: its last screen (no shell runs in the demo).
pub fn terminal(alias: &str) -> String {
    format!(
        "deploy@{alias}:~$ uptime\n \
10:41:07 up 37 days,  2:14,  1 user,  load average: 3.91, 3.62, 2.10\n\
deploy@{alias}:~$ docker stats --no-stream\n\
NAME             CPU %     MEM USAGE / LIMIT\n\
search-indexer   188.42%   1.21GiB / 3.84GiB\n\
storefront       6.10%     412MiB / 3.84GiB\n\
nginx            0.31%     18MiB / 3.84GiB\n\
deploy@{alias}:~$ "
    )
}

/// A thread about something (`tasks:today`, `ssh:web-1`…): one turn, done.
fn thread(app: &mut BenchApp, id: &str, title: &str, subject: &str, journal: Journal) {
    let mut row = session(id, title, 2, false, "desktop");
    // An environment is the thread's top; a tool, an insert before the first message.
    if subject.starts_with("env:") {
        row.subject = subject.into();
    } else if !subject.is_empty() {
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
            vec![crate::model::Insert {
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

/// The blog project's environment, for its environment thread.
pub fn env(app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    let Some(e) = app.project_env.as_mut() else {
        return;
    };
    let v = serde_json::json!({
        "dir": "~/.bench/projects/p-blog",
        "soul": "The Northwind blog: product news and guides for our customers, published on Ghost.\n\nWrite plainly, for people who use the product, not for engineers. Every post gets a cover and two tags; drafts first, publish only when asked.",
        "mcp": [
            {"name": "ghost", "kind": "http", "enabled": true, "connected": true, "tools": 12},
            {"name": "unsplash", "kind": "stdio", "enabled": true, "connected": true, "tools": 3},
            {"name": "figma", "kind": "http", "enabled": true, "connected": true, "tools": 0, "error": "needs signing in", "needs_auth": true},
            {"name": "search-console", "kind": "stdio", "enabled": false, "connected": false, "tools": 0}
        ],
        "mcp_json": "{\n  \"mcpServers\": {\n    \"ghost\": { \"url\": \"https://blog.example.com/ghost/mcp\", \"headers\": { \"Authorization\": \"${GHOST_ADMIN_KEY}\" } },\n    \"unsplash\": { \"command\": \"npx\", \"args\": [\"unsplash-mcp\"] },\n    \"search-console\": { \"command\": \"uvx\", \"args\": [\"gsc-mcp\"], \"enabled\": false }\n  }\n}\n",
        "skills": [
            {"name": "release-notes", "description": "Turn a changelog into a post: what changed for the reader, not the commit list.", "scope": "project", "dir": "~/.bench/projects/p-blog/skills/release-notes"},
            {"name": "cover-image", "description": "A 1600×900 cover in the blog's colors, from a photo or a gradient.", "scope": "project", "dir": "~/.bench/projects/p-blog/skills/cover-image"},
            {"name": "ghost-theme", "description": "How the blog's Ghost theme is built and changed.", "scope": "repo", "dir": "~/Projects/blog/.agents/skills/ghost-theme"},
            {"name": "humanizer", "description": "Make a text read as written by a person.", "scope": "general", "dir": "~/.bench/agents/default/skills/humanizer"}
        ],
        "hosts": [{"id": "h-blog", "name": "blog-files", "kind": "sftp", "user": "deploy", "host": "203.0.113.30"}],
        "cron": [
            {"id": "c-digest", "name": "Weekly digest", "prompt": "Draft the weekly digest from this week's posts and comments; save it as a draft.", "schedule": {"expr": "0 9 * * 1"}, "schedule_display": "0 9 * * 1", "enabled": true, "next_run_at": "2026-10-26T09:00:00+03:00", "last_run_at": "2026-10-19T09:00:00+03:00", "last_status": "ok", "last_output": "The digest draft is saved: 4 posts, 2 top comments."},
            {"id": "c-links", "name": "Broken links", "prompt": "Check every post's links; list the broken ones with the post.", "schedule": {"expr": "every 1d"}, "schedule_display": "every 1d", "enabled": false, "next_run_at": null, "last_status": null}
        ]
    });
    crate::app::project_env::apply(e, &v, window, cx);
    // `BENCH_DEMO=env-open`: a skill's card open, as when it is being edited.
    if std::env::var("BENCH_DEMO").as_deref() == Ok("env-open") && e.card.is_none() {
        app.env_card("skill", "release-notes".into(), window, cx);
        if let Some(e) = app.project_env.as_mut() {
            e.folded.extend([
                "Project",
                "Model",
                "MCP servers",
                "Skills",
                "Prompts",
                "Schedule",
                "Accesses",
            ]);
        }
    }
}

/// One thread per subject, so every sidebar group opens to something.
fn subject_threads(app: &mut BenchApp, now: f64) {
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
        "ssh:web-1",
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
        "ssh:web-1",
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
        crate::remote_browser::image_of(include_bytes!("../assets/demo/site.jpg"));
    thread(
        app,
        "d-env",
        "Northwind blog: environment",
        "env:p-blog",
        done(
            "Add the Ghost admin API, so you can publish without the browser.",
            vec![tool("edit", "~/.bench/projects/p-blog/mcp.json")],
            "Added **ghost** (12 tools: posts, tags, images) with the admin key as `${GHOST_ADMIN_KEY}`; \
the key itself is in the agent's env. New threads of the blog can draft and publish through it.",
            14.0,
        ),
    );
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

/// Fill the window with the demo instead of connecting.
pub fn load(app: &mut BenchApp, cx: &mut Context<BenchApp>) {
    let now = crate::app::now_secs();
    app.demo = true;
    app.target = Target::Local;
    app.connecting = false;
    app.profiles = vec![Profile {
        name: "default".into(),
        label: "default".into(),
        model: "Qwen3.8-Next-Flash".into(),
    }];
    app.profile = Some("default".into());
    app.profile_ready = true;
    app.approval_mode = Some("smart".into());
    app.default_cwd = Some("/Users/demo".into());

    app.sessions = vec![
        session(
            "d-landing",
            "Launch the spring landing page",
            42,
            false,
            "desktop",
        ),
        session(
            "d-notes",
            "Summarize this week's meeting notes",
            8,
            false,
            "desktop",
        ),
        session(
            "d-api",
            "Add rate limiting to the public API",
            23,
            false,
            "desktop",
        ),
        session(
            "d-tests",
            "Fix the flaky checkout test",
            15,
            false,
            "desktop",
        ),
        session(
            "d-audit-1",
            "Competitors: pricing pages",
            31,
            true,
            "desktop",
        ),
        session(
            "d-audit-2",
            "Competitors: onboarding flows",
            19,
            false,
            "desktop",
        ),
        session(
            "d-tg",
            "Invoice for March from the inbox",
            6,
            false,
            "telegram",
        ),
        session("d-cron", "Daily digest of new issues", 4, false, "cron"),
    ];
    let place = |cwd: &str, repo: &str| Place {
        cwd: cwd.into(),
        repo: repo.into(),
        branch: if repo.is_empty() {
            String::new()
        } else {
            "main".into()
        },
    };
    for (id, p) in [
        (
            "d-api",
            place("/Users/demo/code/storefront", "/Users/demo/code/storefront"),
        ),
        (
            "d-tests",
            place("/Users/demo/code/storefront", "/Users/demo/code/storefront"),
        ),
        (
            "d-audit-1",
            place("/Users/demo/Research/Competitor audit", ""),
        ),
        (
            "d-audit-2",
            place("/Users/demo/Research/Competitor audit", ""),
        ),
    ] {
        app.places.insert(id.into(), p);
    }

    let (chart, flow, clips) = media();

    let answer = format!(
        "The new landing page is **live**, and the numbers so far look good.\n\n\
## What changed\n\n\
- A shorter hero: one promise, one button, the price right under it.\n\
- Social proof moved up: three customer quotes before the feature list.\n\
- The signup form lost two fields (company size, phone).\n\n\
Weekly signups, three weeks before and after:\n\n\
{chart}\n\n\
The flow the page now follows:\n\n\
{flow}\n\n\
Conversion went from $c_0 = 2.1\\%$ to $c_1 = 2.8\\%$, a lift of $\\frac{{c_1 - c_0}}{{c_0}} \\approx 33\\%$.\n\n\
To run it locally:\n\n\
```bash\nnpm install\nnpm run dev -- --port 5173\n```\n\n\
{clips}"
    );
    let mut journal = Journal::default();
    journal.turn(
        "Ship the spring landing page we discussed: shorter hero, quotes first, two \
fewer form fields. Then check what it did to signups and give me a short summary.",
        Some("Plan: edit the page, run the dev server, compare signups."),
        vec![
            tool("read", "/Users/demo/code/storefront/src/pages/landing.tsx"),
            edit(
                "/Users/demo/code/storefront/src/pages/landing.tsx",
                "@@ -12,9 +12,7 @@\n-  <Hero title={long} subtitle={sub} />\n+  <Hero title=\"Ship faster\" />\n+  <Quotes />\n   <Features />\n-  <Quotes />\n",
            ),
            edit(
                "/Users/demo/code/storefront/src/components/SignupForm.tsx",
                "@@ -30,6 +30,4 @@\n-  <Field name=\"companySize\" />\n-  <Field name=\"phone\" />\n",
            ),
            tool("bash", "npm run dev -- --port 5173"),
            tool("web_fetch", "https://www.nngroup.com/articles/landing-page-guidelines/"),
            tool("web_fetch", "https://developer.mozilla.org/en-US/docs/Web/Performance"),
            tool("execute_code", "chart of weekly signups"),
            tool("text_to_speech", "summary.m4a"),
        ],
        &answer,
        now - 312.0,
        252.0,
    );
    let landing = journal.thread();
    if std::env::var("BENCH_DEMO").as_deref() == Ok("top") {
        if let Some(::thread::Row::Work { key, .. }) = landing
            .rows()
            .into_iter()
            .find(|r| matches!(r, ::thread::Row::Work { .. }))
        {
            app.open_work.insert(key);
        }
    }
    app.thread = landing.clone();
    app.demo_threads.insert("d-landing".into(), landing);
    app.session = Some("d-landing".into());
    other_modes(app);
    subject_threads(app, now);
    // The demo's projects: each thread in one (the Telegram and cron ones stay without).
    let project = |id: &str, name: &str, folder: Option<&str>| crate::model::ProjectEntry {
        id: id.into(),
        name: name.into(),
        folder: folder.map(str::to_string),
        host: None,
        browser: true,
    };
    app.projects = vec![
        project("p-blog", "Northwind blog", None),
        project("p-store", "Storefront", Some("/Users/demo/code/storefront")),
        project("p-audit", "Competitor audit", None),
        project("p-me", "Personal", None),
    ];
    for s in app.sessions.iter_mut() {
        s.project = match s.id.as_str() {
            "d-site" | "d-env" => "p-blog",
            "d-landing" | "d-api" | "d-tests" | "d-web1" | "d-web1-tls" => "p-store",
            "d-audit-1" | "d-audit-2" => "p-audit",
            "d-today" | "d-tool" | "d-notes" => "p-me",
            _ => "",
        }
        .into();
    }
    // The demo's own folds, not the person's: every group open but the long Tools.
    app.ssh.collapsed = ["s:Tools".to_string()].into_iter().collect();
    // `BENCH_DEMO=tasks|server|browser|tool`: that thread open instead.
    let open = match std::env::var("BENCH_DEMO").as_deref() {
        Ok("tasks") => Some("d-today"),
        Ok("server") => Some("d-web1"),
        Ok("tool") => Some("d-tool"),
        Ok("env") | Ok("env-open") => Some("d-env"),
        Ok("landing") | Ok("top") => None,
        _ => Some("d-site"),
    };
    if let Some(id) = open {
        app.session = Some(id.into());
        app.thread = app.demo_threads.get(id).cloned().unwrap_or_default();
    }
    app.processes = processes(now);
    if open.is_some() {
        app.processes.clear();
    }
    app.usage = Some(Usage {
        model: "Qwen3.8-Next-Flash".into(),
        context_used: 38_400,
        context_max: 131_072,
        calls: 14,
        input: 212_000,
        output: 9_800,
        ..Default::default()
    });
    cx.notify();
}
