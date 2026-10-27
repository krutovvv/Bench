//! The thread's state and how events change it.

use std::collections::HashSet;

use protocol::{QueuedItem, StreamKind, ThreadEvent, ThreadItem, ThreadOpen};
use serde_json::{Map, Value};

use crate::rows::{Row, rows_of};

#[derive(Clone, Debug, Default)]
pub struct Thread {
    /// The journal, ordered by `ThreadItem::order`.
    log: Vec<ThreadItem>,
    keys: HashSet<String>,
    queue: Vec<QueuedItem>,
    inserts: Map<String, Value>,
    running: bool,
    /// The person held the run at a step boundary; it waits until they let it go on.
    paused: bool,
    /// The answer being written: (reasoning, text).
    stream: (String, String),
    /// Words the model wrote while working, whose message went on to call a tool: narration,
    /// shown inside the work line and staying there when the entry commits.
    narrated: String,
    /// The last entry id seen: `thread.open {since: head}` after a reconnect.
    head: u64,
    /// Messages the person sent that the journal has not shown yet, in the order sent.
    sent: Vec<String>,
    /// Messages pressed while the agent works that its queue has not returned yet: shown as
    /// waiting rows of their own, without controls, until the queue answers or takes them.
    sent_queue: Vec<String>,
    /// Grows with every change, so a view knows when to rebuild its rows.
    version: u64,
}

impl Thread {
    pub fn open(o: ThreadOpen) -> Self {
        let mut t = Thread {
            queue: o.queue,
            inserts: o.inserts,
            running: o.running,
            paused: o.paused,
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
        self.paused = o.paused;
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
                // The queue came back holding the words: the waiting echo was its, it is one row now.
                self.sent_queue
                    .retain(|s| !self.queue.iter().any(|q| &q.text == s));
            }
            ThreadEvent::Inserts(m) => self.inserts = m,
            ThreadEvent::Run(on) => {
                self.running = on;
                if !on {
                    self.stream = Default::default();
                    self.narrated.clear();
                    self.paused = false;
                }
            }
            ThreadEvent::Paused(on) => self.paused = on,
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
            // A queued message the agent took: it leaves the queue in the same step it joins the
            // journal, even before the queue's own event arrives, so it is never shown twice.
            if let ThreadItem::User { text, .. } = &it {
                if let Some(k) = self.queue.iter().position(|q| &q.text == text) {
                    self.queue.remove(k);
                }
                if let Some(k) = self.sent.iter().position(|s| s == text) {
                    self.sent.remove(k);
                }
                if let Some(k) = self.sent_queue.iter().position(|s| s == text) {
                    self.sent_queue.remove(k);
                }
            }
            self.head = self.head.max(it.seq());
            let at = self.log.partition_point(|x| x.order() <= it.order());
            self.log.insert(at, it);
        }
        self.version += 1;
    }

    /// The person sent `text`: it shows at once, where the journal will put it.
    pub fn sent(&mut self, text: String) {
        self.sent.push(text);
        self.version += 1;
    }

    /// Enter pressed while the run lives: the message is on its way to the queue; show it
    /// as waiting at once, so the composer never empties onto nothing.
    pub fn queued_now(&mut self, text: String) {
        self.sent_queue.push(text);
        self.version += 1;
    }

    /// A sent message's text became what is actually sent (its files' places once uploaded): the
    /// journal's entry then matches it and takes its place.
    pub fn resent(&mut self, from: &str, to: String) {
        match self.sent.iter_mut().find(|s| *s == from) {
            Some(s) => *s = to,
            None => self.sent.push(to),
        }
        self.version += 1;
    }

    /// Every sent message the journal has not shown: the send failed.
    pub fn clear_sent(&mut self) {
        self.sent.clear();
        self.sent_queue.clear();
        self.version += 1;
    }

    /// A sent message that never reached the journal (the send failed): it goes.
    pub fn unsent(&mut self, text: &str) {
        self.sent.retain(|s| s != text);
        self.version += 1;
    }

    /// The thread's lines, in order.
    pub fn rows(&self) -> Vec<Row> {
        rows_of(
            &self.log,
            &self.inserts,
            self.running,
            self.paused,
            &self.stream,
            &self.narrated,
            &self.sent,
            &self.queue,
            &self.sent_queue,
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

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn paused(&self) -> bool {
        self.paused
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
