//! Settings → Profiles: the gateway's profiles, each one's SOUL.md, rename, delete; a new one
//! is a clone of the profile picked in the status bar (no dialog).

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::model::Profile;
use crate::rest;

impl HermesApp {
    pub(super) fn load_profiles_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest(
            window,
            cx,
            rest::profile_rows,
            |this, r, window, cx| match r {
                Ok(rows) => {
                    this.prof_rows = rows;
                    let open = this
                        .prof_open
                        .clone()
                        .filter(|n| this.prof_rows.iter().any(|p| &p.name == n))
                        .or_else(|| this.profile.clone());
                    if let Some(name) = open {
                        this.open_profile_admin(name, window, cx);
                    }
                    cx.notify();
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Profiles: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    pub fn open_profile_admin(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prof_open = Some(name.clone());
        self.prof_delete_armed = false;
        let label = self
            .prof_rows
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.label.clone())
            .unwrap_or_else(|| name.clone());
        self.prof_name
            .update(cx, |s, cx| s.set_value(label, window, cx));
        self.soul_editor
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.rest(
            window,
            cx,
            // The gateway admits /api/profiles/<name>/… only on a ticket minted for <name>,
            // so every call below rides the open profile's grant, not the status bar's.
            move |c, _| rest::profile_soul(c, &name, &name),
            |this, r, window, cx| match r {
                Ok(text) => this
                    .soul_editor
                    .update(cx, |s, cx| s.set_value(text, window, cx)),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("SYSTEM.md: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
        cx.notify();
    }

    /// A copy of the status bar's profile, named "<it>-copy" (renamed later in place).
    pub fn new_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = self.current_profile();
        let from = if from.is_empty() {
            "default".to_string()
        } else {
            from
        };
        let mut name = format!("{from}-copy");
        let mut k = 2;
        while self.prof_rows.iter().any(|p| p.name == name) {
            name = format!("{from}-copy-{k}");
            k += 1;
        }
        let (n, f) = (name.clone(), from.clone());
        self.toast(
            NotificationType::Info,
            format!("Creating {name} from {from}…"),
            window,
            cx,
        );
        self.rest(
            window,
            cx,
            // Any served profile may mint the create grant; the clone source is `from`.
            move |c, p| rest::create_profile(c, p, &n, &f),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    this.profiles.push(Profile {
                        name: name.clone(),
                        label: name.clone(),
                        model: String::new(),
                    });
                    this.prof_open = Some(name);
                    this.load_profiles_admin(window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("New agent: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Save SOUL.md, and rename when the name field changed.
    pub fn save_profile_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.prof_open.clone() else {
            return;
        };
        let label = self
            .prof_rows
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.label.clone())
            .unwrap_or_else(|| name.clone());
        let wanted = self.prof_name.read(cx).value().trim().to_string();
        let soul = self.soul_editor.read(cx).value().to_string();
        let rename = (!wanted.is_empty() && wanted != label).then_some(wanted);
        let n = name.clone();
        self.rest(
            window,
            cx,
            move |c, _| {
                rest::save_profile_soul(c, &n, &n, &soul)?;
                match rename {
                    Some(new) => rest::rename_profile(c, &n, &n, &new).map(Some),
                    None => Ok(None),
                }
            },
            move |this, r, window, cx| match r {
                Ok(renamed) => {
                    if let Some(new) = renamed.filter(|new| *new != name) {
                        for p in this.profiles.iter_mut().filter(|p| p.name == name) {
                            p.name = new.clone();
                            p.label = new.clone();
                        }
                        if this.profile.as_deref() == Some(&name) {
                            this.profile = Some(new.clone());
                        }
                        this.prof_open = Some(new);
                    }
                    this.toast(NotificationType::Success, "Saved", window, cx);
                    this.load_profiles_admin(window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Agent: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// First press arms, second deletes the profile (its folder, chats and skills).
    pub fn delete_profile_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.prof_open.clone() else {
            return;
        };
        let is_default = self
            .prof_rows
            .iter()
            .any(|p| p.name == name && p.is_default);
        if is_default || self.profile.as_deref() == Some(&name) {
            self.toast(
                NotificationType::Warning,
                "Switch the status bar to another profile first (default cannot be deleted)",
                window,
                cx,
            );
            return;
        }
        if !self.prof_delete_armed {
            self.prof_delete_armed = true;
            cx.notify();
            return;
        }
        self.prof_delete_armed = false;
        let n = name.clone();
        self.rest(
            window,
            cx,
            move |c, _| rest::delete_profile(c, &n, &n),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    this.profiles.retain(|p| p.name != name);
                    this.prof_open = None;
                    this.toast(
                        NotificationType::Success,
                        format!("Deleted {name}"),
                        window,
                        cx,
                    );
                    this.load_profiles_admin(window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Delete: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }
}
