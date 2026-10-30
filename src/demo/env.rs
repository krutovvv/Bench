//! What the demo's agent says of the blog project and its files, as its REST answers carry it (any
//! project the demo shows reads as the blog): its environment, its folders, files and their last
//! commit, its specs, its language server, the presets, the gear's models.

use serde_json::{Value, json};

/// `/api/projects/<id>/env`.
pub(super) fn env() -> Value {
    json!({
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
        "evaluator": true,
        "evaluator_ready": true,
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
    })
}

/// `/api/presets`: the language servers a project takes with one click.
pub(super) fn presets() -> Value {
    let lsp = |title: &str, exts: &[&str]| json!({"title": title, "spec": {"extensions": exts}});
    json!({
        "mcp": {},
        "lsp": {
            "typescript": lsp("typescript-language-server", &["ts", "tsx", "js", "jsx"]),
            "pyright": lsp("pyright", &["py"]),
            "basedpyright": lsp("basedpyright", &["py"]),
            "rust-analyzer": lsp("rust-analyzer", &["rs"]),
            "php": lsp("intelephense", &["php", "inc"])
        }
    })
}

/// `/api/git/ignored`: what git ignores in the blog, the tree dims these rows.
pub(super) fn ignored(path: &str) -> Value {
    match path.trim_end_matches('/') {
        "/Users/demo/code/blog" => json!({
            "root": "/Users/demo/code/blog",
            "ignored": ["node_modules/", "dist/", "package-lock.json"]
        }),
        _ => json!({"root": "", "ignored": []}),
    }
}

/// `/api/fs/list`.
pub(super) fn listing(path: &str) -> Value {
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
    let entries: Vec<Value> = names
        .iter()
        .map(|(n, d)| json!({"name": n, "path": format!("{}/{n}", path.trim_end_matches('/')), "isDirectory": d}))
        .collect();
    json!({"entries": entries})
}

/// A demo file's text.
pub(super) fn file_text(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "SYSTEM.md" => "# Northwind blog\n\nProduct news and guides for our customers, published on Ghost.\n\nWrite plainly, for people who use the product, not for engineers. Every post gets a cover and two tags; drafts first, publish only when asked.\n".into(),
        "release-2-4.md" => "---\ntitle: \"Northwind 2.4: search that understands you\"\ntags: [releases, search]\ncover: /images/2-4-cover.png\nstatus: draft\n---\n\nSearch now reads what you mean, not only what you type. Ask for\n\"invoices from last spring\" and it finds them, even when none of them says *spring*.\n\n## What changed\n\n- Results understand dates, people and places.\n- Typos no longer hide what you were after.\n- The search box remembers your last five questions.\n\n## Why it matters\n\nMost of you search the same few things every day. Now it takes one try.\n".into(),
        "SKILL.md" => "---\nname: release-notes\ndescription: Turn a changelog into a post: what changed for the reader, not the commit list.\n---\n\n1. Read the changelog since the last release.\n2. Keep what a customer notices; drop refactors.\n3. Write the post in `content/posts/`, a draft, with a cover and two tags.\n".into(),
        "SPEC.md" => spec_text(path),
        _ => String::new(),
    }
}

/// `/api/specs`: the blog's specs.
pub(super) fn specs() -> Value {
    let root = "/Users/demo/code/blog";
    let spec = |id: &str, title: &str, parent: Option<&str>, refs: &[&str], dir: &str, draft: bool| {
        json!({
            "id": id,
            "title": title,
            "status": if draft { "draft" } else { "active" },
            "parent": parent,
            "references": refs,
            "path": format!("{root}/{dir}SPEC.md"),
        })
    };
    json!({
        "root": root,
        "specs": [
            spec("blog", "Northwind blog — product news for customers", None, &[], "", false),
            spec("posts", "content/posts — one release, one post", Some("blog"), &["theme"], "content/posts/", false),
            spec("pages", "content/pages — the pages that stay", Some("blog"), &[], "content/pages/", true),
            spec("theme", "theme — the Ghost theme and its covers", Some("blog"), &[], "theme/", false),
        ],
    })
}

fn spec_text(path: &str) -> String {
    if path.ends_with("content/posts/SPEC.md") {
        return "---\nid: posts\ntype: module-design\nstatus: active\ntitle: content/posts — one release, one post\nparent: blog\nreferences: [theme]\n---\n\n# One release, one post\n\nEvery release the customers notice gets one post, written for the people who use the product.\n\n## Boundary\n\n- A post says what changed **for the reader**: a refactor, a dependency bump or an internal rename is not news.\n- The changelog is the source; the post never promises what the changelog does not have.\n\n## Contracts\n\n- Front matter: `title`, two `tags`, a `cover` (1600×900, from `theme`), `status: draft` until the person says *publish*.\n- A post is a draft first, always. Publishing is the person's call.\n\n## Decisions\n\n- **Short over complete.** Three changes a reader feels beat ten they do not.\n- Titles say the benefit (*search that understands you*), not the version alone.\n".into();
    }
    "---\nid: blog\ntype: goal-and-requirements\ntitle: Northwind blog\n---\n\n# Northwind blog\n\nProduct news and guides for our customers.\n".into()
}

