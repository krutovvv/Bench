//! A made-up session for screenshots and the web demo: `BENCH_DEMO=1 ./target/release/bench`
//! opens the browser thread (the centrepiece); `BENCH_DEMO=server|tool` another subject's
//! thread; `BENCH_DEMO=landing` the landing-page chat at the end of its answer, `BENCH_DEMO=top`
//! at its beginning with the agent's steps unfolded.
//!
//! No agent, nothing of the person's: invented chats in the sidebar and one
//! open session that shows what the agent's client can do — steps and a subagent, an answer with
//! a code block, a chart, a diagram, a formula, a video and a voice reply, its sources, the
//! files it changed, a process still running. The media live in `assets/demo/`.

mod drop;
mod env;
mod inserts;
mod journal;
mod thread;
mod threads;

pub use env::{env, file_head, file_problems, file_text, ignored, listing};
pub use inserts::terminal;

use inserts::other_modes;
use journal::*;
use threads::subject_threads;

use gpui_kit::*;

use crate::app::BenchApp;
use protocol::Usage;
use protocol::ThreadItem;
use serde_json::json;

/// The blog project's environment, for its settings and the Explorer (any project the demo
/// Fill the window with the demo instead of connecting.
impl BenchApp {
    /// A demo thread in the center's view, as if it were opened (nothing is fetched).
    pub(crate) fn show_demo_thread(&mut self, id: &str, cx: &mut App) {
        let journal = self.demo_threads.get(id).cloned().unwrap_or_default();
        open_thread(self, cx, |t| {
            t.id = Some(id.into());
            t.journal = journal;
            t.loaded = true;
        });
    }
}

/// The open thread's state, set up (before anything is drawn).
fn open_thread(app: &mut BenchApp, cx: &mut App, f: impl FnOnce(&mut crate::thread::ActiveThread)) {
    app.active_thread.update(cx, |v, _| f(&mut v.thread));
}

