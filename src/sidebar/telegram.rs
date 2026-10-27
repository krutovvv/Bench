//! Which Telegram threads the sidebar shows, and the profile's reasoning level.

use std::collections::HashSet;

use gpui_kit::component::IndexPath;
use gpui_kit::*;

use crate::client::connection;
use crate::client::rest;

use crate::app::BenchApp;

impl BenchApp {
    /// Start each profile on its configured reasoning effort.
    pub(crate) fn load_profile_effort(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.effort_pending = false;
        self.profile_ready = false;
        self.rest(window, cx, rest::profile_defaults, |this, r, window, cx| {
            this.profile_ready = true;
            let Ok((effort, model, models, approval, context)) = r else {
                return;
            };
            this.approval_mode = Some(approval);
            this.profile_context = context;
            this.profile_models = models;
            this.add_picker_models();
            let level = effort.and_then(|v| this.efforts.iter().position(|e| *e == v));
            if let Some(i) = level {
                this.profile_effort = i;
            }
            // The profile's level is for new chats: an open session already has its own.
            if let Some(i) = level.filter(|_| this.runtime.is_none()) {
                this.effort = i;
                this.effort_other = None;
                this.effort_pending = false;
                this.effort_choice.update(cx, |s, cx| {
                    s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                });
            }
            if let (Some(model), Some(p)) = (model, this.profile.clone()) {
                if let Some(profile) = this.profiles.iter_mut().find(|x| x.name == p) {
                    profile.model = model;
                }
            }
        });
    }

    pub(crate) fn telegram_prefs_path(&self) -> std::path::PathBuf {
        let key: String = format!("{}-{}", self.target.label(), self.current_profile())
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        connection::state_dir().join(format!("telegram-{key}.json"))
    }

    pub(crate) fn load_telegram_owners(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest(window, cx, rest::telegram_owners, |this, r, _, cx| {
            let Ok(rows) = r else { return };
            for (session, user, name) in rows {
                if !this.tg_people.iter().any(|(u, _)| *u == user) {
                    this.tg_people.push((user.clone(), name));
                }
                this.tg_owner.insert(session, user);
            }
            if this.tg_visible.is_none() {
                this.tg_visible = this.initial_telegram_visible();
            }
            cx.notify();
        });
    }

    /// Saved choice, else the account whose name matches this Mac's user, else everyone.
    pub(crate) fn initial_telegram_visible(&self) -> Option<HashSet<String>> {
        if let Ok(text) = std::fs::read_to_string(self.telegram_prefs_path()) {
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(&text) {
                return Some(ids.into_iter().collect());
            }
        }
        let me = std::process::Command::new("id")
            .arg("-F")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase())
            .unwrap_or_default();
        let first = me.split_whitespace().next().unwrap_or_default().to_string();
        let mine: HashSet<String> = self
            .tg_people
            .iter()
            .filter(|(_, name)| {
                !first.is_empty() && name.to_lowercase().split_whitespace().any(|w| w == first)
            })
            .map(|(u, _)| u.clone())
            .collect();
        (!mine.is_empty()).then_some(mine)
    }

    /// Indices into `sessions` the list should show (other people's Telegram chats filtered out).
    pub fn visible_sessions(&self) -> Vec<usize> {
        (0..self.sessions.len())
            .filter(|&i| {
                let s = &self.sessions[i];
                match (&self.tg_visible, s.source.as_str()) {
                    (Some(vis), "telegram") => {
                        self.tg_owner.get(&s.id).is_none_or(|u| vis.contains(u))
                    }
                    _ => true,
                }
            })
            .collect()
    }
}
