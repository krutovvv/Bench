//! Reaching the agent: connect to a target, send the open session a command, list its threads,
//! ask its REST API off the UI thread.

use gpui_kit::*;

use super::{BenchApp, agents};
use crate::app::model::Target;
use crate::client::connection::{self, Connection};
use crate::client::rpc::{Cmd, Gateway};

impl BenchApp {
    pub fn connect(&mut self, target: Target, _window: &mut Window, cx: &mut Context<Self>) {
        self.agents.remove(""); // drops the old thread
        self.target = target.clone();
        self.connecting = true;
        self.profile_ready = false;
        self.approval_mode = None;
        self.opened.clear();
        self.profiles.clear();
        self.profile = None;
        self.sessions.clear();
        self.skills.clear();
        self.reset_chat(cx);
        self.pv = Default::default();
        self.gw_status = None;
        self.gw_log.clear();
        let dir = connection::state_dir();
        let _ = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(dir.join("last-target"), target.label()));
        self.agents.insert(
            String::new(),
            agents::AgentLink::new(Gateway::start(target, self.evt_tx.clone())),
        );
        cx.notify();
    }

    /// To the agent the command concerns (`agents.rs`): this Mac's, or a remote project's.
    pub(crate) fn cmd(&self, cmd: Cmd) {
        self.route(cmd);
    }

    pub fn current_profile(&self) -> String {
        self.profile.clone().unwrap_or_default()
    }

    /// Events carry the runtime id.
    pub(crate) fn is_current(&self, sid: &str) -> bool {
        self.runtime.as_deref() == Some(sid)
    }

    pub(crate) fn refresh_sessions(&self) {
        if let Some(p) = self.profile.clone() {
            self.cmd(Cmd::Sessions(p));
        }
    }

    // ---- background REST helpers -------------------------------------------

    /// Run `job` off the UI thread with the current connection, then `done` on it.
    pub(crate) fn rest<T: Send + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        job: impl FnOnce(&Connection, &str) -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let Some(conn) = self.local_conn() else {
            return;
        };
        let profile = self.current_profile();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { job(&conn, &profile) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                done(this, result, window, cx);
                cx.notify();
            });
        })
        .detach();
    }
}
