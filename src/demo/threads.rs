//! The demo's threads as an agent lists and opens them: the landing page's, one per subject so
//! every sidebar group opens to something, each one turn and done, and the rest of the list.

use std::collections::HashMap;

use protocol::{ThreadItem, ThreadOpen, ThreadSummary, Usage};

use super::hooks::DemoHooks;
use super::journal::*;
use crate::app::BenchApp;

/// A thread about something (`ssh:local`, `tool:json`…): one turn, done; the insert it is about
/// stands first, before the first message.
fn with_insert(id: &str, subject: &str, journal: Journal) -> ThreadOpen {
    let mut open = journal.open();
    let spec = match subject.split_once(':') {
        Some(("tool", t)) => format!("tools:{t}"),
        Some(("browser", _)) => "browser".into(),
        _ => subject.to_string(),
    };
    let (key, mut state) = BenchApp::insert_spec(&spec);
    if key == "browser" {
        state.insert("tab".into(), serde_json::json!(super::agent::BROWSER_TAB));
        state.insert("url".into(), serde_json::json!(super::agent::BROWSER_URL));
    }
    let insert = format!("i{id}");
    open.items.insert(
        0,
        ThreadItem::Insert {
            key: "0".into(),
            seq: 0,
            id: insert.clone(),
            insert_key: key.clone(),
        },
    );
    let state = protocol::InsertState {
        key,
        state,
        ..Default::default()
    };
    open.inserts = [(insert, state)].into();
    open
}

/// The landing page's thread: steps, an answer with a code block, a chart, a diagram, a formula, a
/// video and a voice reply, its sources, the files it changed, a process still running.
fn landing(hooks: &DemoHooks, now: f64) -> ThreadOpen {
    // `BENCH_DEMO_JOURNAL=<file>`: a real thread in the landing's place, as `thread.open` gave it
    // (saved from the agent) — the person's data drawn by this copy, nothing sent anywhere.
    if let Some(open) = hooks
        .journal
        .as_ref()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str::<ThreadOpen>(&t).ok())
    {
        return open;
    }
    let mut journal = Journal::default();
    if hooks.mode() == "long" {
        // A thread of sixty exchanges.
        for k in 0..60 {
            let answer = format!("Answer {k}. ")
                + &"The numbers held steady through the week. ".repeat(8 + k % 5 * 6);
            journal.turn(&format!("Question {k}?"), None, vec![], &answer, now - 7200. + k as f64 * 100., 30.);
        }
        return journal.open();
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
            tool("process", "npm run dev -- --port 5173"),
            tool("web_fetch", "https://www.nngroup.com/articles/landing-page-guidelines/"),
            tool("web_fetch", "https://developer.mozilla.org/en-US/docs/Web/Performance"),
            tool("execute_code", "chart of weekly signups"),
            tool("text_to_speech", "summary.m4a"),
        ],
        &answer,
        now - 312.0,
        252.0,
    );
    // What the turn changed, git's view: the two edits and the lock file `npm install` rewrote.
    let changes = journal.changes(
        "/Users/demo/code/storefront",
        &[
            ("src/pages/SPEC.md", 3, 1),
            ("src/pages/landing.tsx", 2, 2),
            ("src/components/SignupForm.tsx", 0, 2),
            ("package-lock.json", 14, 9),
        ],
    );
    if hooks.review.is_some() {
        journal.review(
            &changes,
            &[
                (
                    "/Users/demo/code/storefront/src/components/SignupForm.tsx",
                    "28-31",
                    "The phone field is gone from the form but `submit()` still sends `phone`: the API will get undefined. Drop it there too.",
                ),
                (
                    "/Users/demo/code/storefront/package-lock.json",
                    "",
                    "The lock file changed though no dependency was asked for: `npm install` upgraded 4 packages. Revert it unless that was meant.",
                ),
            ],
        );
    }
    journal.started(processes(now));
    journal.open()
}

