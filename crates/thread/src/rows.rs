//! The journal as rows: messages, pieces of work, answers, inserts, the queue.

use std::collections::HashSet;

use protocol::{QueuedItem, ThreadItem};
use serde_json::{Map, Value};

/// One line of the thread. `key` is stable: a view keeps a row's state (folded, measured) by it.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// The person's message.
    User {
        key: String,
        text: String,
        at: Option<f64>,
    },
    /// The agent's steps between two messages: folded into "N actions · time".
    Work {
        key: String,
        steps: Vec<Step>,
        from: Option<f64>,
        to: Option<f64>,
        live: bool,
    },
    /// What the agent said: an answer, a plan, a question — each its own message.
    Answer {
        key: String,
        text: String,
        error: Option<String>,
    },
    Insert {
        key: String,
        id: String,
        insert_key: String,
    },
    /// Why a run stopped.
    Fail { key: String, text: String },
    /// The answer being written now (its text; the model's reasoning is not shown while it streams).
    Writing { text: String },
    /// A message waiting for the agent.
    Queued {
        key: String,
        sub: String,
        mode: String,
        text: String,
    },
}

impl Row {
    pub fn key(&self) -> &str {
        match self {
            Row::User { key, .. }
            | Row::Work { key, .. }
            | Row::Answer { key, .. }
            | Row::Insert { key, .. }
            | Row::Fail { key, .. }
            | Row::Queued { key, .. } => key,
            Row::Writing { .. } => "writing",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Thought(String),
    /// Prose the model wrote between tool calls.
    Note(String),
    Tool {
        call_id: String,
        name: String,
        args: Value,
        preview: String,
        output: Option<String>,
        failed: bool,
        diff: Option<String>,
    },
}

impl Step {
    pub fn is_tool(&self) -> bool {
        matches!(self, Step::Tool { .. })
    }
}

pub(crate) fn rows_of(
    log: &[ThreadItem],
    inserts: &Map<String, Value>,
    running: bool,
    stream: &(String, String),
    sent: &[String],
    queue: &[QueuedItem],
) -> Vec<Row> {
    // An assistant entry with tool calls is work: its text is a note on the way, not an answer.
    let working: HashSet<u64> = log
        .iter()
        .filter(|i| matches!(i, ThreadItem::Tool { .. }))
        .map(|i| i.seq())
        .collect();
    let mut rows = Vec::new();
    let mut work: Option<Row> = None;
    // An insert moved after the n-th message (put while that message waited in the composer):
    // (n, its row), shown just before message n + 1.
    let mut moved: Vec<(usize, Row)> = vec![];
    let mut users = 0usize;
    let close = |rows: &mut Vec<Row>, work: &mut Option<Row>| {
        if let Some(w) = work.take() {
            rows.push(w);
        }
    };
    let step = |work: &mut Option<Row>, key: &str, at: Option<f64>, s: Step| {
        let w = work.get_or_insert_with(|| Row::Work {
            key: key.to_string(),
            steps: vec![],
            from: at,
            to: at,
            live: false,
        });
        if let Row::Work {
            steps, from, to, ..
        } = w
        {
            steps.push(s);
            if from.is_none() {
                *from = at;
            }
            if at.is_some() {
                *to = at;
            }
        }
    };
    for it in log {
        match it {
            ThreadItem::User { key, text, at, .. } => {
                close(&mut rows, &mut work);
                rows.extend(take_moved(&mut moved, users));
                users += 1;
                rows.push(Row::User {
                    key: key.clone(),
                    text: text.clone(),
                    at: *at,
                });
            }
            ThreadItem::Thought { key, text, at, .. } => {
                step(&mut work, key, *at, Step::Thought(text.clone()))
            }
            ThreadItem::Text {
                key,
                seq,
                text,
                at,
                error,
            } => {
                if error.is_none() && working.contains(seq) {
                    step(&mut work, key, *at, Step::Note(text.clone()));
                } else {
                    close(&mut rows, &mut work);
                    rows.push(Row::Answer {
                        key: key.clone(),
                        text: text.clone(),
                        error: error.clone(),
                    });
                }
            }
            ThreadItem::Tool {
                key,
                call_id,
                name,
                args,
                preview,
                at,
                ..
            } => step(
                &mut work,
                key,
                *at,
                Step::Tool {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    args: args.clone(),
                    preview: preview.clone(),
                    output: None,
                    failed: false,
                    diff: None,
                },
            ),
            ThreadItem::Result {
                call_id,
                output,
                failed,
                diff,
                at,
                ..
            } => {
                if let Some(Row::Work { steps, to, .. }) = work.as_mut() {
                    for s in steps.iter_mut().rev() {
                        if let Step::Tool {
                            call_id: c,
                            output: o,
                            failed: f,
                            diff: d,
                            ..
                        } = s
                        {
                            if c == call_id {
                                *o = Some(output.clone());
                                *f = *failed;
                                *d = diff.clone();
                                break;
                            }
                        }
                    }
                    if at.is_some() {
                        *to = *at;
                    }
                }
            }
            ThreadItem::Insert {
                key,
                id,
                insert_key,
                ..
            } => {
                let removed = inserts
                    .get(id)
                    .and_then(|s| s.get("removed"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !removed {
                    let state = inserts.get(id);
                    let insert_key = state
                        .and_then(|s| s.get("key"))
                        .and_then(Value::as_str)
                        .unwrap_or(insert_key);
                    let row = Row::Insert {
                        key: key.clone(),
                        id: id.clone(),
                        insert_key: insert_key.to_string(),
                    };
                    match state.and_then(|s| s.get("after")).and_then(Value::as_u64) {
                        Some(n) if n as usize > users => moved.push((n as usize, row)),
                        _ => {
                            close(&mut rows, &mut work);
                            rows.push(row);
                        }
                    }
                }
            }
            ThreadItem::Fail { key, text, .. } => {
                close(&mut rows, &mut work);
                rows.push(Row::Fail {
                    key: key.clone(),
                    text: text.clone(),
                });
            }
        }
    }
    // What the model is thinking now: a step of the work in progress, folded with it like the
    // thinking already in the journal; the thread shows only the work's one line until opened.
    let thinking = (running && !stream.0.is_empty()).then(|| Step::Thought(stream.0.clone()));
    // The work in progress is live while the run is: a view counts its time on from `from`.
    let had_work = work.is_some();
    if let Some(Row::Work { live, steps, .. }) = work.as_mut() {
        *live = running;
        steps.extend(thinking.clone());
    }
    close(&mut rows, &mut work);
    rows.extend(moved.into_iter().map(|(_, r)| r));
    // Thinking with no work row to hold it yet (before the first step on a message, or after an
    // answer): a live work row of its own; right after a message it shows "Thinking…" even empty.
    if running
        && !had_work
        && (thinking.is_some()
            || (stream.1.is_empty() && matches!(rows.last(), Some(Row::User { .. }))))
    {
        rows.push(Row::Work {
            key: "thinking".into(),
            steps: thinking.into_iter().collect(),
            from: None,
            to: None,
            live: true,
        });
    }
    if running && !stream.1.is_empty() {
        rows.push(Row::Writing {
            text: stream.1.clone(),
        });
    }
    rows.extend(sent.iter().enumerate().map(|(k, t)| Row::User {
        key: format!("sent-{k}"),
        text: t.clone(),
        at: None,
    }));
    rows.extend(queue.iter().map(|q| Row::Queued {
        key: q.key.clone(),
        sub: q.sub.clone(),
        mode: q.mode.clone(),
        text: q.text.clone(),
    }));
    rows
}

/// The moved inserts whose place is after message `n`, in the order they were put.
fn take_moved(moved: &mut Vec<(usize, Row)>, n: usize) -> Vec<Row> {
    let (now, later): (Vec<_>, Vec<_>) = std::mem::take(moved)
        .into_iter()
        .partition(|(at, _)| *at <= n);
    *moved = later;
    now.into_iter().map(|(_, r)| r).collect()
}
