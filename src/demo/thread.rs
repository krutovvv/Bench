//! The demo's hooks on a thread view, for screenshots: a draft in the composer, the thread scrolled
//! to its end or to an item, a queue and a plan waiting (`BENCH_DEMO_DRAFT`, `BENCH_DEMO_SCROLL`,
//! `BENCH_DEMO_QUEUE`). Run by the view only in the demo.

use gpui_kit::*;

use crate::thread::ThreadView;

impl ThreadView {
    /// The demo's hooks on the thread (`BENCH_DEMO_DRAFT`, `BENCH_DEMO_SCROLL`, for screenshots).
    pub(crate) fn demo_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).value().is_empty() {
            if let Some(text) = std::env::var("BENCH_DEMO_DRAFT")
                .ok()
                .filter(|_| !self.demo_draft_done)
            {
                self.demo_draft_done = true;
                self.composer
                    .update(cx, |s, cx| s.set_value(text, window, cx));
            }
        }
        // `BENCH_DEMO_SCROLL=end`: the thread keeps to its end (a tail shot: what grows after the
        // first frame — the queue, an answer — is still in the picture).
        if std::env::var("BENCH_DEMO_SCROLL").as_deref() == Ok("end") {
            self.demo_scrolled = true;
            self.thread.tv.to_end();
        }
        // `BENCH_DEMO_SCROLL=<item>:<px>`: the thread scrolled to there (a reader in the middle).
        if !self.demo_scrolled {
            if let Some((i, o)) = std::env::var("BENCH_DEMO_SCROLL").ok().and_then(|v| {
                let (i, o) = v.split_once(':')?;
                Some((i.parse::<usize>().ok()?, o.parse::<f32>().ok()?))
            }) {
                self.demo_scrolled = true;
                self.thread.tv.list.scroll_to(ListOffset {
                    item_ix: i,
                    offset_in_item: px(o),
                });
            }
        }
        // `BENCH_DEMO_QUEUE=1`: two messages waiting after the last row, one the agent will take
        // up next, one a follow-up, and the plan the agent is working through — which stands
        // above them, with the work it plans.
        if std::env::var("BENCH_DEMO_QUEUE").is_ok() && self.thread.journal.queue().is_empty() {
            self.thread.journal.apply(protocol::ThreadEvent::Queue(vec![
                protocol::QueuedItem {
                    key: "q-1".into(),
                    sub: "q-1".into(),
                    mode: protocol::QueueMode::Steer,
                    text: "And the cover: the one from the spring shoot.".into(),
                    request: None,
                },
                protocol::QueuedItem {
                    key: "q-2".into(),
                    sub: "q-2".into(),
                    mode: protocol::QueueMode::FollowUp,
                    text: "Skip the pricing table for now, it goes in the next release.".into(),
                    request: None,
                },
            ]));
            // A queue like this means a live run: the plan of that run shows, and it shows in
            // the work it belongs to — a journal item, as any plan is. Before the plan: what
            // the agent said aloud and the call it went on to make — the header line keeps the
            // words while the command runs.
            self.thread.journal.apply(protocol::ThreadEvent::Run(true));
            self.thread.journal.apply(protocol::ThreadEvent::Append(vec![
                protocol::ThreadItem::Tool {
                    key: "demo-live-1".into(),
                    seq: u64::MAX - 5,
                    call_id: "demo-live-c".into(),
                    name: "bash".into(),
                    args: serde_json::json!({ "command": "python3 bills/match.py --month oct" }),
                    preview: "python3 bills/match.py --month oct".into(),
                    at: None,
                },
                protocol::ThreadItem::Result {
                    key: "demo-live-2".into(),
                    seq: u64::MAX - 4,
                    call_id: "demo-live-c".into(),
                    name: "bash".into(),
                    output: "3 unmatched".into(),
                    failed: false,
                    diff: None,
                    at: None,
                },
                protocol::ThreadItem::Text {
                    key: "demo-live-3".into(),
                    // The words of a message that went on to call: narration, a note on the
                    // work line (`rows_of`), exactly as the agent marks them.
                    seq: u64::MAX - 2,
                    text: "Reading the billing export, then matching it against the invoices."
                        .into(),
                    at: None,
                    error: None,
                    narration: Some(true),
                },
                protocol::ThreadItem::Tool {
                    key: "demo-live-4".into(),
                    seq: u64::MAX - 2,
                    call_id: "demo-live-c2".into(),
                    name: "bash".into(),
                    args: serde_json::json!({ "command": "grep -c INV-2026 invoices/*.csv" }),
                    preview: "grep -c INV-2026 invoices/*.csv".into(),
                    at: None,
                },
                protocol::ThreadItem::Plan {
                    key: "demo-plan".into(),
                    seq: u64::MAX - 1,
                    id: "demo-plan".into(),
                    todos: ["done", "doing", "next"]
                        .into_iter()
                        .zip(["completed", "in_progress", "pending"])
                        .enumerate()
                        .map(|(k, (c, s))| protocol::PlanStep {
                            id: format!("{k}"),
                            content: format!("{c}: the step the agent wrote for itself"),
                            status: s.into(),
                            parent: None,
                        })
                        .collect(),
                },
            ]));
        }
    }
}
