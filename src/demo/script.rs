//! What the hooks ask of the window, done from outside as the person would have: first what is the
//! window's own (marks, folds), then, when the agent's answers are in, the thread opened and a
//! picture's state set; then the presses a few seconds in.

use std::time::Duration;

use gpui_kit::*;
use protocol::{StreamKind, ThreadEvent, ThreadItem};

use super::hooks::DemoHooks;
use crate::app::BenchApp;
use crate::thread::{ActiveThread, ThreadView};

type Cx<'a> = AsyncWindowContext;

/// The demo's marks and folds, not the person's, and what shows from the first frame.
fn seed(hooks: &DemoHooks, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    // Two projects in colors, a thread in one.
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
    app.sidebar.collapsed = ["s:Tools".to_string()].into_iter().collect();
    if let Some(key) = &hooks.arm {
        app.armed.arm(key.clone());
    }
    app.sidebar.held_drop = hooks.drop_line.as_ref().and_then(|spec| {
        let (group, edge) = spec.split_once(':')?;
        let edge = if edge == "above" { bench_ui::DropEdge::Above } else { bench_ui::DropEdge::Below };
        Some((group.parse().ok()?, edge))
    });
    if hooks.pop.as_deref() == Some("agent") {
        app.sidebar.agent_popover_open = true;
        app.sidebar.agent_log = crate::client::fake::fake().map(|f| f.agent_log()).unwrap_or_default();
    }
    let view = app.active_thread.clone();
    let shown = hooks.tools_shown.is_some();
    let prefill = hooks.prefill.clone();
    view.update(cx, |v, cx| {
        v.tools_shown = shown;
        if let Some(text) = prefill {
            v.composer.update(cx, |s, cx| s.set_value(text, window, cx));
        }
    });
    // The tools insert's input: a cron line; a long paste or a page to preview where asked.
    let tools = &mut app.inserts.tools;
    tools.inputs.insert(devtools::Tool::Cron, "*/15 9-18 * * 1-5".into());
    match hooks.tool.as_deref() {
        Some("long") => {
            let page = (0..120)
                .map(|n| format!("<div class=\"post\">{n} <!DOCTYPE html><h1>413 Request Entity Too Large</h1> openresty/1.15.8.2</div>"))
                .collect::<Vec<_>>()
                .join("\n");
            tools.inputs.insert(devtools::Tool::Base64, page.clone());
            tools.preview = page;
        }
        Some(which @ ("html" | "preview")) => {
            let lines = (1..=40)
                .map(|n| format!("{n}. A line of the page, long enough to fill the pane's width."))
                .collect::<Vec<_>>()
                .join("\n");
            let tool = if which == "html" { devtools::Tool::HtmlToMarkdown } else { devtools::Tool::MarkdownPreview };
            tools.inputs.insert(tool, lines.clone());
            tools.preview = lines;
        }
        _ => {}
    }
}

/// The open thread's state, once it has loaded: what the hooks open in it.
fn open_in_thread(hooks: &DemoHooks, t: &mut ActiveThread) {
    let rows = t.journal.rows();
    let works = || rows.iter().filter(|r| matches!(r, ::thread::Row::Work { .. }));
    if let Some(how) = &hooks.work {
        let mut parts = how.split(':');
        let n: usize = parts.next().and_then(|n| n.parse().ok()).unwrap_or(1);
        let unfold: Vec<usize> = parts.filter_map(|i| i.parse().ok()).collect();
        let calls = hooks.work_calls.as_ref().and_then(|c| c.parse::<usize>().ok());
        let work = works()
            .filter(|r| match calls {
                Some(c) => matches!(r, ::thread::Row::Work { steps, .. } if steps.iter().filter(|s| s.is_tool()).count() == c),
                None => true,
            })
            .nth(n.saturating_sub(1));
        if let Some(::thread::Row::Work { key, steps, .. }) = work {
            t.open_work.insert(key.clone());
            // The i-th call (the list counts the agent's words and reasoning too).
            let listed: Vec<usize> = (0..steps.len()).filter(|k| steps[*k].is_tool()).collect();
            if let Some(k) = unfold.first().and_then(|i| listed.get(i.saturating_sub(1))) {
                if hooks.card.as_deref() != Some("0") {
                    t.open_steps.insert((key.clone(), *k));
                }
                for m in ::thread::steps::milestones(steps) {
                    if m.steps.contains(k) {
                        t.open_milestone.insert(key.clone(), m.steps[0]);
                    }
                }
            }
        }
    }
    if hooks.mode() == "top" {
        if let Some(::thread::Row::Work { key, .. }) = works().next() {
            t.open_work.insert(key.clone());
        }
    }
    let changes = rows.iter().find_map(|r| match r {
        ::thread::Row::Changes { key, files, .. } => Some((key.clone(), files[0].path.clone())),
        _ => None,
    });
    if let (Some(how), Some((key, first))) = (&hooks.files, changes.clone()) {
        t.files_open.insert(key.clone());
        if how == "diff" {
            t.diffs_open.insert((key.clone(), first.clone()));
            t.diffs.insert(
                (key, first.clone()),
                Some(crate::explorer::code::git::unified(
                    first.rsplit('/').next().unwrap_or(&first),
                    &super::env::turn_file("demo-base", &first).unwrap_or_default(),
                    &super::env::turn_file("demo-head", &first).unwrap_or_default(),
                )),
            );
        }
    }
    if hooks.review.is_some() {
        if let Some((key, _)) = changes {
            t.files_open.insert(key.clone());
            t.review_open.insert(key);
        }
    }
    if hooks.procs.is_some() {
        let work = rows.iter().find_map(|r| match r {
            ::thread::Row::Work { key, processes, .. } if !processes.is_empty() => Some(key.clone()),
            _ => None,
        });
        if let Some(key) = work {
            t.processes_open.insert(key);
            t.process_open = Some("41".into());
        }
    }
}

