//! Settings → Gateways: each machine's Hermes version, whether a release is out, and an
//! update button (always with a full backup first: Hermes does not support running an older
//! release on data a newer one wrote, so the backup is the way back).

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::connection;
use crate::model::Target;

impl HermesApp {
    /// Version and update check for every machine (in parallel).
    pub fn check_installs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for target in self.targets.clone() {
            let key = target.label();
            if self.installs.get(&key).is_some_and(|i| i.busy.is_some()) {
                continue;
            }
            let entry = self.installs.entry(key.clone()).or_default();
            entry.busy = Some("Checking…".into());
            entry.error = None;
            self.background(
                window,
                cx,
                move || {
                    let version = connection::hermes_cli(&target, &["--version"])?;
                    let check = connection::hermes_cli(&target, &["update", "--check"]);
                    Ok((version, check))
                },
                move |this, r, _, cx| {
                    let i = this.installs.entry(key).or_default();
                    i.busy = None;
                    match r {
                        Ok((version, check)) => {
                            i.parse_version(&version);
                            match check {
                                Ok(text) => i.parse_check(&text),
                                Err(e) => i.error = Some(format!("{e:#}")),
                            }
                        }
                        Err(e) => i.error = Some(format!("{e:#}")),
                    }
                    cx.notify();
                },
            );
        }
        cx.notify();
    }

    /// `hermes update --yes --backup` on one machine. The gateway restarts at the end, so a
    /// chat connected to it reconnects.
    pub fn update_install(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let key = target.label();
        let entry = self.installs.entry(key.clone()).or_default();
        if entry.busy.is_some() {
            return;
        }
        entry.busy = Some("Updating (backup first, then the gateway restarts)…".into());
        entry.error = None;
        let reconnect = target == self.target;
        self.background(
            window,
            cx,
            move || {
                connection::hermes_cli(&target, &["update", "--yes", "--backup"])?;
                let version = connection::hermes_cli(&target, &["--version"])?;
                Ok((target, version))
            },
            move |this, r, window, cx| {
                let i = this.installs.entry(key.clone()).or_default();
                i.busy = None;
                match r {
                    Ok((target, version)) => {
                        i.parse_version(&version);
                        i.available = None;
                        let v = i.version.clone();
                        this.toast(
                            NotificationType::Success,
                            format!("{key}: Hermes {v}"),
                            window,
                            cx,
                        );
                        if reconnect {
                            this.connect(target, window, cx);
                        }
                    }
                    Err(e) => {
                        i.error = Some(format!("{e:#}"));
                        this.toast(
                            NotificationType::Error,
                            format!("{key}: update failed"),
                            window,
                            cx,
                        );
                    }
                }
                cx.notify();
            },
        );
        cx.notify();
    }
}
