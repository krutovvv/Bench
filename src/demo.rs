//! A made-up session for screenshots: `BENCH_DEMO=1 ./target/release/bench` (the end of the
//! answer), `BENCH_DEMO=top` (its beginning, with the agent's steps unfolded).
//!
//! No gateway, no Reminders, nothing of the person's: invented chats in the sidebar and one
//! open session that shows what the Hermes client can do — steps and a subagent, an answer with
//! a code block, a chart, a diagram, a formula, a video and a voice reply, its sources, the
//! files it changed, a process still running. The media live in `assets/demo/`.

use gpui_kit::*;

use crate::app::HermesApp;
use crate::model::{BgProcess, FileEdit, Place, Profile, SessionRow, Step, Target, Turn, Usage};

fn demo_file(name: &str) -> String {
    format!("{}/assets/demo/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn session(id: &str, title: &str, messages: u64, running: bool, source: &str) -> SessionRow {
    SessionRow {
        id: id.into(),
        title: title.into(),
        messages,
        model: "Qwen3.8-Next-Flash".into(),
        running,
        source: source.into(),
    }
}

fn tool(name: &str, detail: &str) -> Step {
    Step::Tool {
        name: name.into(),
        detail: detail.into(),
        failed: false,
        call_id: String::new(),
        args: String::new(),
        output: String::new(),
    }
}

/// Fill the window with the demo instead of connecting.
pub fn load(app: &mut HermesApp, cx: &mut Context<HermesApp>) {
    let now = crate::app::now_secs();
    app.demo = true;
    app.target = Target::Local;
    app.connecting = false;
    app.hermes_missing = false;
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

    let chart = demo_file("chart.png");
    let video = demo_file("preview.mp4");
    let voice = demo_file("summary.m4a");
    crate::media::warm(&video);
    let top = std::env::var("BENCH_DEMO").as_deref() == Ok("top");
    let answer = format!(
        "The new landing page is **live**, and the numbers so far look good.\n\n\
## What changed\n\n\
- A shorter hero: one promise, one button, the price right under it.\n\
- Social proof moved up: three customer quotes before the feature list.\n\
- The signup form lost two fields (company size, phone).\n\n\
Weekly signups, three weeks before and after:\n\n\
MEDIA:{chart}\n\n\
The flow the page now follows:\n\n\
```mermaid\ngraph LR\n  A[Visit] --> B[Hero]\n  B --> C[Quotes]\n  C --> D[Pricing]\n  D --> E[Signup]\n```\n\n\
Conversion went from $c_0 = 2.1\\%$ to $c_1 = 2.8\\%$, a lift of $\\frac{{c_1 - c_0}}{{c_0}} \\approx 33\\%$.\n\n\
To run it locally:\n\n\
```bash\nnpm install\nnpm run dev -- --port 5173\n```\n\n\
A 4-second preview of the hero animation, and the summary as a voice note:\n\n\
MEDIA:{video}\n\n\
MEDIA:{voice}\n"
    );
    app.turns = vec![Turn {
        user: "Ship the spring landing page we discussed: shorter hero, quotes first, two \
fewer form fields. Then check what it did to signups and give me a short summary."
            .into(),
        steps: vec![
            Step::Thought("Plan: edit the page, run the dev server, compare signups.".into()),
            tool("read_file", "src/pages/landing.tsx"),
            tool("patch", "src/pages/landing.tsx"),
            tool("patch", "src/components/SignupForm.tsx"),
            tool("terminal", "npm run dev -- --port 5173"),
            tool("browser_navigate", "http://localhost:5173"),
            Step::Subagent {
                id: "sub-1".into(),
                goal: "Pull six weeks of signups from analytics".into(),
                status: "completed".into(),
                lines: vec!["web_search  analytics export API".into(), "terminal  export.csv".into()],
                summary: "6 weeks exported".into(),
            },
            tool("execute_code", "chart of weekly signups"),
            tool("text_to_speech", "summary.m4a"),
        ],
        answer,
        files: vec![
            FileEdit {
                path: "/Users/demo/code/storefront/src/pages/landing.tsx".into(),
                diff: Some(
                    "@@ -12,9 +12,7 @@\n-  <Hero title={long} subtitle={sub} />\n+  <Hero title=\"Ship faster\" />\n+  <Quotes />\n   <Features />\n-  <Quotes />\n".into(),
                ),
            },
            FileEdit {
                path: "/Users/demo/code/storefront/src/components/SignupForm.tsx".into(),
                diff: Some("@@ -30,6 +30,4 @@\n-  <Field name=\"companySize\" />\n-  <Field name=\"phone\" />\n".into()),
            },
        ],
        sources: vec![
            "https://www.nngroup.com/articles/landing-page-guidelines/".into(),
            "https://developer.mozilla.org/en-US/docs/Web/Performance".into(),
        ],
        started: Some(now - 312.0),
        finished: Some(now - 60.0),
        expanded: top,
        ..Default::default()
    }];
    // The beginning of the answer: once the first frames have laid the chat out (and media
    // loading has pulled it to the bottom), scroll back to the top and stop following the tail.
    if top {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.follow = false;
                this.chat_scroll.scroll_to_top_of_item(0);
                cx.notify();
            });
        })
        .detach();
    }
    app.session = Some("d-landing".into());
    app.processes = vec![BgProcess {
        id: "p-1".into(),
        command: "npm run dev -- --port 5173".into(),
        running: true,
        exit_code: None,
        started: Some(now - 250.0),
        exited: None,
        reason: None,
        tail: "VITE v6.2  ready in 412 ms\n  ➜  Local:   http://localhost:5173/".into(),
    }];
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
