//! The journal as rows: messages, pieces of work (with the plan and the questions that grew
//! inside them), answers, inserts, the queue.

use std::collections::HashMap;

use protocol::{PlanStep, QueuedItem, ThreadItem};
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
    /// The agent's steps between two messages: folded into "N actions · time". The plan it kept
    /// and the questions it put to the person live here, as journal items of the same turn: they
    /// are born in this work, tick and settle in it, and never stand on their own in the list.
    Work {
        key: String,
        steps: Vec<Step>,
        from: Option<f64>,
        to: Option<f64>,
        live: bool,
        /// The live run stands held at a step boundary (the person's pause).
        paused: bool,
        /// The agent's plans written in this work: (id, the newest snapshot of its steps).
        plan: Vec<(String, Vec<PlanStep>)>,
        /// The agent's questions put in this work, settled or still waiting.
        asks: Vec<AskCard>,
    },
    /// What the agent said: its answer.
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

/// The agent's question (the `clarify` tool, an approval) as the thread shows it. `answer` is
/// the person's once an `asked` item settled it; `None` means the agent still waits.
#[derive(Clone, Debug, PartialEq)]
pub struct AskCard {
    pub id: String,
    /// "clarify" or "approve".
    pub ask_kind: String,
    pub question: String,
    /// Extra text under the question: the command an approval wants to run.
    pub detail: Option<String>,
    pub choices: Vec<String>,
    pub multi: bool,
    pub answer: Option<String>,
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

/// The plan and the questions are journal items of the turn that wrote them; no side state,
/// no dates: the journal alone says where they stand.
pub(crate) fn rows_of(
    log: &[ThreadItem],
    inserts: &Map<String, Value>,
    running: bool,
    paused: bool,
    stream: &(String, String),
    sent: &[String],
    queue: &[QueuedItem],
) -> Vec<Row> {
    // Words on the way are notes on the work line: a text is a note only while its turn
    // still has a thought or a call AFTER it — the last words of a turn answer, however much
    // the turn acted before (Pi commits a turn's texts, thoughts and calls under one seq, so
    // "the turn acted" alone would swallow every final answer). While the run is live, the
    // trailing text is still on the way too: another call may follow it.
    let mut actions_left: HashMap<u64, usize> = HashMap::new();
    for i in log {
        if matches!(i, ThreadItem::Tool { .. } | ThreadItem::Thought { .. }) {
            *actions_left.entry(i.seq()).or_default() += 1;
        }
    }
    // Each plan lives once: its place is its first snapshot, its steps the newest one. Each
    // question too, with the answer the journal has for it.
    let mut plan_first: HashMap<&str, u64> = HashMap::new();
    let mut plan_latest: HashMap<&str, Vec<PlanStep>> = HashMap::new();
    let mut ask_first: HashMap<&str, u64> = HashMap::new();
    let mut asked: HashMap<&str, Option<&String>> = HashMap::new();
    for it in log {
        match it {
            ThreadItem::Plan { id, seq, todos, .. } => {
                plan_first.entry(id).or_insert(*seq);
                plan_latest.insert(id, todos.clone());
            }
            ThreadItem::Ask { id, seq, .. } => {
                ask_first.entry(id).or_insert(*seq);
            }
            ThreadItem::Asked { id, answer, .. } => {
                asked.insert(id, answer.as_ref());
            }
            _ => {}
        }
    }
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
            paused: false,
            plan: vec![],
            asks: vec![],
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
    // The plan and the questions join the work that wrote them: the newest snapshot ticks the
    // card at its birth place; nothing here is placed by a clock.
    let attach = |work: &mut Option<Row>, key: &str, put: &dyn Fn(&mut Row)| {
        let w = work.get_or_insert_with(|| Row::Work {
            key: key.to_string(),
            steps: vec![],
            from: None,
            to: None,
            live: false,
            paused: false,
            plan: vec![],
            asks: vec![],
        });
        put(w);
    };
    for (idx, it) in log.iter().enumerate() {
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
            ThreadItem::Thought {
                key, seq, text, at, ..
            } => {
                *actions_left.entry(*seq).or_default() =
                    actions_left.get(seq).copied().unwrap_or(0).saturating_sub(1);
                step(&mut work, key, *at, Step::Thought(text.clone()))
            }
            ThreadItem::Text {
                key,
                seq,
                text,
                at,
                error,
            } => {
                let on_the_way = actions_left.get(seq).copied().unwrap_or(0) > 0
                    || (running && idx + 1 == log.len());
                if error.is_none() && on_the_way {
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
                seq,
                call_id,
                name,
                args,
                preview,
                at,
                ..
            } => {
                *actions_left.entry(*seq).or_default() =
                    actions_left.get(seq).copied().unwrap_or(0).saturating_sub(1);
                step(
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
            );
            }
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
            // The plan and the questions are parts of the work that wrote them: the card stands
            // where its first snapshot was written and ticks from the newest one.
            ThreadItem::Plan { key, seq, id, .. } if plan_first.get(id.as_str()) == Some(seq) => {
                let todos = plan_latest.get(id.as_str()).cloned().unwrap_or_default();
                let key = key.clone();
                let id = id.clone();
                attach(&mut work, &key, &|w| {
                    if let Row::Work { plan, .. } = w {
                        plan.push((id.clone(), todos.clone()));
                    }
                });
            }
            ThreadItem::Ask {
                key,
                seq,
                id,
                ask_kind,
                question,
                detail,
                choices,
                multi,
                ..
            } if ask_first.get(id.as_str()) == Some(seq) => {
                let card = AskCard {
                    id: id.clone(),
                    ask_kind: ask_kind.clone(),
                    question: question.clone(),
                    detail: detail.clone(),
                    choices: choices.clone(),
                    multi: *multi,
                    answer: asked.get(id.as_str()).copied().flatten().cloned(),
                };
                let key = key.clone();
                attach(&mut work, &key, &|w| {
                    if let Row::Work { asks, .. } = w {
                        asks.push(card.clone());
                    }
                });
            }
            ThreadItem::Plan { .. } | ThreadItem::Ask { .. } | ThreadItem::Asked { .. } => {}
        }
    }
    // What the model is thinking now: a step of the work in progress, folded with it like the
    // thinking already in the journal; the thread shows only the work's one line until opened.
    let thinking = (running && !stream.0.is_empty()).then(|| Step::Thought(stream.0.clone()));
    // The work in progress is live while the run is: a view counts its time on from `from`.
    let had_work = work.is_some();
    if let Some(Row::Work { live, paused: p, steps, .. }) = work.as_mut() {
        *live = running;
        *p = if running { paused } else { false };
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
            paused,
            plan: vec![],
            asks: vec![],
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
