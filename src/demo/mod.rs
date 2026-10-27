//! A made-up session for screenshots and the web demo: `BENCH_DEMO=1 ./target/release/bench`
//! opens the browser thread (the centrepiece); `BENCH_DEMO=tasks|server|tool` another subject's
//! thread; `BENCH_DEMO=landing` the landing-page chat at the end of its answer, `BENCH_DEMO=top`
//! at its beginning with the agent's steps unfolded.
//!
//! No gateway, nothing of the person's: invented chats in the sidebar and one
//! open session that shows what the agent's client can do — steps and a subagent, an answer with
//! a code block, a chart, a diagram, a formula, a video and a voice reply, its sources, the
//! files it changed, a process still running. The media live in `assets/demo/`.

mod env;
mod inserts;
mod journal;
mod threads;

pub use env::{env, file_text, listing};
pub use inserts::terminal;
pub use threads::processes;

use inserts::other_modes;
use journal::*;
use threads::subject_threads;

use gpui_kit::*;

use crate::app::BenchApp;
use crate::app::model::{Place, Profile, Target, Usage};
use protocol::ThreadItem;
use serde_json::json;

/// The blog project's environment, for its settings and the Explorer (any project the demo
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
    let project = |id: &str, name: &str, folder: Option<&str>| crate::app::model::ProjectEntry {
        id: id.into(),
        name: name.into(),
        folder: folder.map(str::to_string),
        agent: String::new(),
        machine: None,
        browser: true,
    };
    // The production site: worked on over SSH, on web-1 (its terminal is that server's).
    let web = crate::app::model::ProjectEntry {
        machine: Some("web-1".into()),
        ..project("p-web", "Production site", Some("/var/www/storefront"))
    };
    app.projects = vec![
        project("p-blog", "Northwind blog", Some("/Users/demo/code/blog")),
        project("p-store", "Storefront", Some("/Users/demo/code/storefront")),
        web,
        project("p-audit", "Competitor audit", None),
        project("p-me", "Personal", None),
    ];
    for s in app.sessions.iter_mut() {
        s.project = match s.id.as_str() {
            "d-site" => "p-blog",
            "d-landing" | "d-api" | "d-tests" => "p-store",
            "d-web1" | "d-web1-tls" => "p-web",
            "d-audit-1" | "d-audit-2" => "p-audit",
            "d-today" | "d-tool" | "d-notes" => "p-me",
            _ => "",
        }
        .into();
    }
    // The demo's own marks, not the person's: two projects in colors, a thread in one.
    app.marks = [("p:p-blog", 4), ("p:p-audit", 1), ("d-api", 2)]
        .into_iter()
        .map(|(k, c)| {
            (
                k.to_string(),
                crate::sidebar::marks::Mark {
                    color: Some(c),
                    pinned: false,
                },
            )
        })
        .collect();
    // The demo's own folds, not the person's: every group open but the long Tools.
    app.ssh.collapsed = ["s:Tools".to_string()].into_iter().collect();
    // `BENCH_DEMO=tasks|server|browser|tool`: that thread open instead.
    let open = match std::env::var("BENCH_DEMO").as_deref() {
        Ok("tasks") => Some("d-today"),
        Ok("server") => Some("d-web1"),
        Ok("tool") => Some("d-tool"),
        Ok("landing") | Ok("top") | Ok("new") | Ok("long") => None,
        _ => Some("d-site"),
    };
    if let Some(id) = open {
        app.session = Some(id.into());
        app.thread = app.demo_threads.get(id).cloned().unwrap_or_default();
        app.thread_project = app
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.project.clone())
            .unwrap_or_default();
    }
    // `BENCH_DEMO=long`: a thread of sixty exchanges, opened at its top.
    if std::env::var("BENCH_DEMO").as_deref() == Ok("long") {
        let mut journal = Journal::default();
        for k in 0..60 {
            let answer = format!("Answer {k}. ")
                + &"The numbers held steady through the week. ".repeat(8 + k % 5 * 6);
            journal.turn(
                &format!("Question {k}?"),
                None,
                vec![],
                &answer,
                now - 7200. + k as f64 * 100.,
                30.,
            );
        }
        app.session = Some("d-landing".into());
        app.thread = journal.thread();
        app.processes.clear();
    }
    // `BENCH_DEMO_JUMP=1`: "Jump to latest" pressed two seconds after opening.
    if std::env::var("BENCH_DEMO_JUMP").is_ok() {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            this.update(cx, |app, cx| app.jump_to_latest(cx)).ok();
        })
        .detach();
    }
    // `BENCH_DEMO=new`: a new thread, nothing in it but the composer.
    if std::env::var("BENCH_DEMO").as_deref() == Ok("new") {
        app.session = None;
        app.thread = Default::default();
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
    if std::env::var("BENCH_DEMO_RUN").is_ok() {
        run(cx);
    }
    // `BENCH_PANEL=providers` (+ `BENCH_GEAR_TAB=<0|1|2>`): the gear open, for screenshots.
    // The new project's page has its own hook: `BENCH_DEMO=new-project` (thread/mod.rs).
    // `BENCH_CLARIFY=1` (with `BENCH_CLARIFY_MULTI=1`) over a demo thread: the agent's question
    // card, its choices long enough to show they wrap.
    if std::env::var("BENCH_CLARIFY").is_ok() {
        app.clarify = Some(crate::app::model::ClarifyReq {
            session: app.session.clone().unwrap_or_default(),
            prompt_id: "demo".into(),
            generation: None,
            question: "Как оформить разделы шестерни (Settings)?".into(),
            choices: vec![
                "Навигация слева, как в System Settings: колонка ~180px с иконками и названиями разделов (Agents, Presets, Appearance), активный подсвечен акцентной полоской.".into(),
                "Настоящие табы сверху: иконка + название, активный с акцентным подчёркиванием, без рамок-кнопок. Меньше менять, но это всё ещё горизонтальная навигация.".into(),
                "Тихая боковая навигация как у сайдбара: те же строки, что в списке проектов (иконка + имя, акцентная полоска 2px у активного), отделена тонкой линией.".into(),
            ],
            multi_select: std::env::var("BENCH_CLARIFY_MULTI").is_ok(),
        });
    }
    // `BENCH_DEMO=explorer`: the blog in the Explorer (`env` opens its documents); `BENCH_DEMO=env`:
    // the same, its settings open.
    match std::env::var("BENCH_DEMO").as_deref() {
        Ok("explorer") | Ok("env") => {
            app.sidebar_panel = crate::workspace::SidebarPanel::Explorer;
            app.explorer.project = "p-blog".into();
        }
        _ => {}
    }
    if std::env::var("BENCH_PANEL").as_deref() == Ok("providers") {
        app.gear_tab = std::env::var("BENCH_GEAR_TAB")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        app.center = crate::workspace::Center::Providers;
    }
    cx.notify();
}