/// Every thread the demo's agent keeps: the list as it says it, and each one's journal.
pub(super) fn threads(hooks: &DemoHooks, now: f64) -> (Vec<ThreadSummary>, HashMap<String, ThreadOpen>) {
    let mut opens = HashMap::new();
    opens.insert("d-landing".to_string(), landing(hooks, now));
    let done = |user: &str, calls: Vec<Call>, answer: &str, secs: f64| {
        let mut j = Journal::default();
        j.turn(user, None, calls, answer, now - 600.0, secs);
        j
    };
    opens.insert(
        "d-web1-tls".to_string(),
        with_insert(
            "d-web1-tls",
            "ssh:local",
            done(
                "Renew the certificates before they expire on Friday.",
                vec![tool("ssh_run", "certbot renew"), tool("ssh_run", "systemctl reload nginx")],
                "Renewed both certificates (until **January 22**) and reloaded nginx; the site answers over \
the new ones.",
                31.0,
            ),
        ),
    );
    opens.insert(
        "d-web1".to_string(),
        with_insert(
            "d-web1",
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
        ),
    );
    opens.insert(
        "d-site".to_string(),
        with_insert(
            "d-site",
            "browser:d1",
            done(
                "Open the blog's admin, make a draft from release-notes.md and stop before publishing.",
                vec![
                    tool("browser_tabs", "new"),
                    tool("browser_navigate", super::agent::BROWSER_URL),
                    tool("browser_fill_form", "title, body, tags"),
                    tool("browser_snapshot", ""),
                ],
                "The draft **Release 2.4: faster search** is in the editor with the text from \
`release-notes.md`, the tags *releases* and *search*, and a cover. It is saved as a draft; nothing \
is published.\n\nThe page is right here — click and type in it if something needs a touch. Say \
*publish* and I'll press the button.",
                48.0,
            ),
        ),
    );
    let subject = match hooks.tool.as_deref() {
        Some("html") => "tool:rich-text-to-markdown",
        Some("preview") => "tool:markdown-preview",
        Some("long") => "tool:base64",
        _ => "tool:cron-parser",
    };
    opens.insert(
        "d-tool".to_string(),
        with_insert(
            "d-tool",
            subject,
            done(
                "Is this right for every 15 minutes during work hours on weekdays?",
                vec![],
                "Yes: `*/15 9-18 * * 1-5` fires at :00, :15, :30 and :45 from 09:00 to 18:45, Monday to \
Friday — the next runs are listed above. If 18:45 is too late, make the hours `9-17`.",
                4.0,
            ),
        ),
    );
    // What the hooks ask of the thread the window opens on.
    if let Some(open) = hooks.opens().and_then(|id| opens.get_mut(id.as_str())) {
        if hooks.queue.is_some() {
            queue(open);
        }
        if hooks.clarify.is_some() {
            clarify(hooks, open);
        }
    }
    for open in opens.values_mut() {
        open.usage = Some(Usage {
            model: "Qwen3.8-Next-Flash".into(),
            context_used: 38_400,
            context_max: 131_072,
            calls: 14,
            input: 212_000,
            output: 9_800,
            judge_calls: Some(23),
            judge_nudges: Some(4),
            ..Default::default()
        });
    }

    const STORE: &str = "/Users/demo/code/storefront";
    const AUDIT: &str = "/Users/demo/Research/Competitor audit";
    let row = |id: &str, title: &str, project: Option<&str>, source: &str| ThreadSummary {
        id: id.into(),
        title: title.into(),
        source: source.into(),
        project: project.map(str::to_string),
        ..Default::default()
    };
    let in_store = |r: ThreadSummary| ThreadSummary {
        cwd: STORE.into(),
        repo: Some(STORE.into()),
        ..r
    };
    let in_audit = |r: ThreadSummary| ThreadSummary {
        cwd: AUDIT.into(),
        ..r
    };
    let rows = vec![
        in_store(row("d-landing", "Launch the spring landing page", Some("p-store"), "desktop")),
        ThreadSummary {
            parent: Some("d-landing".into()),
            ..row("d-landing-fork", "Launch the spring landing page (fork)", Some("p-store"), "desktop")
        },
        row("d-notes", "Summarize this week's meeting notes", Some("p-me"), "desktop"),
        // It asks to run a command: its row has the dot.
        ThreadSummary {
            waiting: true,
            ..in_store(row("d-api", "Add rate limiting to the public API", Some("p-store"), "desktop"))
        },
        in_store(row("d-tests", "Fix the flaky checkout test", Some("p-store"), "desktop")),
        ThreadSummary {
            running: true,
            ..in_audit(row("d-audit-1", "Competitors: pricing pages", Some("p-audit"), "desktop"))
        },
        in_audit(row("d-audit-2", "Competitors: onboarding flows", Some("p-audit"), "desktop")),
        // The Telegram and cron ones stay without a project.
        row("d-tg", "Invoice for March from the inbox", None, "telegram"),
        row("d-cron", "Daily digest of new issues", None, "cron"),
        row("d-web1-tls", "Renew the TLS certificates", Some("p-web"), "desktop"),
        row("d-web1", "Why is the site slow", Some("p-web"), "desktop"),
        row("d-site", "Publish the release notes", Some("p-blog"), "desktop"),
        row("d-tool", "Cron for work hours", Some("p-me"), "desktop"),
    ];
    (rows, opens)
}

