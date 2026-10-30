//! The journal as rows: messages, pieces of work (with the plan and the questions that grew
//! inside them), answers, inserts, the queue.

use std::collections::HashMap;

use protocol::{FileChange, Finding, Hold, Inserts, PlanStep, Process, QueuedItem, ThreadItem};
use serde_json::Value;

use crate::state::Pressed;

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
        /// The live run's holds (the person's pauses): its clock skips them, and the last one
        /// open means it stands held. Empty for finished work.
        holds: Vec<Hold>,
        /// The plan this work keeps: its first `todo` call's id, and the steps of its newest.
        plan: Option<(String, Vec<PlanStep>)>,
        /// The agent's questions put in this work, settled or still waiting.
        asks: Vec<AskCard>,
        /// The background processes its calls started, running or ended: they stay at their
        /// call, whatever came after.
        processes: Vec<Process>,
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
    /// What a turn changed in its repository, under its answer: the trees at its start and end
    /// (a file's diff of the turn is read from them) and the files.
    Changes {
        key: String,
        root: String,
        base: String,
        head: String,
        files: Vec<FileChange>,
        /// Its review, once made (`ThreadItem::Review` of this key): findings, or why not.
        review: Option<(Vec<Finding>, Option<String>)>,
    },
    /// The answer being written now (its text; the model's reasoning is not shown while it streams).
    Writing { text: String },
    /// Pi stopped the work after too many restarts: it waits to be asked to run it again.
    Parked,
    /// A message waiting for the agent.
    Queued {
        key: String,
        sub: String,
        mode: protocol::QueueMode,
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

/// How long work from `from` to `until` (ms) took, less what the person held of it: a held
/// run's clock stands still and goes on from where it stood.
pub fn worked_ms(from: f64, until: f64, holds: &[Hold]) -> f64 {
    let held: f64 = holds
        .iter()
        .map(|h| (h.to.unwrap_or(until).min(until) - h.from.max(from)).max(0.))
        .sum();
    (until - from - held).max(0.)
}

impl Row {
    /// Work whose run stands held now.
    pub fn held(&self) -> bool {
        matches!(self, Row::Work { live: true, holds, .. } if holds.last().is_some_and(|h| h.to.is_none()))
    }

    pub fn key(&self) -> &str {
        match self {
            Row::User { key, .. }
            | Row::Work { key, .. }
            | Row::Answer { key, .. }
            | Row::Insert { key, .. }
            | Row::Fail { key, .. }
            | Row::Changes { key, .. }
            | Row::Queued { key, .. } => key,
            Row::Writing { .. } => "writing",
            Row::Parked => "parked",
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

/// Every row stands on a journal item; the plan and the questions are the agent's own calls
/// (`todo`, `clarify`) inside the work that made them, and so are the background processes,
/// at the `process` call that started each (src/thread/SPEC.md).
pub(crate) fn rows_of(
    log: &[ThreadItem],
    inserts: &Inserts,
    running: bool,
    holds: &[Hold],
    stream: &(String, String),
    narrated: &str,
    sent: &[Pressed],
    queue: &[QueuedItem],
    sent_queue: &[Pressed],
    processes: &[Process],
    parked: bool,
) -> Vec<Row> {
    // A question's answer is its call's result, which comes later in the journal.
    let asked: HashMap<&str, &str> = log
        .iter()
        .filter_map(|it| match it {
            ThreadItem::Asked { id, answer, .. } => {
                Some((id.as_str(), answer.as_deref().unwrap_or("")))
            }
            _ => None,
        })
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
            holds: vec![],
            plan: None,
            asks: vec![],
            processes: vec![],
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
    // The plan and the questions join the work whose calls they are.
    let attach = |work: &mut Option<Row>, key: &str, put: &dyn Fn(&mut Row)| {
        let w = work.get_or_insert_with(|| Row::Work {
            key: key.to_string(),
            steps: vec![],
            from: None,
            to: None,
            live: false,
            holds: vec![],
            plan: None,
            asks: vec![],
            processes: vec![],
        });
        put(w);
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
                text,
                at,
                error,
                narration,
                ..
            } => {
                // The words of a message that also called a tool are notes on the way; the
                // agent reads this from the entry's own blocks (`narration`), nothing is guessed.
                if error.is_none() && narration.unwrap_or(false) {
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
            } => {
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
                let state = inserts.get(id);
                if !state.is_some_and(|s| s.removed) {
                    // The document's key wins: an insert moved to another server keeps its entry.
                    let insert_key = state.map_or(insert_key.as_str(), |s| s.key.as_str());
                    let row = Row::Insert {
                        key: key.clone(),
                        id: id.clone(),
                        insert_key: insert_key.to_string(),
                    };
                    match state.and_then(|s| s.after) {
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
            ThreadItem::Changes {
                key,
                root,
                base,
                head,
                files,
                ..
            } => {
                close(&mut rows, &mut work);
                rows.push(Row::Changes {
                    key: key.clone(),
                    root: root.clone(),
                    base: base.clone(),
                    head: head.clone(),
                    files: files.clone(),
                    review: None,
                });
            }
            // A review settles the changes it looked at, where they stand: no row of its own.
            ThreadItem::Review {
                of, findings, error, ..
            } => {
                for r in rows.iter_mut().rev() {
                    if let Row::Changes { key, review, .. } = r {
                        if key == of {
                            *review = Some((findings.clone(), error.clone()));
                            break;
                        }
                    }
                }
            }
            // The plan is the work's first `todo` call; a later one in the same work ticks it.
            ThreadItem::Plan { key, id, todos, .. } => {
                attach(&mut work, key, &|w| {
                    if let Row::Work { plan, .. } = w {
                        match plan {
                            Some((_, steps)) => *steps = todos.clone(),
                            None => *plan = Some((id.clone(), todos.clone())),
                        }
                    }
                });
            }
            ThreadItem::Ask {
                key,
                id,
                ask_kind,
                question,
                detail,
                choices,
                multi,
                ..
            } => {
                let card = AskCard {
                    id: id.clone(),
                    ask_kind: ask_kind.clone(),
                    question: question.clone(),
                    detail: detail.clone(),
                    choices: choices.clone(),
                    multi: *multi,
                    answer: asked.get(id.as_str()).map(|a| a.to_string()),
                };
                attach(&mut work, key, &|w| {
                    if let Row::Work { asks, .. } = w {
                        asks.push(card.clone());
                    }
                });
            }
            ThreadItem::Asked { .. } => {}
        }
    }
    // What the model is thinking now: a step of the work in progress, folded with it like the
    // thinking already in the journal; the thread shows only the work's one line until opened.
    // The narration streamed so far sits where its note will sit when the entry commits:
    // the words do not move when the journal catches up with them.
    let thinking = (running && !stream.0.is_empty()).then(|| Step::Thought(stream.0.clone()));
    let drafting = (running && !narrated.is_empty()).then(|| Step::Note(narrated.to_string()));
    // The work in progress is live while the run is: a view counts its time on from `from`.
    let had_work = work.is_some();
    if let Some(Row::Work {
        live,
        holds: h,
        steps,
        ..
    }) = work.as_mut()
    {
        *live = running;
        if running {
            *h = holds.to_vec();
        }
        steps.extend(thinking.clone());
        steps.extend(drafting.clone());
    }
    close(&mut rows, &mut work);
    rows.extend(moved.into_iter().map(|(_, r)| r));
    // Thinking with no work row to hold it yet (before the first step on a message, or after an
    // answer): a live work row of its own; right after a message it shows "Thinking…" even empty.
    if running
        && !had_work
        && (thinking.is_some()
            || drafting.is_some()
            || (stream.1.is_empty() && matches!(rows.last(), Some(Row::User { .. }))))
    {
        rows.push(Row::Work {
            key: "thinking".into(),
            steps: thinking.into_iter().chain(drafting).collect(),
            from: None,
            to: None,
            live: true,
            holds: holds.to_vec(),
            plan: None,
            asks: vec![],
            processes: vec![],
        });
    }
    if !processes.is_empty() {
        for row in &mut rows {
            if let Row::Work {
                steps,
                processes: mine,
                ..
            } = row
            {
                mine.extend(
                    processes
                        .iter()
                        .filter(|p| {
                            steps.iter().any(
                                |s| matches!(s, Step::Tool { call_id, .. } if *call_id == p.call_id),
                            )
                        })
                        .cloned(),
                );
            }
        }
    }
    if running && !stream.1.is_empty() {
        rows.push(Row::Writing {
            text: stream.1.clone(),
        });
    }
    if parked {
        rows.push(Row::Parked);
    }
    rows.extend(sent.iter().map(|p| Row::User {
        key: format!("sent-{}", p.request),
        text: p.text.clone(),
        at: None,
    }));
    rows.extend(queue.iter().map(|q| Row::Queued {
        key: q.key.clone(),
        sub: q.sub.clone(),
        mode: q.mode,
        text: q.text.clone(),
    }));
    // Words on their way to the queue: the row of a waiting message, but with no controls —
    // the agent has no id for them yet, so there is nothing to withdraw or hurry.
    rows.extend(sent_queue.iter().map(|p| Row::Queued {
        key: format!("sent-queue-{}", p.request),
        sub: String::new(),
        mode: protocol::QueueMode::FollowUp,
        text: p.text.clone(),
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
