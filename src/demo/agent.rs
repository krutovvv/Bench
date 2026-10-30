//! The demo's agent: a `FakeAgent` (`client/fake.rs`) that answers what Bench asks of a real one
//! from made-up threads and a made-up blog project, and its machine from made-up servers.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use percent_encoding::percent_decode_str;
use protocol::{ThreadOpen, ThreadSummary};
use serde_json::{Value, json};

use super::env;
use super::hooks::DemoHooks;
use crate::client::fake::{FakeAgent, FakeTab};

/// The tab the browser insert of the blog's thread shows.
pub(super) const BROWSER_TAB: &str = "demo-tab";
pub(super) const BROWSER_URL: &str = "https://blog.example.com/admin/posts/new";

pub struct DemoAgent {
    rows: Vec<ThreadSummary>,
    opens: HashMap<String, ThreadOpen>,
    hooks: &'static DemoHooks,
}

impl DemoAgent {
    pub fn new(hooks: &'static DemoHooks) -> Self {
        let (rows, opens) = super::threads::threads(hooks, crate::app::now_secs());
        Self { rows, opens, hooks }
    }

    /// `thread.open`: its items after `since`, and the rest as it is.
    fn open_thread(&self, params: &Value) -> ThreadOpen {
        let id = params["thread"].as_str().unwrap_or_default();
        let since = params["since"].as_u64().unwrap_or(0);
        let mut open = self.opens.get(id).cloned().unwrap_or_default();
        open.items.retain(|it| since == 0 || item_seq(it) > since);
        open
    }

    fn projects(&self) -> Value {
        let project = |id: &str, name: &str, folder: Option<&str>| {
            json!({"id": id, "name": name, "folder": folder, "browser": true})
        };
        let mut blog = project("p-blog", "Northwind blog", Some("/Users/demo/code/blog"));
        // Its bot takes its readers' questions in Telegram.
        blog["telegram"] = json!({"token": "4711:AAExampleTokenForScreenshots"});
        // Worked on over SSH, on web-1; its certificates are checked on a schedule.
        let mut web = project("p-web", "Production site", Some("/var/www/storefront"));
        web["machine"] = json!("web-1");
        web["cron"] = json!(true);
        json!({
            "projects": [
                blog,
                project("p-store", "Storefront", Some("/Users/demo/code/storefront")),
                web,
                project("p-audit", "Competitor audit", None),
                project("p-me", "Personal", None),
            ],
            "host_browser": true,
            // This Mac's agent has its general bot and a schedule of its own (the daily digest).
            "host_telegram": true,
            "host_cron": true,
        })
    }
}

/// A journal item's entry id.
fn item_seq(item: &protocol::ThreadItem) -> u64 {
    serde_json::to_value(item)
        .ok()
        .and_then(|v| v["seq"].as_u64())
        .unwrap_or(0)
}

/// A query's value, decoded.
fn query<'a>(path: &'a str, key: &str) -> String {
    path.split_once('?')
        .map_or("", |(_, q)| q)
        .split('&')
        .find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
        .map(|v| percent_decode_str(v).decode_utf8_lossy().into_owned())
        .unwrap_or_default()
}