/// `BENCH_DEMO_QUEUE=1`: two messages waiting after the last row, one the agent will take up next,
/// one a follow-up; a queue like this means a live run, and its plan shows in the work it belongs
/// to, after what the agent said aloud and the call it went on to make.
fn queue(open: &mut ThreadOpen) {
    open.queue = vec![
        protocol::QueuedItem {
            key: "q-1".into(),
            sub: "q-1".into(),
            mode: protocol::QueueMode::Steer,
            text: "And the cover: the one from the spring shoot.".into(),
            request: None,
        },
        protocol::QueuedItem {
            key: "q-2".into(),
            sub: "q-2".into(),
            mode: protocol::QueueMode::FollowUp,
            text: "Skip the pricing table for now, it goes in the next release.".into(),
            request: None,
        },
    ];
    open.running = true;
    open.items.extend([
        ThreadItem::Tool {
            key: "demo-live-1".into(),
            seq: u64::MAX - 5,
            call_id: "demo-live-c".into(),
            name: "bash".into(),
            args: serde_json::json!({ "command": "python3 bills/match.py --month oct" }),
            preview: "python3 bills/match.py --month oct".into(),
            at: None,
        },
        ThreadItem::Result {
            key: "demo-live-2".into(),
            seq: u64::MAX - 4,
            call_id: "demo-live-c".into(),
            name: "bash".into(),
            output: "3 unmatched".into(),
            failed: false,
            diff: None,
            at: None,
        },
        ThreadItem::Text {
            key: "demo-live-3".into(),
            // The words of a message that went on to call: narration, a note on the work line
            // (`rows_of`), exactly as the agent marks them.
            seq: u64::MAX - 2,
            text: "Reading the billing export, then matching it against the invoices.".into(),
            at: None,
            error: None,
            narration: Some(true),
        },
        ThreadItem::Tool {
            key: "demo-live-4".into(),
            seq: u64::MAX - 2,
            call_id: "demo-live-c2".into(),
            name: "bash".into(),
            args: serde_json::json!({ "command": "grep -c INV-2026 invoices/*.csv" }),
            preview: "grep -c INV-2026 invoices/*.csv".into(),
            at: None,
        },
        ThreadItem::Plan {
            key: "demo-plan".into(),
            seq: u64::MAX - 1,
            id: "demo-plan".into(),
            todos: ["done", "doing", "next"]
                .into_iter()
                .zip(["completed", "in_progress", "pending"])
                .enumerate()
                .map(|(k, (c, s))| protocol::PlanStep {
                    id: format!("{k}"),
                    content: format!("{c}: the step the agent wrote for itself"),
                    status: s.into(),
                    parent: None,
                })
                .collect(),
        },
    ]);
}

/// `BENCH_CLARIFY=1`: the agent's question — its journal item and the live prompt that lets it be
/// answered — inside the work that asked it, its choices long enough to show they wrap;
/// `=answered` once answered, as it stays in the thread.
fn clarify(hooks: &DemoHooks, open: &mut ThreadOpen) {
    let choices: Vec<String> = vec![
        "Навигация слева, как в System Settings: колонка ~180px с иконками и названиями разделов (Agents, Presets, Appearance), активный подсвечен акцентной полоской.".into(),
        "Настоящие табы сверху: иконка + название, активный с акцентным подчёркиванием, без рамок-кнопок. Меньше менять, но это всё ещё горизонтальная навигация.".into(),
        "Тихая боковая навигация как у сайдбара: те же строки, что в списке проектов (иконка + имя, акцентная полоска 2px у активного), отделена тонкой линией.".into(),
    ];
    open.items.extend([
        ThreadItem::Tool {
            key: "demo-ask-t".into(),
            seq: 9990,
            call_id: "demo-ask-t".into(),
            name: "ask".into(),
            args: serde_json::json!({}),
            preview: "ask".into(),
            at: None,
        },
        ThreadItem::Ask {
            key: "demo-ask".into(),
            seq: 9991,
            id: "demo".into(),
            ask_kind: "clarify".into(),
            question: "Как оформить разделы шестерни (Settings)?".into(),
            detail: None,
            choices: choices.clone(),
            multi: hooks.clarify_multi.is_some(),
        },
    ]);
    if hooks.clarify.as_deref() == Some("answered") {
        open.items.push(ThreadItem::Asked {
            key: "demo-asked".into(),
            seq: 9992,
            id: "demo".into(),
            answer: Some(choices[0].clone()),
        });
        return;
    }
    open.asks = vec![protocol::Ask {
        id: "demo".into(),
        kind: protocol::AskKind::Clarify,
        choices,
        ..Default::default()
    }];
}

/// The dev server the landing thread's agent left running (`process start`).
fn processes(now: f64) -> Vec<protocol::Process> {
    vec![protocol::Process {
        id: "41".into(),
        command: "npm run dev -- --port 5173".into(),
        state: protocol::ProcessState::Running,
        started: Some((now - 250.0) * 1000.),
        tail: "VITE v6.2  ready in 412 ms\n  ➜  Local:   http://localhost:5173/".into(),
        ..Default::default()
    }]
}
