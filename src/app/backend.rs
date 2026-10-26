//! The agent's process and macOS notifications: restart it, test a notification.

use std::time::Duration;

use web_time::Instant;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use crate::connection::{self};

use super::*;

impl BenchApp {
    /// Restart the agent on the machine we are connected to, then reconnect. Its runs are
    /// durable, so it never refuses (the `Busy` branch: a backend that refused while work ran,
    /// pressed again within a minute, restarted anyway).
    /// The gateway panel's notification line: refresh what macOS allows, and with `test` also
    /// send one — the way to see for oneself that Bench can say it is done from the background.
    pub fn notif(&mut self, test: bool, cx: &mut Context<Self>) {
        if test {
            crate::notify::post(
                "bench.test",
                "Bench",
                "This is a test: Bench says it is done.",
            );
        }
        crate::notify::check();
        cx.notify();
    }

    /// System Settings → Notifications: the choice is macOS's to keep, not ours.
    pub fn notif_settings(&mut self, cx: &mut Context<Self>) {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.notifications")
            .spawn();
        cx.notify();
    }

    pub fn restart_gateway(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        let interrupt = self
            .restart_armed
            .take()
            .is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        self.toast(
            NotificationType::Info,
            if interrupt {
                "Interrupting running work and restarting the agent…"
            } else {
                "Restarting the agent…"
            },
            window,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let t = target.clone();
            let result = cx
                .background_spawn(async move { connection::restart_gateway(&t, interrupt) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(connection::Restart::Done) => {
                    this.toast(NotificationType::Success, "Agent restarted", window, cx);
                    this.connect(target, window, cx);
                }
                Ok(connection::Restart::Busy(what)) => {
                    this.restart_armed = Some(Instant::now());
                    this.toast(
                        NotificationType::Warning,
                        format!("Not restarted: {what}. Press restart again to interrupt it and restart."),
                        window,
                        cx,
                    );
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Restart: {e:#}"),
                    window,
                    cx,
                ),
            });
        })
        .detach();
    }
}
