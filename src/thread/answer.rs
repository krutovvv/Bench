//! The agent asks (the `clarify` tool): answer with a choice, several, your own words, or decline.

use gpui_kit::*;

use super::ThreadView;
use crate::client::rpc::Cmd;

impl ThreadView {
    /// `answer` empty = declined (the agent goes on without it).
    pub fn answer_clarify(&mut self, answer: String, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(req), Some(profile)) = (self.thread.clarify.take(), self.profile(cx)) else {
            return;
        };
        if let Some(live) = self.thread.live.as_mut() {
            live.label = "Working…".into();
        }
        self.thread.clarify_picked.clear();
        self.clarify_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.cmd(
            Cmd::Clarify {
                profile,
                req,
                answer,
            },
            cx,
        );
        cx.notify();
    }

    /// Multi-select: tick or untick a choice.
    pub fn toggle_clarify_choice(&mut self, k: usize, cx: &mut Context<Self>) {
        match self.thread.clarify_picked.iter().position(|x| *x == k) {
            Some(at) => {
                self.thread.clarify_picked.remove(at);
            }
            None => self.thread.clarify_picked.push(k),
        }
        cx.notify();
    }

    /// Multi-select: send the ticked choices, comma-separated, in their order.
    pub fn send_clarify_picks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(req) = self.thread.clarify.as_ref() else {
            return;
        };
        let mut picked = self.thread.clarify_picked.clone();
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
