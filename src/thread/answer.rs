//! The agent asks (the `clarify` tool): answer with a choice, several, your own words, or decline.

use gpui_kit::*;

use crate::app::BenchApp;
use crate::client::rpc::Cmd;

impl BenchApp {
    /// `answer` empty = declined (the agent goes on without it).
    pub fn answer_clarify(&mut self, answer: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(req), Some(profile)) = (self.active_thread.clarify.take(), self.profile.clone())
        else {
            return;
        };
        if let Some(live) = self.active_thread.live.as_mut() {
            live.label = "Working…".into();
        }
        self.active_thread.clarify_picked.clear();
        self.clarify_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.cmd(Cmd::Clarify {
            profile,
            req,
            answer,
        });
        cx.notify();
    }

    /// Multi-select: tick or untick a choice.
    pub fn toggle_clarify_choice(&mut self, k: usize, cx: &mut Context<Self>) {
        match self
            .active_thread
            .clarify_picked
            .iter()
            .position(|x| *x == k)
        {
            Some(at) => {
                self.active_thread.clarify_picked.remove(at);
            }
            None => self.active_thread.clarify_picked.push(k),
        }
        cx.notify();
    }

    /// Multi-select: send the ticked choices, comma-separated, in their order.
    pub fn send_clarify_picks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(req) = self.active_thread.clarify.as_ref() else {
            return;
        };
        let mut picked = self.active_thread.clarify_picked.clone();
        picked.sort();
        let answer = picked
            .iter()
            .filter_map(|k| req.choices.get(*k).cloned())
            .collect::<Vec<_>>()
            .join(", ");
        if !answer.is_empty() {
            self.answer_clarify(answer, window, cx);
        }
    }
}