/// A file's text at the last commit (`/api/git/head`): the release post had another title, no
/// line on the search box, and one more sentence.
pub(super) fn file_head(path: &str) -> Option<String> {
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

/// What the blog's language server says of a file (`/api/lsp/diagnostics`): the release post's
/// cover is missing.
pub(super) fn file_problems(path: &str) -> Value {
    match path.rsplit('/').next().unwrap_or(path) {
        "release-2-4.md" => json!([{
            "range": {"start": {"line": 3, "character": 7}, "end": {"line": 3, "character": 28}},
            "severity": 2,
            "source": "marksman",
            "message": "File does not exist: /images/2-4-cover.png",
        }]),
        _ => json!([]),
    }
}

/// `/api/git/show`: the landing's and the signup form's two sides of the turn.
pub(super) fn turn_file(tree: &str, path: &str) -> Option<String> {
    let after = tree == "demo-head";
    match path.rsplit('/').next().unwrap_or(path) {
        "landing.tsx" => Some(if after {
            "export default function Landing() {\n  return (\n    <Page>\n      <Hero title=\"Ship faster\" />\n      <Quotes />\n      <Features />\n      <Pricing />\n    </Page>\n  );\n}\n".into()
        } else {
            "export default function Landing() {\n  return (\n    <Page>\n      <Hero title={long} subtitle={sub} />\n      <Features />\n      <Quotes />\n      <Pricing />\n    </Page>\n  );\n}\n".into()
        }),
        "SignupForm.tsx" => Some(if after {
            "<Form>\n  <Field name=\"email\" />\n  <Field name=\"name\" />\n  <Submit>Start free</Submit>\n</Form>\n".into()
        } else {
            "<Form>\n  <Field name=\"email\" />\n  <Field name=\"name\" />\n  <Field name=\"companySize\" />\n  <Field name=\"phone\" />\n  <Submit>Start free</Submit>\n</Form>\n".into()
        }),
        _ => None,
    }
}

/// The endpoints the gear lists (`/api/providers/custom-endpoints`).
pub(super) fn endpoints() -> Value {
    json!({"endpoints": [
        {
            "id": "dev", "name": "dev", "base_url": "http://192.168.1.100:10137/v1",
            "model": "Qwen3.8-Next-Flash", "context_length": 131072, "parallel": 2, "is_current": true
        },
        {
            "id": "openrouter", "name": "OpenRouter", "base_url": "https://openrouter.ai/api/v1",
            "model": "qwen/qwen3.8-flash", "context_length": 131072
        }
    ]})
}

/// The evaluator's servers (`/api/judge`).
pub(super) fn judge() -> Value {
    json!({
        "servers": [
            {"id": "local-jev", "name": "Local Jev", "base_url": "http://192.168.1.100:10292", "model": "h2o-lightning-4b", "api_key": "sk-none"},
            {"id": "openrouter", "name": "OpenRouter", "base_url": "https://openrouter.ai/api", "model": "typesafe/jev-1.13", "api_key": "sk-or-demo"}
        ],
        "main": "local-jev",
        "fallback": "openrouter"
    })
}

/// `/api/config/raw`: the agent's settings as its config says them. `model`: the main one, if any.
pub(super) fn config(model: bool) -> Value {
    let mut main = json!({"provider": "dev", "context_length": 131072});
    if model {
        main["default"] = json!("Qwen3.8-Next-Flash");
    }
    let yaml = json!({
        "model": main,
        "approvals": {"mode": "smart"},
        "providers": {
            "dev": {"name": "dev", "base_url": "http://192.168.1.100:10137/v1", "model": "Qwen3.8-Next-Flash"},
            "openrouter": {"name": "OpenRouter", "base_url": "https://openrouter.ai/api/v1", "model": "qwen/qwen3.8-flash"}
        },
        "fallback_providers": [{"provider": "openrouter", "model": "qwen/qwen3.8-flash"}],
        "stt": {"provider": "openai", "openai": {"base_url": "http://192.168.1.100:10240/v1", "model": "whisper-1", "language": "ru"}},
        "tts": {"provider": "openai", "openai": {"base_url": "http://192.168.1.100:10240/v1", "model": "tts-1", "voice": "alloy"}},
    });
    json!({"yaml": serde_yaml::to_string(&yaml).unwrap_or_default()})
}