impl FakeAgent for DemoAgent {
    fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        Ok(match method {
            "threads.list" => serde_json::to_value(&self.rows)?,
            "thread.open" => serde_json::to_value(self.open_thread(&params))?,
            "thread.create" | "thread.fork" => json!({"thread": "d-new"}),
            "commands.list" => json!([]),
            // Whatever else is told to it, it took (`Done`).
            _ => json!({}),
        })
    }

    fn rest(&self, method: &str, path: &str, body: Value) -> Result<Value> {
        let route = path.split('?').next().unwrap_or(path);
        let file = query(path, "path");
        Ok(match route {
            "/api/status" => json!({}),
            "/api/agents" => json!({"agents": []}),
            "/api/presets" => env::presets(),
            "/api/projects" => self.projects(),
            "/api/config/raw" => env::config(self.hooks.no_model.is_none()),
            "/api/providers/custom-endpoints" => env::endpoints(),
            "/api/env" => json!({}),
            "/api/judge" => env::judge(),
            "/api/fs/default-cwd" => json!({"cwd": "/Users/demo"}),
            "/api/fs/list" => env::listing(&file),
            "/api/fs/read-text" => json!({"text": env::file_text(&file)}),
            "/api/git/head" => json!({"text": env::file_head(&file)}),
            "/api/git/ignored" => env::ignored(&file),
            "/api/git/show" => json!({"text": env::turn_file(&query(path, "tree"), &file)}),
            "/api/specs" => env::specs(),
            "/api/lsp/diagnostics" => {
                let path = body["path"].as_str().unwrap_or_default();
                json!({"known": true, "problems": env::file_problems(path)})
            }
            "/api/lsp/problems" => {
                let post = "/Users/demo/code/blog/content/posts/release-2-4.md";
                json!({"files": [{"path": post, "problems": env::file_problems(post)}]})
            }
            "/api/threads/search" => json!({"results": []}),
            r if r.starts_with("/api/projects/") && r.ends_with("/env") => env::env(),
            r if r.starts_with("/api/projects/") && r.ends_with("/prompts") => json!({"prompts": []}),
            _ if method != "GET" => json!({}),
            _ => return Err(anyhow!("the demo's agent has no {route}")),
        })
    }

    fn git(&self, dir: &str, args: &[&str]) -> Result<String> {
        match args {
            ["rev-parse", "--show-toplevel"] => Ok(dir.to_string()),
            ["rev-parse", "--abbrev-ref", "HEAD"] => Ok("main".into()),
            _ => Err(anyhow!("git {}: not in the demo", args.first().unwrap_or(&""))),
        }
    }

    /// Made-up servers, never the person's.
    fn ssh_servers(&self) -> Vec<crate::system::sshconfig::Server> {
        let server = |alias: &str, hostname: &str, folder: &str| crate::system::sshconfig::Server {
            alias: alias.into(),
            hostname: hostname.into(),
            identity: String::new(),
            user: "deploy".into(),
            port: String::new(),
            folder: folder.into(),
            path: String::new(),
            shared: false,
        };
        vec![
            server("web-1", "203.0.113.10", "Production"),
            server("web-2", "203.0.113.11", "Production"),
            server("db-main", "203.0.113.20", "Production"),
            server("staging", "198.51.100.5", "Staging"),
            server("ci-runner", "198.51.100.9", "Staging"),
            server("homelab", "192.168.1.50", ""),
        ]
    }

    /// The server's last screen, and a program that waits: no shell runs in the demo.
    fn terminal_script(&self, host: &str) -> String {
        let host = if host.is_empty() { "local" } else { host };
        let screen = format!(
            "deploy@{host}:~$ uptime\n \
10:41:07 up 37 days,  2:14,  1 user,  load average: 3.91, 3.62, 2.10\n\
deploy@{host}:~$ docker stats --no-stream\n\
NAME             CPU %     MEM USAGE / LIMIT\n\
search-indexer   188.42%   1.21GiB / 3.84GiB\n\
storefront       6.10%     412MiB / 3.84GiB\n\
nginx            0.31%     18MiB / 3.84GiB\n\
deploy@{host}:~$ "
        );
        format!("printf '%s' '{screen}'; exec sleep 86400")
    }

    fn browser_tab(&self) -> FakeTab {
        FakeTab {
            id: BROWSER_TAB.into(),
            url: BROWSER_URL.into(),
            title: "New post".into(),
            jpeg: include_bytes!("../../assets/demo/site.jpg"),
        }
    }

    /// `BENCH_DEMO_POP=agent`: lines long enough that the panel must cut them.
    fn agent_log(&self) -> Vec<String> {
        if self.hooks.pop.as_deref() != Some("agent") {
            return vec![];
        }
        [
            "[mcp] default/rust_lsp:6 default/playwright:25",
            "[mcp] project:c15b4fa96b5b/yandex-metrika: connect timed out after 30000 ms, retrying in 5s (attempt 3 of 5)",
            "[model] project_6ca7d18dde42.dev/Qwen3.8-Next-Flash failed (503: {\"code\":\"model_busy\",\"message\":\"Too many concurrent requests for Qwen3.8-Next-Flash; retry shortly\"}); answering with project_6ca7d18dde42.openrouter/qwen/qwen3.8-flash",
            "[cron] project_dd3776fe5719: ran \"Morning brief\" in 42s",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }
}