pub fn load(app: &mut BenchApp, cx: &mut Context<BenchApp>) {
    let now = crate::app::now_secs();
    app.demo = true;
    app.defaults.ready = true;
    app.defaults.context_length = 131_072;
    app.defaults.approval_mode = Some("smart".into());
    app.defaults.cwd = Some("/Users/demo".into());

    app.threads.list = vec![
        row(
            "d-landing",
            "Launch the spring landing page",
            false,
            "desktop",
        ),
        row(
            "d-notes",
            "Summarize this week's meeting notes",
            false,
            "desktop",
        ),
        row(
            "d-api",
            "Add rate limiting to the public API",
            false,
            "desktop",
        ),
        row(
            "d-tests",
            "Fix the flaky checkout test",
            false,
            "desktop",
        ),
        row(
            "d-audit-1",
            "Competitors: pricing pages",
            true,
            "desktop",
        ),
        row(
            "d-audit-2",
            "Competitors: onboarding flows",
            false,
            "desktop",
        ),
        row(
            "d-tg",
            "Invoice for March from the inbox",
            false,
            "telegram",
        ),
        row("d-cron", "Daily digest of new issues", false, "cron"),
    ];
    // Where the demo's threads work: a folder, and the repo it is in.
    for (id, cwd, repo) in [
        ("d-landing", "/Users/demo/code/storefront", Some("/Users/demo/code/storefront")),
        ("d-api", "/Users/demo/code/storefront", Some("/Users/demo/code/storefront")),
        ("d-tests", "/Users/demo/code/storefront", Some("/Users/demo/code/storefront")),
        ("d-audit-1", "/Users/demo/Research/Competitor audit", None),
        ("d-audit-2", "/Users/demo/Research/Competitor audit", None),
    ] {
        if let Some(r) = app.threads.list.iter_mut().find(|r| r.id.as_str() == id) {
            r.cwd = cwd.into();
            r.repo = repo.map(str::to_string);
        }
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
    journal.started(threads::processes(now));
    let landing = journal.thread();
    if std::env::var("BENCH_DEMO").as_deref() == Ok("top") {
        if let Some(::thread::Row::Work { key, .. }) = landing
            .rows()
            .into_iter()
            .find(|r| matches!(r, ::thread::Row::Work { .. }))
        {
            open_thread(app, cx, |t| {
                t.open_work.insert(key);
            });
        }
    }
    // `BENCH_DEMO_FILES=1`: the changed files' list unfolded.
    if std::env::var("BENCH_DEMO_FILES").is_ok() {
        open_thread(app, cx, |t| t.files_open = true);
    }
    // `BENCH_DEMO_PROCS=1`: the background processes' card unfolded, its output shown.
    if std::env::var("BENCH_DEMO_PROCS").is_ok() {
        if let Some(::thread::Row::Work { key, .. }) = landing
            .rows()
            .into_iter()
            .find(|r| matches!(r, ::thread::Row::Work { processes, .. } if !processes.is_empty()))
        {
            open_thread(app, cx, |t| {
                t.processes_open.insert(key);
                t.process_open = Some("41".into());
            });
        }
    }
    app.demo_threads.insert("d-landing".into(), landing.clone());
    open_thread(app, cx, |t| {
        t.journal = landing;
        t.id = Some("d-landing".into());
        t.loaded = true;
        t.repo = Some(crate::app::repo::Repo::from_top("/Users/demo/code/storefront", "main"));
    });
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
        telegram: false,
    };
    // The production site: worked on over SSH, on web-1 (its terminal is that server's).
    let web = crate::app::model::ProjectEntry {
        machine: Some("web-1".into()),
        ..project("p-web", "Production site", Some("/var/www/storefront"))
    };
    app.places.projects = vec![
        // The blog's bot takes its readers' questions in Telegram.
        crate::app::model::ProjectEntry {
            telegram: true,
            ..project("p-blog", "Northwind blog", Some("/Users/demo/code/blog"))
        },
        project("p-store", "Storefront", Some("/Users/demo/code/storefront")),
        web,
        project("p-audit", "Competitor audit", None),
        project("p-me", "Personal", None),
    ];
    for s in app.threads.list.iter_mut() {
        // The rate limiting asks to run a command: its row has the dot.
        s.waiting = s.id.as_str() == "d-api";
        s.project = match s.id.as_str() {
            "d-site" => "p-blog",
            "d-landing" | "d-api" | "d-tests" => "p-store",
            "d-web1" | "d-web1-tls" => "p-web",
            "d-audit-1" | "d-audit-2" => "p-audit",
            "d-tool" | "d-notes" => "p-me",
            _ => "",
        }
        .into();
    }
    // The demo's own marks, not the person's: two projects in colors, a thread in one.
    app.sidebar.marks = [("p:p-blog", 4), ("p:p-audit", 1), ("d-api", 2)]
        .into_iter()
        .map(|(k, c)| {
            (
                k.to_string(),
                crate::sidebar::marks::Mark {
                    color: Some(c),
                    pinned: k == "d-api",
                },
            )
        })
        .collect();
    // `BENCH_DEMO_ARM=<key>`: that delete waits for its second press (`thread:d-api`, `p:p-web`).
    if let Ok(key) = std::env::var("BENCH_DEMO_ARM") {
        app.armed.arm(key);
    }
    // The demo's own folds, not the person's: every group open but the long Tools.
    app.sidebar.collapsed = ["s:Tools".to_string()].into_iter().collect();
    // `BENCH_DEMO=server|browser|tool`: that thread open instead.
    let open = match std::env::var("BENCH_DEMO").as_deref() {
        Ok("server") => Some("d-web1"),
        Ok("tool") => Some("d-tool"),
        Ok("landing") | Ok("top") | Ok("new") | Ok("long") => None,
        _ => Some("d-site"),
    };
    if let Some(id) = open {
        let journal = app.demo_threads.get(id).cloned().unwrap_or_default();
        let project = app.project_of_thread("", &id.into());
        open_thread(app, cx, |t| {
            t.id = Some(id.into());
            t.journal = journal;
            t.project = project;
            t.repo = None;
        });
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
        open_thread(app, cx, |t| {
            t.id = Some("d-landing".into());
            t.journal = journal.thread();
        });
    }
    // `BENCH_DEMO_JUMP=1`: "Jump to latest" pressed two seconds after opening.
    if std::env::var("BENCH_DEMO_JUMP").is_ok() {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            this.update(cx, |app, cx| {
                app.active_thread.update(cx, |v, cx| v.jump_to_latest(cx))
            })
            .ok();
        })
        .detach();
    }
    // `BENCH_DEMO_NEW=1`: the dock panel's "+" pressed two seconds after opening — with
    // `BENCH_DEMO=explorer BENCH_EXPLORER=thread` it shows a new thread in the dock while
    // the document keeps the middle.
    if std::env::var("BENCH_DEMO_NEW").is_ok() {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            this.update_in(cx, |app, window, cx| {
                app.new_thread_in_panel(cx);
                app.focus_composer(window, cx);
            })
            .ok();
        })
        .detach();
    }
    // `BENCH_DEMO_TYPE=1` writes a line into the composer two seconds in (with
    // `BENCH_TOOLS_SHOWN=1` it shows the composer's hover tools where they sit with text).
    if std::env::var("BENCH_DEMO_TYPE").is_ok() {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            this.update_in(cx, |app, window, cx| {
                app.focus_composer(window, cx);
                app.active_thread.update(cx, |v, cx| {
                    v.composer.update(cx, |s, cx| {
                        s.set_value(
                            "Check the staging deploy and tell me whether it is healthy.\n\
                                 \n\
                                 Second paragraph, long enough to reach the very right of the \
                                 field, where the tools' gutter keeps the paperclip and the mic \
                                 clear of every word.",
                            window,
                            cx,
                        );
                    })
                });
            })
            .ok();
        })
        .detach();
    }
    // `BENCH_DEMO_SEND=1` (with `BENCH_DEMO_TYPE=1`) presses Send three seconds in: the
    // message goes to the queue of the running demo thread and its waiting row must show
    // the same frame, before the (absent) agent could ever answer.
    if std::env::var("BENCH_DEMO_SEND").is_ok() {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(3))
                .await;
            // Deferred a beat off the app's own update: Send reads the app, and it must
            // not be borrowed already.
            this.update_in(cx, |app, window, cx| {
                let at = app.active_thread.clone();
                window.defer(cx, move |window, cx| {
                    at.update(cx, |v, cx| {
                        if v.thread.live.is_none() {
                            v.thread.live = Some(crate::app::Live {
                                label: "Thinking…".into(),
                            });
                        }
                        v.send(window, cx);
                    });
                });
            })
            .ok();
        })
        .detach();
    }
    // `BENCH_DEMO=new`: a new thread, nothing in it but the composer.
    if std::env::var("BENCH_DEMO").as_deref() == Ok("new") {
        open_thread(app, cx, |t| {
            t.id = None;
            t.journal = Default::default();
            t.repo = None;
        });
    }
    open_thread(app, cx, |t| {
        t.journal.apply(protocol::ThreadEvent::Usage(Usage {
            model: "Qwen3.8-Next-Flash".into(),
            context_used: 38_400,
            context_max: 131_072,
            calls: 14,
            input: 212_000,
            output: 9_800,
            ..Default::default()
        }))
    });
    if std::env::var("BENCH_DEMO_RUN").is_ok() {
        run(cx);
    }
    // `BENCH_PANEL=providers` (+ `BENCH_GEAR_TAB=<0|1|2>`): the gear open, for screenshots.
    // The new project's page has its own hook: `BENCH_DEMO=new-project` (thread/mod.rs).
    // `BENCH_CLARIFY=1` (with `BENCH_CLARIFY_MULTI=1`) over a demo thread: the agent's question
    // — its journal item and the live prompt that lets it be answered — inside the work that
    // asked it, its choices long enough to show they wrap.
    if std::env::var("BENCH_CLARIFY").is_ok() {
        let multi = std::env::var("BENCH_CLARIFY_MULTI").is_ok();
        let question = "Как оформить разделы шестерни (Settings)?";
        let choices: Vec<String> = vec![
            "Навигация слева, как в System Settings: колонка ~180px с иконками и названиями разделов (Agents, Presets, Appearance), активный подсвечен акцентной полоской.".into(),
            "Настоящие табы сверху: иконка + название, активный с акцентным подчёркиванием, без рамок-кнопок. Меньше менять, но это всё ещё горизонтальная навигация.".into(),
            "Тихая боковая навигация как у сайдбара: те же строки, что в списке проектов (иконка + имя, акцентная полоска 2px у активного), отделена тонкой линией.".into(),
        ];
        let journal_choices = choices.clone();
        open_thread(app, cx, move |t| {
            t.journal.apply(protocol::ThreadEvent::Append(vec![
                protocol::ThreadItem::Tool {
                    key: "demo-ask-t".into(),
                    seq: 9990,
                    call_id: "demo-ask-t".into(),
                    name: "ask".into(),
                    args: serde_json::json!({}),
                    preview: "ask".into(),
                    at: None,
                },
                protocol::ThreadItem::Ask {
                    key: "demo-ask".into(),
                    seq: 9991,
                    id: "demo".into(),
                    ask_kind: "clarify".into(),
                    question: question.into(),
                    detail: None,
                    choices: journal_choices,
                    multi,
                },
            ]));
            t.journal.apply(protocol::ThreadEvent::Asks(vec![protocol::Ask {
                id: "demo".into(),
                kind: protocol::AskKind::Clarify,
                choices,
                ..Default::default()
            }]));
        });
    }
    // `BENCH_DEMO=explorer`: the blog in the Explorer (`env` opens its documents); `BENCH_DEMO=env`:
    // the same, its settings open.
    match std::env::var("BENCH_DEMO").as_deref() {
        Ok("explorer") | Ok("env") => {
            app.left_dock.activate(crate::workspace::Panel::Explorer);
            app.explorer.project = "p-blog".into();
            // `BENCH_DEMO_SIDE=threads`: the tree's place given to the project's threads.
            app.explorer.threads = std::env::var("BENCH_DEMO_SIDE").as_deref() == Ok("threads");
        }
        _ => {}
    }
    if std::env::var("BENCH_PANEL").as_deref() == Ok("providers") {
        app.gear.tab = std::env::var("BENCH_GEAR_TAB")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        app.center = crate::workspace::Center::Settings;
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
                    sub: None,
                    request: None,
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
                    key: "901.2".into(),
                    seq: 901,
                    call_id: "c901".into(),
                    name: "web_fetch".into(),
                    args: json!({}),
                    preview: "analytics".into(),
                    at: Some(now),
                },
            ]),
        ));
        // Words the model wrote between its thinking and the call: the line of work shows
        // this narrative, with the newest call's kind after a dot.
        events.push((
            400,
            ThreadEvent::Append(vec![ThreadItem::Text {
                key: "901.1".into(),
                seq: 901,
                text: "Checking the traffic splits, then rewriting the query.".into(),
                at: Some(now),
                error: None,
                narration: Some(true),
            }]),
        ));
        // BENCH_DEMO_PAUSED=1 holds the run mid-work, for the hold's picture.
        if std::env::var("BENCH_DEMO_PAUSED").is_ok() {
            let held = now + 400.;
            events.push((
                400,
                ThreadEvent::Holds(vec![::thread::Hold {
                    from: held,
                    to: None,
                }]),
            ));
            events.push((
                6000,
                ThreadEvent::Holds(vec![::thread::Hold {
                    from: held,
                    to: Some(held + 6000.),
                }]),
            ));
        }
        events.push((
            3000,
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
                narration: Some(false),
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
                    app.active_thread.update(cx, |v, cx| {
                        // As a real stream: the run's label, the speed meter, the journal.
                        v.thread.on_thread_event(&e);
                        cx.notify();
                    })
                })
                .is_err()
            {
                return;
            }
        }
    })
    .detach();
}
