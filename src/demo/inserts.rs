//! What the inserts show in the demo: made-up ~/.ssh/config servers (and a terminal's last
//! screen), so the demo never shows the person's own.

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

/// Made-up ~/.ssh/config servers, so the demo never shows the person's own.
pub(super) fn other_modes(app: &mut BenchApp) {
    app.inserts.ssh.servers = vec![
        server("web-1", "203.0.113.10", "Production"),
        server("web-2", "203.0.113.11", "Production"),
        server("db-main", "203.0.113.20", "Production"),
        server("staging", "198.51.100.5", "Staging"),
        server("ci-runner", "198.51.100.9", "Staging"),
        server("homelab", "192.168.1.50", ""),
    ];
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