/// `BENCH_DEMO=explorer`: the blog's tree open on a post, the soul and a skill, a file open, and
/// what `BENCH_EXPLORER` asks for. `BENCH_DEMO=env` is that with its settings open.
fn open_explorer(hooks: &DemoHooks, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    use crate::explorer::{DocKey, FileRef};
    use crate::thread::composer::attachments::{CodeRef, CommentRef, Reference};
    let dir = "~/.bench/projects/p-blog".to_string();
    let root = FileRef {
        project: "p-blog".into(),
        machine: None,
        path: "/Users/demo/code/blog".into(),
    };
    let skill = app.agent_file(format!("{dir}/skills/release-notes"));
    for f in [root.clone(), root.join("content"), root.join("content/posts"), skill] {
        app.explorer.open.insert(format!("d:{}", f.listing_key()));
        app.list_folder(f, window, cx);
    }
    app.explorer.open.extend(
        ["files", "instructions", "skills", "g:Tools", "s:mcp", "g:Schedule", "s:cron"].map(String::from),
    );
    let post = root.join("content/posts/release-2-4.md");
    app.open_doc(DocKey::File(app.agent_file(format!("{dir}/SYSTEM.md"))), window, cx);
    app.open_doc(DocKey::File(post.clone()), window, cx);
    let explorer = if hooks.mode() == "env" { "settings" } else { hooks.explorer.as_deref().unwrap_or_default() };
    match explorer {
        "settings" => app.open_doc(DocKey::Settings { project: "p-blog".into() }, window, cx),
        "part" => app.open_part("p-blog".into(), "mcp", "ghost".into(), window, cx),
        "lsp" => app.open_part("p-blog".into(), "lsp", "pyright".into(), window, cx),
        "lsp-new" => app.open_part("p-blog".into(), "lsp", "new".into(), window, cx),
        "diff" => app.open_diff(post, window, cx),
        // The project's terminal under the post; `explorer_later` sends its lines to the thread.
        "terminal" => {
            let project = app.terminal_project(cx);
            app.inserts.dock_terms.add(&project);
            app.inserts.dock_terms.add(&project);
            app.bottom_dock.activate(crate::workspace::Panel::Terminal);
        }
        "problems" => app.open_problems(window, cx),
        // The thread beside the post, lines of it sent to the composer (⌘L).
        "thread" => {
            app.right_dock.activate(crate::workspace::Panel::Thread);
            app.add_reference(
                Reference::Code(CodeRef {
                    path: "/Users/demo/code/blog/content/posts/release-2-4.md".into(),
                    lines: Some((11, 15)),
                    text: Some("## What changed".into()),
                }),
                cx,
            );
        }
        // The specs: the group open, the posts' spec read in its tab.
        "specs" => {
            app.explorer.open.extend(["specs".to_string(), "spec:blog".to_string()]);
            app.ensure_specs(window, cx);
        }
        // Review comments: one already in the composer, the next being written under the post.
        "comment" => {
            app.right_dock.activate(crate::workspace::Panel::Thread);
            app.add_reference(
                Reference::Comment(CommentRef {
                    id: "c3f1".into(),
                    code: CodeRef {
                        path: "/Users/demo/code/blog/content/posts/release-2-4.md".into(),
                        lines: Some((3, 3)),
                        text: Some("date: 2026-10-20".into()),
                    },
                    note: "The release is on the 28th".into(),
                }),
                cx,
            );
            app.start_comment(window, cx);
            if let Some(c) = app.explorer.comment.as_mut() {
                c.code.lines = Some((11, 15));
                c.input.update(cx, |s, cx| {
                    s.set_value("Shorter: one line per change, no marketing words", window, cx)
                });
            }
        }
        _ => {}
    }
}