/// `BENCH_DEMO_RUN=1`: a run plays in the open thread as the agent sends it — a message, the
/// model thinking, a tool, the answer streamed, its entry, the run's end — for watching the
/// thread hold still while it is written.
fn run(cx: &mut Context<BenchApp>) {
    use protocol::{StreamKind, ThreadEvent};
    cx.spawn(async move |this, cx| {
        let now = crate::app::now_secs() * 1000.;
        let mut events = vec![
            (
                1500,
                ThreadEvent::Append(vec![ThreadItem::User {
                    key: "900.0".into(),
                    seq: 900,
                    text: "And the bounce rate?".into(),
                    at: Some(now),
                }]),
            ),
            (100, ThreadEvent::Run(true)),
        ];
        for _ in 0..10 {
            events.push((
                80,
                ThreadEvent::Stream(StreamKind::Thought, "Looking at the numbers. ".into()),
            ));
        }
        events.push((
            300,
            ThreadEvent::Append(vec![
                ThreadItem::Thought {
                    key: "901.0".into(),
                    seq: 901,
                    text: "Looking at the numbers. ".repeat(10),
                    at: Some(now),
                },
                ThreadItem::Tool {
                    key: "901.1".into(),
                    seq: 901,
                    call_id: "c901".into(),
                    name: "web_fetch".into(),
                    args: json!({}),
                    preview: "analytics".into(),
                    at: Some(now),
                },
            ]),
        ));
        events.push((
            600,
            ThreadEvent::Append(vec![ThreadItem::Result {
                key: "902.0".into(),
                seq: 902,
                call_id: "c901".into(),
                name: "web_fetch".into(),
                output: "ok".into(),
                failed: false,
                diff: None,
                at: Some(now),
            }]),
        ));
        let line = "The bounce rate fell from 61% to 48% in the three weeks since the launch. ";
        for k in 0..60 {
            let delta = if k % 6 == 5 {
                format!("{line}\n\n")
            } else {
                line.to_string()
            };
            events.push((60, ThreadEvent::Stream(StreamKind::Text, delta)));
        }
        let text = (0..10)
            .map(|_| line.repeat(6))
            .collect::<Vec<_>>()
            .join("\n\n");
        events.push((
            300,
            ThreadEvent::Append(vec![ThreadItem::Text {
                key: "903.0".into(),
                seq: 903,
                text,
                at: Some(now),
                error: None,
            }]),
        ));
        events.push((100, ThreadEvent::Run(false)));
        for (ms, e) in events {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(ms))
                .await;
            eprintln!(
                "demo-run: {}",
                match &e {
                    ThreadEvent::Append(_) => "append",
                    ThreadEvent::Stream(..) => "stream",
                    ThreadEvent::Run(true) => "run start",
                    ThreadEvent::Run(false) => "run end",
                    _ => "other",
                }
            );
            if this
                .update(cx, |app, cx| {
                    app.thread.apply(e);
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
        }
    })
    .detach();
}
