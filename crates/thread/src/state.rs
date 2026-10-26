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
    /// The answer being written: (reasoning, text).
    stream: (String, String),
    /// The last entry id seen: `thread.open {since: head}` after a reconnect.
    head: u64,
    /// Grows with every change, so a view knows when to rebuild its rows.
    version: u64,
}

impl Thread {
    pub fn open(o: ThreadOpen) -> Self {
        let mut t = Thread {
            queue: o.queue,
            inserts: o.inserts,
            running: o.running,
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
            },
            ThreadEvent::Queue(q) => self.queue = q,
            ThreadEvent::Inserts(m) => self.inserts = m,
            ThreadEvent::Run(on) => {
                self.running = on;
                if !on {
                    self.stream = Default::default();
                }
            }
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
            }
            // A queued message the agent took: it leaves the queue in the same step it joins the
            // journal, even before the queue's own event arrives, so it is never shown twice.
            if let ThreadItem::User { text, .. } = &it {
                if let Some(k) = self.queue.iter().position(|q| &q.text == text) {
                    self.queue.remove(k);
                }
            }
            self.head = self.head.max(it.seq());
            let at = self.log.partition_point(|x| x.order() <= it.order());
            self.log.insert(at, it);
        }
        self.version += 1;
    }

    /// The thread's lines, in order.
    pub fn rows(&self, now_ms: f64) -> Vec<Row> {
        rows_of(
            &self.log,
            &self.inserts,
            self.running,
            &self.stream,
            &self.queue,
            now_ms,
        )
    }

    pub fn running(&self) -> bool {
        self.running
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