/// What in the Explorer waits on what it read: the terminal's lines, the problems, a spec.
fn explorer_later(hooks: &DemoHooks, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    match hooks.explorer.as_deref() {
        Some("terminal") => app.send_terminal_to_thread(window, cx),
        Some("problems") => app.send_problems_to_thread("p-blog", window, cx),
        Some("specs") => {
            let spec = app
                .explorer
                .specs
                .get(&app.explorer.project)
                .cloned()
                .flatten()
                .and_then(|s| s.specs.get(1).cloned());
            if let Some(s) = spec {
                let key = app.spec_key(&s);
                app.open_doc(key, window, cx);
            }
        }
        _ => {}
    }
}

/// The gear open, on its tab.
fn open_gear(hooks: &DemoHooks, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    app.gear.tab = hooks.gear_tab.as_ref().and_then(|t| t.parse().ok()).unwrap_or(0);
    app.leave_code_for_gear(cx);
    app.center = crate::workspace::Center::Settings;
    app.load_providers(window, cx);
}

/// `BENCH_DEMO_SCROLL`, or where a hook's work stands: the item and the pixels into it.
fn scroll_to(hooks: &DemoHooks, v: &ThreadView) -> Option<(usize, f32)> {
    let spec = match hooks.mode() {
        "top" => Some("0:0".to_string()),
        _ => hooks.scroll.clone(),
    };
    if let Some((i, o)) = spec.as_ref().and_then(|s| s.split_once(':')) {
        return Some((i.parse().ok()?, o.parse().ok()?));
    }
    // The opened work at the window's top, unless a scroll was asked for.
    if hooks.work.is_some() && spec.is_none() {
        let rows = v.thread.journal.rows();
        let key = v.thread.open_work.iter().next()?;
        let ix = rows.iter().position(|r| r.key() == *key)?;
        let down = hooks.work_down.as_ref().and_then(|d| d.parse().ok()).unwrap_or(0.);
        return Some((ix, down));
    }
    None
}

async fn wait(cx: &mut Cx<'_>, ms: u64) {
    cx.background_executor().timer(Duration::from_millis(ms)).await;
}

/// Back to `this` every 30 ms until `ready`.
async fn until(this: &WeakEntity<BenchApp>, cx: &mut Cx<'_>, ready: impl Fn(&BenchApp, &App) -> bool) -> Option<()> {
    loop {
        if this.read_with(cx, |app, cx| ready(app, cx)).ok()? {
            return Some(());
        }
        wait(cx, 30).await;
    }
}

/// The whole script, on `app`'s window.
pub(super) fn run(hooks: &'static DemoHooks, app: &Entity<BenchApp>, window: &mut Window, cx: &mut App) {
    app.update(cx, |app, cx| {
        seed(hooks, app, window, cx);
        cx.spawn_in(window, async move |this, cx| {
            script(hooks, this, cx).await;
        })
        .detach();
        presses(hooks, window, cx);
    });
}

