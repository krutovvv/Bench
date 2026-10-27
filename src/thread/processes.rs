//! The agent's background processes in the open session (`process.list`): polled every few
//! seconds while a session is open, shown as a card under the last answer, each one stoppable
//! (`process.kill`) and with its latest output a click away.

use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rpc::Cmd;
use crate::app::model::BgProcess;

impl BenchApp {
    /// Ask the gateway again (the poll, and right after a Stop).
    pub(crate) fn poll_processes(&self) {
        if let (Some(profile), Some(session)) = (self.profile.clone(), self.runtime.clone()) {
            self.cmd(Cmd::Processes { profile, session });
        }
    }

    pub(crate) fn on_processes(&mut self, list: Vec<BgProcess>, cx: &mut Context<Self>) {
        // Running ones first, then the newest.
        let mut list = list;
        list.sort_by(|a, b| {
            b.running.cmp(&a.running).then(
                b.started
                    .unwrap_or(0.)
                    .partial_cmp(&a.started.unwrap_or(0.))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let running = list.iter().any(|p| p.running);
        if list != self.processes || running {
            // Running ones repaint for their clocks.
            self.processes = list;
            cx.notify();
        }
    }

    pub(crate) fn kill_process(&mut self, id: String, cx: &mut Context<Self>) {
        if let (Some(profile), Some(session)) = (self.profile.clone(), self.runtime.clone()) {
            self.cmd(Cmd::KillProcess {
                profile,
                session,
                id,
            });
            self.poll_processes();
        }
        cx.notify();
    }

    /// A row's output shown or hidden.
    pub(crate) fn toggle_process_output(&mut self, id: String, cx: &mut Context<Self>) {
        if self.process_open.as_deref() == Some(id.as_str()) {
            self.process_open = None;
        } else {
            self.process_open = Some(id);
        }
        cx.notify();
    }
}
