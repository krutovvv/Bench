//! What the inserts show in the demo: made-up ~/.ssh/config servers (and a terminal's last screen)
//! and tasks, so the demo never shows the person's own.


use crate::app::BenchApp;

fn server(alias: &str, hostname: &str, folder: &str) -> crate::system::sshconfig::Server {
    crate::system::sshconfig::Server {
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

fn task(
    id: &str,
    title: &str,
    tags: &[&str],
    day: Option<i64>,
) -> crate::inserts::tasks::model::Task {
    crate::inserts::tasks::model::Task {
        id: id.into(),
        title: title.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        day: day.map(|d| crate::inserts::tasks::model::today() + chrono::Duration::days(d)),
        created: crate::inserts::tasks::model::now(),
        project: "p-me".into(),
        ..Default::default()
    }
}

/// Made-up ~/.ssh/config servers and tasks, so the demo never shows the person's own.
pub(super) fn other_modes(app: &mut BenchApp) {
    app.ssh.servers = vec![
        server("web-1", "203.0.113.10", "Production"),
        server("web-2", "203.0.113.11", "Production"),
        server("db-main", "203.0.113.20", "Production"),
        server("staging", "198.51.100.5", "Staging"),
        server("ci-runner", "198.51.100.9", "Staging"),
        server("homelab", "192.168.1.50", ""),
    ];
    let today = crate::inserts::tasks::model::today();
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