async fn script(hooks: &'static DemoHooks, this: WeakEntity<BenchApp>, cx: &mut Cx<'_>) -> Option<()> {
    // The agent is up and has said what it keeps.
    until(&this, cx, |app, _| {
        !app.threads.list.is_empty() && !app.places.projects.is_empty() && app.defaults.ready
    })
    .await?;
    // What the window opens on.
    this.update_in(cx, |app, window, cx| {
        match hooks.opens() {
            Some(id) => app.open_thread(String::new(), id, cx),
            None => app.active_thread.update(cx, |v, _| v.tools_shown = hooks.tools_shown.is_some()),
        }
        if hooks.panel.as_deref() == Some("providers") {
            open_gear(hooks, app, window, cx);
        }
        if hooks.mode() == "new-project" {
            app.new_project_form(window, cx);
        }
    })
    .ok()?;
    if hooks.opens().is_some() {
        until(&this, cx, |app, cx| app.thread(cx).loaded).await?;
    }
    this.update_in(cx, |app, window, cx| {
        app.active_thread.update(cx, |v, cx| {
            open_in_thread(hooks, &mut v.thread);
            if let Some(text) = hooks.draft.clone() {
                v.composer.update(cx, |s, cx| s.set_value(text, window, cx));
            }
            cx.notify();
        });
        if let Some(id) = &hooks.beside {
            app.open_beside(id.as_str().into(), window, cx);
        }
        if let Some(q) = &hooks.search {
            app.open_search(window, cx);
            if q != "1" {
                app.sidebar.search.update(cx, |s, cx| s.set_value(q.clone(), window, cx));
                app.search_threads(q.clone(), window, cx);
            }
        }
        if let Some(id) = &hooks.rename {
            app.start_rename(crate::sidebar::items::Item::Thread(id.as_str().into()), window, cx);
        }
        if matches!(hooks.mode(), "explorer" | "env") {
            app.left_dock.activate(crate::workspace::Panel::Explorer);
            app.explorer.project = "p-blog".into();
            app.explorer.threads = hooks.side.as_deref() == Some("threads");
            app.load_env("p-blog", window, cx);
        }
    })
    .ok()?;
    // The Explorer, once the project's environment is read.
    if matches!(hooks.mode(), "explorer" | "env") {
        until(&this, cx, |app, _| app.envs.get("p-blog").is_some_and(|e| !e.dir.is_empty())).await?;
        this.update_in(cx, |app, window, cx| open_explorer(hooks, app, window, cx)).ok()?;
        wait(cx, 700).await;
        this.update_in(cx, |app, window, cx| explorer_later(hooks, app, window, cx)).ok()?;
        if let Some(to) = hooks.env_scroll.as_deref().filter(|t| matches!(*t, "telegram" | "model")) {
            wait(cx, 900).await;
            this.update(cx, |app, cx| {
                let scroll = app.explorer.settings_scroll.clone();
                if to == "model" {
                    scroll.set_offset(point(px(0.), px(-620.)));
                } else {
                    scroll.scroll_to_bottom();
                }
                cx.notify();
            })
            .ok()?;
        }
    }
    // A scroll waits for the rows' first sync, which lands the thread at its end.
    let view = this.read_with(cx, |app, _| app.active_thread.clone()).ok()?;
    until(&this, cx, |_, cx| !view.read(cx).thread.tv.rows.is_empty() || hooks.opens().is_none()).await?;
    view.update(cx, |v, cx| {
        if let Some((i, o)) = scroll_to(hooks, v) {
            v.thread.tv.list.scroll_to(ListOffset {
                item_ix: i,
                offset_in_item: px(o),
            });
            cx.notify();
        }
    });
    // `BENCH_DEMO_SCROLL=end`: the thread keeps to its end (a tail shot: what grows after the first
    // frame — the queue, an answer — is still in the picture).
    if hooks.scroll.as_deref() == Some("end") {
        loop {
            view.update(cx, |v, cx| {
                v.thread.tv.to_end();
                cx.notify();
            });
            wait(cx, 200).await;
        }
    }
    Some(())
}

/// What is pressed a few seconds in, each its own timer.
fn presses(hooks: &'static DemoHooks, window: &mut Window, cx: &mut Context<BenchApp>) {
    let after = |secs: u64, press: Box<dyn FnOnce(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>, window: &mut Window, cx: &mut Context<BenchApp>| {
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(secs)).await;
            this.update_in(cx, |app, window, cx| press(app, window, cx)).ok();
        })
        .detach();
    };
    if hooks.jump.is_some() {
        after(2, Box::new(|app, _, cx| app.active_thread.update(cx, |v, cx| v.jump_to_latest(cx))), window, cx);
    }
    if let Some(how) = hooks.project.clone() {
        after(1, Box::new(move |app, window, cx| new_project(&how, app, window, cx)), window, cx);
    }
    if hooks.new_in_panel.is_some() {
        after(
            2,
            Box::new(|app, window, cx| {
                app.new_thread_in_panel(cx);
                app.focus_composer(window, cx);
            }),
            window,
            cx,
        );
    }
    if let Some(typed) = hooks.typed.clone() {
        after(
            2,
            Box::new(move |app, window, cx| {
                app.focus_composer(window, cx);
                app.active_thread.update(cx, |v, cx| {
                    v.composer.update(cx, |s, cx| {
                        let text = if typed == "1" {
                            "Check the staging deploy and tell me whether it is healthy.\n\
                                 \n\
                                 Second paragraph, long enough to reach the very right of the \
                                 field, where the tools' gutter keeps the paperclip and the mic \
                                 clear of every word."
                                .to_string()
                        } else {
                            typed.clone()
                        };
                        s.set_value(text, window, cx);
                    })
                });
            }),
            window,
            cx,
        );
    }
    // The message goes to the queue of the running demo thread and its waiting row must show the
    // same frame, before the agent could ever answer. Deferred a beat off the app's own update:
    // Send reads the app, and it must not be borrowed already.
    if hooks.send.is_some() {
        after(
            3,
            Box::new(|app, window, cx| {
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
            }),
            window,
            cx,
        );
    }
    if hooks.run.is_some() {
        run_events(hooks, cx);
    }
    if let Some(where_) = hooks.drop.clone() {
        super::probes::drop_file(where_, window, cx);
    }
    if let Some(at) = hooks.click.clone() {
        super::probes::click(&at, window, cx);
    }
}

