//! The thread's state and how events change it.

use std::collections::HashSet;

use protocol::{Hold, Process, QueuedItem, StreamKind, ThreadEvent, ThreadItem, ThreadOpen};
use serde_json::{Map, Value};

use crate::rows::{Row, rows_of};

/// A message shown on Enter, before the agent has it: Bench's id for it and its words.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pressed {
    pub request: String,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct Thread {
    /// The journal, ordered by `ThreadItem::order`.
    log: Vec<ThreadItem>,
    keys: HashSet<String>,
    queue: Vec<QueuedItem>,
    inserts: Map<String, Value>,
    running: bool,
    /// The live run's holds (the person's pauses); the last one open while it stands held.
    holds: Vec<Hold>,
    /// The background processes, as their last record said (`thread.process`).
    processes: Vec<Process>,
    /// The answer being written: (reasoning, text).
    stream: (String, String),
    /// Words the model wrote while working, whose message went on to call a tool: narration,
    /// shown inside the work line and staying there when the entry commits.
    narrated: String,
    /// The last entry id seen: `thread.open {since: head}` after a reconnect.
    head: u64,
    /// Messages the person sent that the journal has not shown yet, in the order sent.
    sent: Vec<Pressed>,
    /// Messages pressed while the agent works that its queue has not returned yet: shown as
    /// waiting rows of their own, without controls, until the queue answers or takes them.
    sent_queue: Vec<Pressed>,
    /// Grows with every change, so a view knows when to rebuild its rows.
    version: u64,
}

impl Thread {
    pub fn open(o: ThreadOpen) -> Self {
        let mut t = Thread {
            queue: o.queue,
            inserts: o.inserts,
            running: o.running,
            holds: o.holds,
            processes: o.processes,
            head: o.head,
            ..Default::default()
        };
        t.append(o.items);
        t
    }

    /// What `thread.open {since: head}` brought after a reconnect: the missed items and the
    /// state as it is now.
    pub fn catch_up(&mut self, o: ThreadOpen) {
        self.queue = o.queue;
        self.inserts = o.inserts;
        self.running = o.running;
        self.holds = o.holds;
        self.processes = o.processes;
        if !o.running {
            self.stream = Default::default();
        }
        self.append(o.items);
        self.head = self.head.max(o.head);
        self.version += 1;
    }

    pub fn apply(&mut self, e: ThreadEvent) {
        match e {
            ThreadEvent::Append(items) => self.append(items),
            ThreadEvent::Stream(kind, delta) => match kind {
                StreamKind::Thought => self.stream.0.push_str(&delta),
                StreamKind::Text => self.stream.1.push_str(&delta),
                StreamKind::Narration => {
                    if delta.is_empty() {
                        // The words streamed so far are narration: they move to the work line.
                        self.narrated += &std::mem::take(&mut self.stream.1);
                    } else {
                        self.narrated += &delta;
                    }
                }
            },
            ThreadEvent::Queue(q) => {
                self.queue = q;
                // The queue came back holding the message: the row shown on Enter was it.
                self.sent_queue.retain(|p| {
                    !self
                        .queue
                        .iter()
                        .any(|q| q.request.as_deref() == Some(p.request.as_str()))
                });
            }
            ThreadEvent::Inserts(m) => self.inserts = m,
            ThreadEvent::Run(on) => {
                self.running = on;
                if !on {
                    self.stream = Default::default();
                    self.narrated.clear();
                    self.holds.clear();
                }
            }
            ThreadEvent::Holds(h) => self.holds = h,
            ThreadEvent::Process(p) => match self.processes.iter_mut().find(|x| x.id == p.id) {
                Some(x) => *x = p,
                None => self.processes.push(p),
            },
        }
        self.version += 1;
    }

    fn append(&mut self, items: Vec<ThreadItem>) {
        for it in items {
            if !self.keys.insert(it.key().to_string()) {
                continue;
            }
            // What the model wrote is now an entry: the stream it came from is done.
            if matches!(
                it,
                ThreadItem::Text { .. } | ThreadItem::Thought { .. } | ThreadItem::Tool { .. }
            ) {
                self.stream = Default::default();
                self.narrated.clear();
            }
            // A message the agent took leaves the queue in the same step it joins the journal,
            // even before the queue's own event arrives — known by its submission; the row shown
            // on Enter by its request. Never by its words: two messages may say the same.
            if let ThreadItem::User { sub, request, .. } = &it {
                if let Some(sub) = sub {
                    self.queue.retain(|q| &q.sub != sub);
                }
                if let Some(r) = request {
                    self.sent.retain(|p| &p.request != r);
                    self.sent_queue.retain(|p| &p.request != r);
                }
            }
            self.head = self.head.max(it.seq());
            let at = self.log.partition_point(|x| x.order() <= it.order());
            self.log.insert(at, it);
        }
        self.version += 1;
    }

    /// The person sent `text` with Bench's id `request`: it shows at once, where the journal
    /// will put it; the journal's entry with the same request takes its place.
    pub fn sent(&mut self, request: String, text: String) {
        self.sent.push(Pressed { request, text });
        self.version += 1;
    }

    /// Enter pressed while the run lives: the message is on its way to the queue; show it
    /// as waiting at once, so the composer never empties onto nothing.
    pub fn queued_now(&mut self, request: String, text: String) {
        self.sent_queue.push(Pressed { request, text });
        self.version += 1;
    }

    /// Every sent message the journal has not shown: the send failed.
    pub fn clear_sent(&mut self) {
        self.sent.clear();
        self.sent_queue.clear();
        self.version += 1;
    }

    /// A sent message that never reached the journal (the send failed): it goes.
    pub fn unsent(&mut self, request: &str) {
        self.sent.retain(|p| p.request != request);
        self.version += 1;
    }

    /// The thread's lines, in order.
    pub fn rows(&self) -> Vec<Row> {
        rows_of(
            &self.log,
            &self.inserts,
            self.running,
            &self.holds,
            &self.stream,
            &self.narrated,
            &self.sent,
            &self.queue,
            &self.sent_queue,
            &self.processes,
        )
    }

    /// How many of the person's messages the journal has, plus those sent and on their way:
    /// an insert put now after message n sits there (`after` in its state).
    pub fn messages(&self) -> usize {
        self.log
            .iter()
            .filter(|i| matches!(i, ThreadItem::User { .. }))
            .count()
            + self.sent.len()
            + self.queue.len()
    }

    /// Whether a background process of the thread still runs.
    pub fn processes_running(&self) -> bool {
        self.processes
            .iter()
            .any(|p| p.state == protocol::ProcessState::Running)
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// The run stands held: the person paused it and has not let it go on.
    pub fn paused(&self) -> bool {
        self.running && self.holds.last().is_some_and(|h| h.to.is_none())
    }

    pub fn head(&self) -> u64 {
        self.head
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn queue(&self) -> &[QueuedItem] {
        &self.queue
    }

    pub fn inserts(&self) -> &Map<String, Value> {
        &self.inserts
    }

    pub fn is_empty(&self) -> bool {
        self.log.is_empty() && self.queue.is_empty()
    }
}
