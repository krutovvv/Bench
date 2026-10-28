//! The agent's defaults for what Bench shows before a thread says its own: the reasoning level
//! a new chat starts on, the models the picker offers, the approval mode, the context window.

use gpui_kit::component::IndexPath;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rest;

impl BenchApp {
    /// Read this Mac's agent's defaults; a new chat starts on its reasoning level.
    pub(crate) fn load_agent_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active_thread
            .update(cx, |v, _| v.thread.effort_pending = false);
        self.settings_ready = false;
        self.rest(window, cx, rest::agent_defaults, |this, r, window, cx| {
            this.settings_ready = true;
            let Ok((effort, _model, models, approval, context)) = r else {
                return;
            };
            this.approval_mode = Some(approval);
            this.context_length = context;
            this.models = models;
            this.add_picker_models();
            let level = effort.and_then(|v| this.efforts.iter().position(|e| *e == v));
            if let Some(i) = level {
                this.default_effort = i;
            }
            // The default is for new chats: an open thread already has its own.
            if let Some(i) = level.filter(|_| this.thread(cx).id.is_none()) {
                this.active_thread.update(cx, |v, _| {
                    v.thread.effort = i;
                    v.thread.effort_other = None;
                    v.thread.effort_pending = false;
                });
                this.effort_choice.update(cx, |s, cx| {
                    s.set_selected_index(Some(IndexPath::new(i)), window, cx)
                });
            }
        });
    }
}