/// `BENCH_DEMO_PROJECT=1`: the new project's page; `=env` with an exported environment chosen for
/// it (`env/backup.rs`); `=long` a server's project with a long folder on it.
fn new_project(how: &str, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    app.new_project_form(window, cx);
    if how == "long" {
        if let Some(f) = app.new_project.form.as_mut() {
            f.place = crate::settings::new_project::Place::OverSsh;
            f.machine = Some("web".into());
            f.name.update(cx, |s, cx| s.set_value("Northwind", window, cx));
            f.server_folder.update(cx, |s, cx| {
                s.set_value(
                    "/home/web/northwind.example/www/wp-content/themes/northwind-2026/templates/landing",
                    window,
                    cx,
                )
            });
        }
        return;
    }
    if how != "env" {
        return;
    }
    let file = serde_json::json!({
        "kind": "bench.environment",
        "name": "Northwind blog",
        "files": {
            "SYSTEM.md": {"text": ""},
            "skills/release-notes/SKILL.md": {"text": ""},
            "skills/cover-image/SKILL.md": {"text": ""},
            "prompts/brief.md": {"text": ""},
            "mcp.json": {"text": "{\"mcpServers\":{\"ghost\":{},\"figma\":{}}}"}
        },
        "cron": [{"name": "Weekly digest"}],
        "telegram": {"token": "demo"}
    });
    if let Some(f) = app.new_project.form.as_mut() {
        f.name.update(cx, |s, cx| s.set_value("Northwind blog", window, cx));
        f.environment = Some(("Northwind blog environment 2026-10-28.json".into(), file));
    }
}

/// `BENCH_DEMO_RUN=1`: a run plays in the open thread as the agent sends it — a message, the
/// model thinking, a tool, the answer streamed, its entry, the run's end — for watching the
/// thread hold still while it is written.
fn run_events(hooks: &'static DemoHooks, cx: &mut Context<BenchApp>) {
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
            events.push((80, ThreadEvent::Stream(StreamKind::Thought, "Looking at the numbers. ".into())));
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
                    args: serde_json::json!({}),
                    preview: "analytics".into(),
                    at: Some(now),
                },
            ]),
        ));
        // Words the model wrote between its thinking and the call: the line of work shows this
        // narrative, with the newest call's kind after a dot.
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
        if hooks.paused.is_some() {
            let held = now + 400.;
            events.push((400, ThreadEvent::Holds(vec![::thread::Hold { from: held, to: None }])));
            events.push((6000, ThreadEvent::Holds(vec![::thread::Hold { from: held, to: Some(held + 6000.) }])));
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
            let delta = if k % 6 == 5 { format!("{line}\n\n") } else { line.to_string() };
            events.push((60, ThreadEvent::Stream(StreamKind::Text, delta)));
        }
        let text = (0..10).map(|_| line.repeat(6)).collect::<Vec<_>>().join("\n\n");
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
            cx.background_executor().timer(Duration::from_millis(ms)).await;
            let shown = this.update(cx, |app, cx| {
                app.active_thread.update(cx, |v, cx| {
                    // As a real stream: the run's label, the speed meter, the journal.
                    v.thread.on_thread_event(&e);
                    cx.notify();
                })
            });
            if shown.is_err() {
                return;
            }
        }
    })
    .detach();
}
