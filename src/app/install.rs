//! Setting things up in a terminal the person watches: installing Hermes on this Mac when it is
//! missing, logging in to Claude. The command is typed into a fresh local shell shown in the chat
//! area; the shell closes itself when the command succeeded (`&& exit`), and then the app moves
//! on (connects, re-checks). On a failure the shell stays, with the log and a "Try again".

use std::sync::atomic::Ordering;
use std::time::Duration;

use gpui_kit::*;

use super::HermesApp;
use crate::connection;
use crate::model::Target;

#[derive(Clone, Copy, PartialEq)]
pub enum SetupKind {
    Hermes,
    ClaudeLogin,
}

impl SetupKind {
    pub fn title(self) -> &'static str {
        match self {
            SetupKind::Hermes => "Installing Hermes Agent",
            SetupKind::ClaudeLogin => "Claude login",
        }
    }

    fn command(self) -> &'static str {
        match self {
            SetupKind::Hermes => connection::INSTALL_HERMES,
            SetupKind::ClaudeLogin => "claude auth login",
        }
    }
}

/// The alias of a setup terminal in `ssh.tabs` (never an ssh host: it has a space).
pub const SETUP_ALIAS: &str = "setup terminal";

/// The running setup: what it is and the id of its terminal tab.
pub struct Setup {
    pub kind: SetupKind,
    pub tab: usize,
}

impl HermesApp {
    /// The setup's terminal in `ssh.tabs`, while it is there.
    pub fn setup_tab(&self) -> Option<usize> {
        let id = self.setup.as_ref()?.tab;
        self.ssh.tabs.iter().position(|t| t.id == id)
    }

    /// Whether the setup's command has ended (its shell is gone).
    pub fn setup_exited(&self) -> bool {
        self.setup_tab()
            .is_some_and(|t| self.ssh.tabs[t].session.exited.load(Ordering::SeqCst))
    }

    pub fn start_setup(&mut self, kind: SetupKind, window: &mut Window, cx: &mut Context<Self>) {
        self.close_setup(cx);
        let Some(tab) = self.spawn_terminal_tab(crate::term::LOCAL, window, cx) else {
            return;
        };
        // Not one of the person's terminals: SSH mode's "this Mac" must not pick it up.
        self.ssh.tabs[tab].alias = SETUP_ALIAS.into();
        let t = &self.ssh.tabs[tab];
        let id = t.id;
        t.session
            .input(format!("{} && exit\n", kind.command()).as_bytes());
        t.focus.focus(window, cx);
        self.setup = Some(Setup { kind, tab: id });
        self.ssh.on = false;
        self.tools.on = false;
        self.panel = super::Panel::Chat;
        cx.notify();

        // Watch for the end: success closes the shell; then connect / re-check.
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let done = this.update_in(cx, |this, window, cx| {
                    if this.setup.as_ref().map(|s| s.tab) != Some(id) {
                        return true; // closed, or another setup took over
                    }
                    if !this.setup_exited() {
                        return false;
                    }
                    let failed = this.setup_tab().is_some_and(|t| {
                        this.ssh.tabs[t]
                            .session
                            .exit_note
                            .lock()
                            .unwrap()
                            .as_deref()
                            .is_some_and(|n| n.contains("code"))
                    });
                    match kind {
                        SetupKind::Hermes if connection::hermes_installed() && !failed => {
                            this.close_setup(cx);
                            this.hermes_missing = false;
                            this.connect(Target::Local, window, cx);
                        }
                        SetupKind::ClaudeLogin if !failed => {
                            this.close_setup(cx);
                            this.check_claude(window, cx);
                        }
                        // A failure keeps the terminal with its log; "Try again" restarts.
                        _ => {}
                    }
                    cx.notify();
                    true
                });
                if done.unwrap_or(true) {
                    break;
                }
            }
        })
        .detach();
    }

    /// Close the setup's terminal (and stop its command).
    pub fn close_setup(&mut self, cx: &mut Context<Self>) {
        if let Some(t) = self.setup_tab() {
            let tab = self.ssh.tabs.remove(t);
            tab.session.kill();
            self.ssh.active = match self.ssh.active {
                Some(a) if a == t => None,
                Some(a) if a > t => Some(a - 1),
                a => a,
            };
        }
        self.setup = None;
        cx.notify();
    }
}
