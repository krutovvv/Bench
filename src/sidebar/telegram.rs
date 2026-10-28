//! Which Telegram threads the sidebar shows: a bot answers anyone who writes to it, and the
//! threads of other people are theirs — the person's own are those whose name there is this Mac
//! user's. When none is, every thread shows.

use crate::app::BenchApp;

/// This Mac user's first name, lowercased (`id -F`), once.
fn me() -> &'static str {
    static ME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ME.get_or_init(|| {
        std::process::Command::new("id")
            .arg("-F")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase())
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    })
}

impl BenchApp {
    /// Indices into `threads` the list shows (other people's Telegram threads left out).
    pub fn visible_threads(&self) -> Vec<usize> {
        let mine = |name: &str| {
            !me().is_empty() && name.to_lowercase().split_whitespace().any(|w| w == me())
        };
        let anyone_mine = self
            .threads.list
            .iter()
            .any(|s| s.source == "telegram" && s.with.as_deref().is_some_and(mine));
        (0..self.threads.list.len())
            .filter(|&i| {
                let s = &self.threads.list[i];
                !anyone_mine || s.source != "telegram" || s.with.as_deref().is_some_and(mine)
            })
            .collect()
    }
}
