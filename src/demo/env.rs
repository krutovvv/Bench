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
        "project": {"name": "Northwind blog"},
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
                "php": {"command": "${LSP}/node_modules/.bin/intelephense", "args": ["--stdio"], "extensions": ["php"]}
            },
            "states": {"typescript": "running"}
        },
        "compaction": 0.9,
        "window": 131072,
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
    // `BENCH_DEMO=explorer`: its tree open on a post, the soul and a skill; a file open, and
    // `BENCH_EXPLORER=beside` another beside it, `=part` an MCP server open instead, `=thread` the
    // thread beside it with lines sent to it.
    if std::env::var("BENCH_DEMO").as_deref() != Ok("explorer") || !app.explorer.tabs.is_empty() {
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
    match std::env::var("BENCH_EXPLORER").as_deref() {
        Ok("beside") => app.open_beside(skill.join("SKILL.md"), window, cx),
        Ok("part") => app.open_part("p-blog".into(), "mcp", "ghost".into(), window, cx),
        // The thread beside the post, lines of it sent to the composer (⌘L).
        Ok("thread") => {
            app.session = Some("d-site".into());
            app.thread = app.demo_threads.get("d-site").cloned().unwrap_or_default();
            app.right = Some(crate::workspace::RightPanel::Thread);
            app.add_reference(
                crate::thread::attachments::CodeRef {
                    path: "/Users/demo/code/blog/content/posts/release-2-4.md".into(),
                    lines: Some((11, 15)),
                    text: Some("## What changed".into()),
                },
                cx,
            );
        }
        _ => {}
    }
}

/// The demo's folders, as the agent would list them: (name, is a folder).
pub fn listing(path: &str) -> Vec<(String, String, bool)> {
    let names: &[(&str, bool)] = match path.trim_end_matches('/') {
        "/Users/demo/code/blog" => &[
            ("content", true),
            ("theme", true),
            (".gitignore", false),
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
        _ => String::new(),
    }
}
