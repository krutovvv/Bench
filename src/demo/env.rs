//! The blog project's environment and files, for its Settings and the Explorer (any project the
//! demo shows reads as the blog).

use gpui_kit::*;

use crate::app::BenchApp;

/// shows reads as the blog).
pub fn env(app: &mut BenchApp, id: &str, window: &mut Window, cx: &mut Context<BenchApp>) {
    let Some(e) = app.envs.get_mut(id) else {
        return;
    };
    let v = serde_json::json!({
        "dir": "~/.bench/projects/p-blog",
        "cwd": "/Users/demo/code/blog",
        "project": {"name": "Northwind blog", "folder": "/Users/demo/code/blog"},
        "soul": "The Northwind blog: product news and guides for our customers, published on Ghost.\n\nWrite plainly, for people who use the product, not for engineers. Every post gets a cover and two tags; drafts first, publish only when asked.",
        "mcp": [
            {"name": "ghost", "kind": "http", "enabled": true, "connected": true, "tools": 12},
            {"name": "unsplash", "kind": "stdio", "enabled": true, "connected": true, "tools": 3},
            {"name": "figma", "kind": "http", "enabled": true, "connected": true, "tools": 0, "error": "needs signing in", "needs_auth": true},
            {"name": "search-console", "kind": "stdio", "enabled": false, "connected": false, "tools": 0}
        ],
        "mcp_json": "{\n  \"mcpServers\": {\n    \"ghost\": { \"url\": \"https://blog.example.com/ghost/mcp\", \"headers\": { \"Authorization\": \"${GHOST_ADMIN_KEY}\" } },\n    \"unsplash\": { \"command\": \"npx\", \"args\": [\"unsplash-mcp\"] },\n    \"search-console\": { \"command\": \"uvx\", \"args\": [\"gsc-mcp\"], \"enabled\": false }\n  }\n}\n",
        "lsp": {
            "servers": {
                "typescript": {"command": "${LSP}/node_modules/.bin/typescript-language-server", "args": ["--stdio"], "extensions": ["ts", "tsx", "js", "jsx"]},
                "php": {"command": "${LSP}/node_modules/.bin/intelephense", "args": ["--stdio"], "extensions": ["php"]},
                "pyright": {"command": "${LSP}/node_modules/.bin/pyright-langserver", "args": ["--stdio"], "extensions": ["py"], "npm": ["pyright"]}
            },
            "states": {
                "typescript": "running · 4 files checked, all answered · last 18:02, post.ts",
                "pyright": "failed: no Node on this machine — install Node.js (with npm) there"
            }
        },
        "compaction": 0.9,
        "code_mode": true,
        "window": 131072,
        "telegram": {
            "username": "northwind_blog_bot",
            "allowed_users": ["246813579"],
            "token": "4711:AAExampleTokenForScreenshots",
            "proxy": "http://127.0.0.1:7890",
            "status": { "state": "live", "detail": "@northwind_blog_bot", "at": 1780000000000i64 }
        },
        "skills": [
            {"name": "release-notes", "description": "Turn a changelog into a post: what changed for the reader, not the commit list.", "scope": "project", "dir": "~/.bench/projects/p-blog/skills/release-notes"},
            {"name": "cover-image", "description": "A 1600×900 cover in the blog's colors, from a photo or a gradient.", "scope": "project", "dir": "~/.bench/projects/p-blog/skills/cover-image"},
            {"name": "ghost-theme", "description": "How the blog's Ghost theme is built and changed.", "scope": "repo", "dir": "~/Projects/blog/.agents/skills/ghost-theme"},
            {"name": "humanizer", "description": "Make a text read as written by a person.", "scope": "general", "dir": "~/.bench/agents/default/skills/humanizer"}
        ],
        "cron": [
            {"id": "c-digest", "name": "Weekly digest", "prompt": "Draft the weekly digest from this week's posts and comments; save it as a draft.", "schedule": {"expr": "0 9 * * 1"}, "schedule_display": "0 9 * * 1", "enabled": true, "next_run_at": "2026-10-26T09:00:00+03:00", "last_run_at": "2026-10-19T09:00:00+03:00", "last_status": "ok", "last_output": "The digest draft is saved: 4 posts, 2 top comments."},
            {"id": "c-links", "name": "Broken links", "prompt": "Check every post's links; list the broken ones with the post.", "schedule": {"expr": "every 1d"}, "schedule_display": "every 1d", "enabled": false, "next_run_at": null, "last_status": null}
        ]
    });
    crate::env::apply(e, &v, window, cx);
    // The presets the gear would have read from the agent (there is none in a demo copy).
    if app.gear.presets.lsp.is_empty() {
        app.gear.presets.lsp = vec![
            (
                "typescript".into(),
                "typescript-language-server".into(),
                serde_json::json!({"extensions": ["ts", "tsx", "js", "jsx"]}),
            ),
            (
                "pyright".into(),
                "pyright".into(),
                serde_json::json!({"extensions": ["py"]}),
            ),
            (
                "basedpyright".into(),
                "basedpyright".into(),
                serde_json::json!({"extensions": ["py"]}),
            ),
            (
                "rust-analyzer".into(),
                "rust-analyzer".into(),
                serde_json::json!({"extensions": ["rs"]}),
            ),
            (
                "php".into(),
                "intelephense".into(),
                serde_json::json!({"extensions": ["php", "inc"]}),
            ),
        ];
    }
    // `BENCH_ENV=telegram`: the page scrolls to the Telegram block a beat after opening.
    if std::env::var("BENCH_ENV").as_deref() == Ok("telegram") {
        let scroll = app.env_scroll.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(900))
                .await;
            scroll.scroll_to_bottom();
            let _ = this.update(cx, |_app, cx| cx.notify());
        })
        .detach();
    }
    // `BENCH_DEMO=explorer`: its tree open on a post, the soul and a skill; a file open, and
    // `BENCH_EXPLORER=comment` a review comment being written, `=specs` the specs and one read, `=settings` its settings, `=part` an MCP server, `=lsp` a language
    // server that failed, `=lsp-new` the "Add a server" card with its preset chips, `=thread` the
    // thread beside it
    // with lines sent to it. `BENCH_DEMO=env` is `explorer` with its settings open.
    let mode = std::env::var("BENCH_DEMO").unwrap_or_default();
    if !matches!(mode.as_str(), "explorer" | "env") || !app.explorer.tabs.is_empty() {
        return;
    }
    let dir = "~/.bench/projects/p-blog".to_string();
    let root = crate::explorer::FileRef {
        project: "p-blog".into(),
        machine: None,
        path: "/Users/demo/code/blog".into(),
    };
    let skill = app.agent_file(format!("{dir}/skills/release-notes"));
    for f in [
        root.clone(),
        root.join("content"),
        root.join("content/posts"),
        skill.clone(),
    ] {
        app.explorer.open.insert(format!("d:{}", f.listing_key()));
        app.list_folder(f, window, cx);
    }
    app.explorer.open.extend(
        [
            "files",
            "instructions",
            "skills",
            "g:Tools",
            "s:mcp",
            "g:Schedule",
            "s:cron",
        ]
        .map(String::from),
    );
    let post = crate::explorer::DocKey::File(root.join("content/posts/release-2-4.md"));
    app.open_doc(
        crate::explorer::DocKey::File(app.agent_file(format!("{dir}/SYSTEM.md"))),
        window,
        cx,
    );
    app.open_doc(post, window, cx);
    let explorer = if mode == "env" {
        "settings".to_string()
    } else {
        std::env::var("BENCH_EXPLORER").unwrap_or_default()
    };
    match explorer.as_str() {
        "settings" => app.open_doc(
            crate::explorer::DocKey::Settings {
                project: "p-blog".into(),
            },
            window,
            cx,
        ),
        "part" => app.open_part("p-blog".into(), "mcp", "ghost".into(), window, cx),
        "lsp" | "lsp-new" => {
            let name = if explorer == "lsp-new" {
                "new"
            } else {
                "pyright"
            };
            app.open_part("p-blog".into(), "lsp", name.into(), window, cx)
        }
        // The release post's changes since the last commit, beside it.
        "diff" => {
            let post = root.join("content/posts/release-2-4.md");
            app.open_diff(post, window, cx);
        }
        // The project's terminal under the post, its lines sent to the thread beside.
        "terminal" => {
            app.show_demo_thread("d-site", cx);
            app.bottom_dock.activate(crate::workspace::Panel::Terminal);
            app.send_terminal_to_thread(window, cx);
        }
        // The project's Problems, sent to the thread beside.
        "problems" => {
            app.show_demo_thread("d-site", cx);
            app.open_problems(window, cx);
            app.send_problems_to_thread("p-blog", window, cx);
        }
        // The thread beside the post, lines of it sent to the composer (⌘L).
        "thread" => {
            app.show_demo_thread("d-site", cx);
            app.right_dock.activate(crate::workspace::Panel::Thread);
            app.add_reference(
                crate::thread::composer::attachments::Reference::Code(crate::thread::composer::attachments::CodeRef {
                    path: "/Users/demo/code/blog/content/posts/release-2-4.md".into(),
                    lines: Some((11, 15)),
                    text: Some("## What changed".into()),
                }),
                cx,
            );
        }
        // The specs: the group open, the posts' spec read in its tab, the thread beside.
        "specs" => {
            app.explorer.open.extend(["specs".to_string(), "spec:blog".to_string()]);
            app.explorer.specs.insert(app.explorer.project.clone(), Some(specs()));
            let s = specs().specs[1].clone();
            let key = app.spec_key(&s);
            app.open_doc(key, window, cx);
        }
        // Review comments: one already in the composer, the next being written under the post.
        "comment" => {
            use crate::thread::composer::attachments::{CodeRef, CommentRef, Reference};
            app.show_demo_thread("d-site", cx);
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

/// The demo's folders, as the agent would list them: (name, is a folder).
/// What git ignores in the demo blog: the tree dims these rows, as it does for a real
/// repository (`/api/git/ignored` reads the real ones).
pub fn ignored(path: &str) -> Vec<String> {
    match path.trim_end_matches('/') {
        "/Users/demo/code/blog" => [
            "/Users/demo/code/blog/node_modules",
            "/Users/demo/code/blog/dist",
            "/Users/demo/code/blog/package-lock.json",
        ]
        .map(str::to_string)
        .to_vec(),
        _ => vec![],
    }
}

pub fn listing(path: &str) -> Vec<(String, String, bool)> {
    let names: &[(&str, bool)] = match path.trim_end_matches('/') {
        "/Users/demo/code/blog" => &[
            ("content", true),
            ("theme", true),
            ("node_modules", true),
            ("dist", true),
            (".gitignore", false),
            ("package-lock.json", false),
            ("package.json", false),
            ("README.md", false),
        ],
        "/Users/demo/code/blog/content" => &[("posts", true), ("pages", true)],
        "/Users/demo/code/blog/content/posts" => &[
            ("release-2-4.md", false),
            ("search-that-understands.md", false),
            ("welcome.md", false),
        ],
        "/Users/demo/code/blog/theme" => &[
            ("default.hbs", false),
            ("post.hbs", false),
            ("style.css", false),
        ],
        p if p.ends_with("skills/release-notes") => &[("references", true), ("SKILL.md", false)],
        p if p.ends_with("references") => &[("style.md", false)],
        _ => &[],
    };
    names
        .iter()
        .map(|(n, d)| {
            (
                n.to_string(),
                format!("{}/{n}", path.trim_end_matches('/')),
                *d,
            )
        })
        .collect()
}

/// A demo file's text.
pub fn file_text(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "SYSTEM.md" => "# Northwind blog\n\nProduct news and guides for our customers, published on Ghost.\n\nWrite plainly, for people who use the product, not for engineers. Every post gets a cover and two tags; drafts first, publish only when asked.\n".into(),
        "release-2-4.md" => "---\ntitle: \"Northwind 2.4: search that understands you\"\ntags: [releases, search]\ncover: /images/2-4-cover.png\nstatus: draft\n---\n\nSearch now reads what you mean, not only what you type. Ask for\n\"invoices from last spring\" and it finds them, even when none of them says *spring*.\n\n## What changed\n\n- Results understand dates, people and places.\n- Typos no longer hide what you were after.\n- The search box remembers your last five questions.\n\n## Why it matters\n\nMost of you search the same few things every day. Now it takes one try.\n".into(),
        "SKILL.md" => "---\nname: release-notes\ndescription: Turn a changelog into a post: what changed for the reader, not the commit list.\n---\n\n1. Read the changelog since the last release.\n2. Keep what a customer notices; drop refactors.\n3. Write the post in `content/posts/`, a draft, with a cover and two tags.\n".into(),
        "SPEC.md" => spec_text(path),
        _ => String::new(),
    }
}

/// The blog's specs, as the agent lists them (`explorer/specs/`).
pub fn specs() -> crate::explorer::specs::Specs {
    use crate::explorer::specs::{Spec, Specs};
    let root = "/Users/demo/code/blog";
    let spec = |id: &str, title: &str, parent: Option<&str>, refs: &[&str], dir: &str, draft: bool| Spec {
        id: id.into(),
        title: title.into(),
        status: Some(if draft { "draft" } else { "active" }.into()),
        parent: parent.map(Into::into),
        references: refs.iter().map(|r| r.to_string()).collect(),
        path: format!("{root}/{dir}SPEC.md"),
    };
    Specs {
        root: root.into(),
        specs: vec![
            spec("blog", "Northwind blog — product news for customers", None, &[], "", false),
            spec("posts", "content/posts — one release, one post", Some("blog"), &["theme"], "content/posts/", false),
            spec("pages", "content/pages — the pages that stay", Some("blog"), &[], "content/pages/", true),
            spec("theme", "theme — the Ghost theme and its covers", Some("blog"), &[], "theme/", false),
        ],
    }
}

fn spec_text(path: &str) -> String {
    if path.ends_with("content/posts/SPEC.md") {
        return "---\nid: posts\ntype: module-design\nstatus: active\ntitle: content/posts — one release, one post\nparent: blog\nreferences: [theme]\n---\n\n# One release, one post\n\nEvery release the customers notice gets one post, written for the people who use the product.\n\n## Boundary\n\n- A post says what changed **for the reader**: a refactor, a dependency bump or an internal rename is not news.\n- The changelog is the source; the post never promises what the changelog does not have.\n\n## Contracts\n\n- Front matter: `title`, two `tags`, a `cover` (1600×900, from `theme`), `status: draft` until the person says *publish*.\n- A post is a draft first, always. Publishing is the person's call.\n\n## Decisions\n\n- **Short over complete.** Three changes a reader feels beat ten they do not.\n- Titles say the benefit (*search that understands you*), not the version alone.\n".into();
    }
    "---\nid: blog\ntype: goal-and-requirements\ntitle: Northwind blog\n---\n\n# Northwind blog\n\nProduct news and guides for our customers.\n".into()
}

/// A file's text at the last commit (`explorer/git.rs`): the release post had another title, no
/// line on the search box, and one more sentence.
pub fn file_head(path: &str) -> Option<String> {
    let now = file_text(path);
    match path.rsplit('/').next().unwrap_or(path) {
        "release-2-4.md" => Some(
            now.replace("search that understands you", "smarter search")
                .replace("- The search box remembers your last five questions.\n", "")
                .replace(
                    "Now it takes one try.\n",
                    "Now it takes one try.\nAnd it is as fast as before.\n",
                ),
        ),
        _ => (!now.is_empty()).then_some(now),
    }
}

/// What the blog's language server says of a file the demo opens (`explorer/lsp.rs`): the
/// release post's cover is missing, its link to a draft is broken.
pub fn file_problems(path: &str) -> Vec<lsp_types::Diagnostic> {
    let at = |line, from, to| lsp_types::Range {
        start: lsp_types::Position::new(line, from),
        end: lsp_types::Position::new(line, to),
    };
    match path.rsplit('/').next().unwrap_or(path) {
        "release-2-4.md" => vec![lsp_types::Diagnostic {
            range: at(3, 7, 28),
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            source: Some("marksman".into()),
            message: "File does not exist: /images/2-4-cover.png".into(),
            ..Default::default()
        }],
        _ => vec![],
    }
}

/// The gear's Agents tab for this Mac's agent: an endpoint, the fallback, voice and the evaluator
/// (`BENCH_PANEL=providers`), as `load_providers` reads them from an agent.
pub fn providers() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let endpoints = serde_json::json!([{
        "id": "dev", "name": "dev", "base_url": "http://192.168.1.100:10137/v1",
        "model": "Qwen3.8-Next-Flash", "context_length": 131072, "current": true
    }]);
    let yaml = serde_json::json!({
        "model": {"provider": "dev", "default": "Qwen3.8-Next-Flash"},
        "fallback_providers": [{"provider": "openrouter", "model": "qwen/qwen3.8-flash"}],
        "stt": {"provider": "openai", "openai": {"base_url": "http://192.168.1.100:10240/v1", "model": "whisper-1", "language": "ru"}},
        "tts": {"provider": "openai", "openai": {"base_url": "http://192.168.1.100:10240/v1", "model": "tts-1", "voice": "alloy"}},
        "judge": {"enabled": true, "base_url": "http://192.168.1.100:10292", "model": "h2o-lightning-4b", "api_key": "sk-none"}
    });
    (endpoints, serde_json::json!({}), yaml)
}
