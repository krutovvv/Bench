use protocol::{
    PlanStep, Process, ProcessState, QueuedItem, StreamKind, ThreadEvent, ThreadItem, ThreadOpen,
};
use serde_json::json;

use crate::{Row, Step, Thread};

fn user(seq: u64, text: &str) -> ThreadItem {
    ThreadItem::User {
        key: format!("{seq}.0"),
        seq,
        text: text.into(),
        at: Some(seq as f64 * 1000.),
        sub: None,
        request: None,
    }
}
/// The person's message as the agent tells it once placed: its submission and Bench's request.
fn placed(seq: u64, text: &str, sub: u64, request: &str) -> ThreadItem {
    let mut it = user(seq, text);
    if let ThreadItem::User {
        sub: s, request: r, ..
    } = &mut it
    {
        *s = Some(sub.to_string());
        *r = Some(request.into());
    }
    it
}
fn text(seq: u64, sub: u32, t: &str) -> ThreadItem {
    ThreadItem::Text {
        key: format!("{seq}.{sub}"),
        seq,
        text: t.into(),
        at: Some(seq as f64 * 1000.),
        error: None,
        // The agent marks every text: these are the words of a message that called nothing.
        narration: Some(false),
    }
}
/// The agent's own marking: words of a message that also called a tool (Pi's signal).
fn narr(seq: u64, sub: u32, t: &str) -> ThreadItem {
    let mut it = text(seq, sub, t);
    if let ThreadItem::Text { narration, .. } = &mut it {
        *narration = Some(true);
    }
    it
}
fn tool(seq: u64, sub: u32, call: &str) -> ThreadItem {
    ThreadItem::Tool {
        key: format!("{seq}.{sub}"),
        seq,
        call_id: call.into(),
        name: "bash".into(),
        args: json!({}),
        preview: String::new(),
        at: Some(seq as f64 * 1000.),
    }
}
fn result(seq: u64, call: &str) -> ThreadItem {
    ThreadItem::Result {
        key: format!("{seq}.0"),
        seq,
        call_id: call.into(),
        name: "bash".into(),
        output: "ok".into(),
        failed: false,
        diff: None,
        at: Some(seq as f64 * 1000.),
    }
}
fn thought(seq: u64, sub: u32, t: &str) -> ThreadItem {
    ThreadItem::Thought {
        key: format!("{seq}.{sub}"),
        seq,
        text: t.into(),
        at: Some(seq as f64 * 1000.),
    }
}
fn queued(sub: u64, t: &str) -> QueuedItem {
    QueuedItem {
        key: format!("q{sub}"),
        sub: sub.to_string(),
        mode: protocol::QueueMode::FollowUp,
        text: t.into(),
        request: None,
    }
}
fn queued_req(sub: u64, t: &str, request: &str) -> QueuedItem {
    QueuedItem {
        request: Some(request.into()),
        ..queued(sub, t)
    }
}
fn plan(seq: u64, id: &str, steps: &[(&str, &str, &str)]) -> ThreadItem {
    ThreadItem::Plan {
        key: seq.to_string(),
        seq,
        id: id.into(),
        todos: steps
            .iter()
            .map(|(i, c, s)| PlanStep {
                id: i.to_string(),
                content: c.to_string(),
                status: s.to_string(),
                parent: None,
            })
            .collect(),
    }
}
fn ask(seq: u64, id: &str, question: &str, choices: &[&str], multi: bool) -> ThreadItem {
    ThreadItem::Ask {
        key: seq.to_string(),
        seq,
        id: id.into(),
        ask_kind: "clarify".into(),
        question: question.into(),
        detail: None,
        choices: choices.iter().map(|c| c.to_string()).collect(),
        multi,
    }
}
fn asked(seq: u64, id: &str, answer: Option<&str>) -> ThreadItem {
    ThreadItem::Asked {
        key: seq.to_string(),
        seq,
        id: id.into(),
        answer: answer.map(|a| a.to_string()),
    }
}
fn kinds(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| match r {
            Row::User { text, .. } => format!("user:{text}"),
            Row::Work {
                steps,
                live,
                plan,
                asks,
                ..
            } => {
                let mut s = format!(
                    "work:{}{}",
                    steps.iter().filter(|s| s.is_tool()).count(),
                    if *live { "+" } else { "" }
                );
                if plan.is_some() {
                    s += "·p";
                }
                if !asks.is_empty() {
                    s += &format!(
                        "·a{}q{}",
                        asks.len(),
                        asks.iter().filter(|a| a.answer.is_some()).count()
                    );
                }
                s
            }
            Row::Answer { text, .. } => format!("answer:{text}"),
            Row::Insert { id, .. } => format!("insert:{id}"),
            Row::Fail { text, .. } => format!("fail:{text}"),
            Row::Changes { files, .. } => format!("changes:{}", files.len()),
            Row::Writing { text, .. } => format!("writing:{text}"),
            Row::Queued { text, .. } => format!("queued:{text}"),
        })
        .collect()
}

#[test]
fn a_turn_is_message_work_answer() {
    let t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            narr(2, 0, "Looking."),
            tool(2, 1, "a"),
            result(3, "a"),
            tool(4, 0, "b"),
            result(5, "b"),
            text(6, 0, "Done."),
        ],
        ..Default::default()
    });
    let rows = t.rows();
    assert_eq!(kinds(&rows), ["user:go", "work:2", "answer:Done."]);
    let Row::Work {
        steps, from, to, ..
    } = &rows[1]
    else {
        panic!()
    };
    assert_eq!(
        steps[0],
        Step::Note("Looking.".into()),
        "text beside a tool call is a note on the way"
    );
    assert!(
        matches!(&steps[1], Step::Tool { output: Some(o), .. } if o == "ok"),
        "the result joins its call"
    );
    assert_eq!((*from, *to), (Some(2000.), Some(5000.)));
}

/// What a turn changed stands under its answer, its own row: the work and the answer before it
/// keep their places, and the next message starts the next turn after it.
#[test]
fn a_turns_changes_follow_its_answer() {
    let changes = ThreadItem::Changes {
        key: "7".into(),
        seq: 7,
        root: "/p".into(),
        base: "t0".into(),
        head: "t1".into(),
        files: vec![protocol::FileChange {
            path: "/p/a.rs".into(),
            add: 2,
            del: 1,
            binary: None,
        }],
    };
    let t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            tool(2, 0, "a"),
            result(3, "a"),
            text(6, 0, "Done."),
            changes,
            user(8, "more"),
        ],
        ..Default::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1", "answer:Done.", "changes:1", "user:more"]
    );
}

/// The plan lives in the work that wrote it: journal items, merged by id into one card that
/// ticks in place as newer snapshots come. No dates, no floating row, nothing to move.
/// The plan is the work's own `todo` calls: the card stands on the first of them and shows the
/// newest. The next piece of work keeps a card of its own; the earlier one stays as it ended.
#[test]
fn rule_1_the_plan_lives_in_the_work_that_wrote_it() {
    let steps = |t: &Thread, k: usize| match &t.rows()[k] {
        Row::Work {
            plan: Some((id, steps)),
            ..
        } => (
            id.clone(),
            steps.iter().map(|s| s.status.clone()).collect::<Vec<_>>(),
        ),
        r => panic!("no plan on {r:?}"),
    };
    let mut t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            tool(2, 0, "a"),
            plan(
                3,
                "c1",
                &[("1", "dig", "in_progress"), ("2", "haul", "pending")],
            ),
            tool(4, 0, "b"),
        ],
        running: true,
        queue: vec![queued(9, "later")],
        ..Default::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:2+·p", "queued:later"],
        "the plan is part of its work, not a row of its own"
    );
    // The next `todo` call of the same work ticks the same card, in place.
    t.apply(ThreadEvent::Append(vec![plan(
        5,
        "c2",
        &[("1", "dig", "completed"), ("2", "haul", "in_progress")],
    )]));
    assert_eq!(kinds(&t.rows()), ["user:go", "work:2+·p", "queued:later"]);
    assert_eq!(
        steps(&t, 1),
        ("c1".into(), vec!["completed".into(), "in_progress".into()]),
        "the card keeps its first call's id and shows the newest steps"
    );
    // A message taken up mid-run starts another piece of work. The first card stays as its
    // work left it; a `todo` call now is a card of the new work.
    t.apply(ThreadEvent::Append(vec![
        user(7, "and hurry"),
        tool(8, 0, "c"),
        plan(
            9,
            "c3",
            &[("1", "dig", "completed"), ("2", "haul", "completed")],
        ),
    ]));
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:go",
            "work:2·p",
            "user:and hurry",
            "work:1+·p",
            "queued:later"
        ]
    );
    assert_eq!(
        steps(&t, 1).1,
        ["completed", "in_progress"],
        "the first card did not move"
    );
    assert_eq!(steps(&t, 3).1, ["completed", "completed"]);
    // The run ends: the cards stay — they say what those turns worked through.
    t.apply(ThreadEvent::Run(false));
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:go",
            "work:2·p",
            "user:and hurry",
            "work:1·p",
            "queued:later"
        ]
    );
}

/// The agent's question is part of the work that asked it: the card waits there and the
/// answer settles it in place — on reopen the journal says the same thing.
#[test]
fn rule_1_the_question_lives_in_the_work_that_asked_it() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            tool(2, 0, "ask"),
            ask(3, "q1", "Which?", &["a", "b"], false),
        ],
        running: true,
        ..Default::default()
    });
    assert_eq!(kinds(&t.rows()), ["user:go", "work:1+·a1q0"]);
    let Row::Work { asks, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(asks[0].answer, None, "the agent still waits");
    // The answer settles the card where it stands; the work below it never moves.
    t.apply(ThreadEvent::Append(vec![
        asked(4, "q1", Some("a")),
        tool(5, 0, "b"),
    ]));
    assert_eq!(kinds(&t.rows()), ["user:go", "work:2+·a1q1"]);
    let Row::Work { asks, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(asks[0].answer.as_deref(), Some("a"));
    // The same thread reopened: the settled card is back, not a fresh question.
    let t2 = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            tool(2, 0, "ask"),
            ask(3, "q1", "Which?", &["a", "b"], false),
            asked(4, "q1", Some("a")),
            tool(5, 0, "b"),
        ],
        ..Default::default()
    });
    assert_eq!(kinds(&t2.rows()), ["user:go", "work:2·a1q1"]);
    // A declined question keeps its card too: the answer is the empty one.
    let t3 = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            tool(2, 0, "ask"),
            ask(3, "q1", "Which?", &[], false),
            asked(4, "q1", Some("")),
        ],
        ..Default::default()
    });
    assert_eq!(kinds(&t3.rows()), ["user:go", "work:1·a1q1"]);
}

fn process(id: &str, call: &str, state: ProcessState) -> Process {
    Process {
        id: id.into(),
        call_id: call.into(),
        command: "npm run dev".into(),
        state,
        ..Default::default()
    }
}

fn processes_of(row: &Row) -> Vec<(String, ProcessState)> {
    match row {
        Row::Work { processes, .. } => processes.iter().map(|p| (p.id.clone(), p.state)).collect(),
        _ => vec![],
    }
}

#[test]
fn rule_1_a_process_lives_at_the_call_that_started_it() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "serve it"),
            tool(2, 0, "p1"),
            text(3, 0, "Running."),
            user(4, "now the docs"),
            tool(5, 0, "x"),
        ],
        processes: vec![process("7", "p1", ProcessState::Running)],
        running: true,
        ..Default::default()
    });
    let rows = t.rows();
    let keys: Vec<String> = rows.iter().map(|r| r.key().to_string()).collect();
    assert_eq!(
        processes_of(&rows[1]),
        [("7".into(), ProcessState::Running)]
    );
    assert!(
        processes_of(&rows[4]).is_empty(),
        "later work does not take it"
    );
    assert!(t.processes_running());
    // It ends: the card changes where it stands, and nothing moves.
    t.apply(ThreadEvent::Process(process(
        "7",
        "p1",
        ProcessState::Restarted,
    )));
    let rows = t.rows();
    assert_eq!(
        rows.iter().map(|r| r.key().to_string()).collect::<Vec<_>>(),
        keys
    );
    assert_eq!(
        processes_of(&rows[1]),
        [("7".into(), ProcessState::Restarted)]
    );
    assert!(!t.processes_running());
}

#[test]
fn rule_1_journal_then_writing_then_queue() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), tool(2, 0, "a")],
        running: true,
        queue: vec![queued(9, "later")],
        ..Default::default()
    });
    t.apply(ThreadEvent::Stream(StreamKind::Text, "Hal".into()));
    t.apply(ThreadEvent::Stream(StreamKind::Text, "f".into()));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1+", "writing:Half", "queued:later"]
    );
    // The answer's entry (a message that called nothing) is the answer at once, in the seat
    // it was written in, though the run still lives; the work above it is done.
    t.apply(ThreadEvent::Append(vec![text(3, 0, "Half done.")]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1", "answer:Half done.", "queued:later"]
    );
    t.apply(ThreadEvent::Run(false));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1", "answer:Half done.", "queued:later"],
        "the work above an answer is done"
    );
}

#[test]
fn rule_1_items_out_of_order_find_their_place() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "a"), text(5, 0, "c")],
        ..Default::default()
    });
    t.apply(ThreadEvent::Append(vec![text(3, 0, "b"), text(3, 0, "b")]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:a", "answer:b", "answer:c"],
        "in key order, a repeat ignored"
    );
}

#[test]
fn rule_2_queue_in_the_agents_order() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        running: true,
        queue: vec![queued(10, "first"), queued(11, "second")],
        ..Default::default()
    });
    t.apply(ThreadEvent::Queue(vec![
        queued(11, "second"),
        queued(10, "first"),
    ]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:0+", "queued:second", "queued:first"],
        "send now: it shows first"
    );
}

#[test]
fn rule_3_a_taken_message_keeps_its_place() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), text(2, 0, "ok")],
        running: true,
        queue: vec![queued(10, "next"), queued(11, "after")],
        ..Default::default()
    });
    let before = t
        .rows()
        .iter()
        .position(|r| matches!(r, Row::Queued { text, .. } if text == "next"))
        .unwrap();
    // Pi places it: the journal entry arrives first (naming its submission), the queue's
    // event after.
    t.apply(ThreadEvent::Append(vec![placed(3, "next", 10, "r1")]));
    let rows = t.rows();
    assert_eq!(
        kinds(&rows),
        [
            "user:go",
            "answer:ok",
            "user:next",
            "work:0+",
            "queued:after"
        ],
        "never shown twice"
    );
    assert_eq!(
        rows.iter()
            .position(|r| matches!(r, Row::User { text, .. } if text == "next")),
        Some(before)
    );
    t.apply(ThreadEvent::Queue(vec![queued(11, "after")]));
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:go",
            "answer:ok",
            "user:next",
            "work:0+",
            "queued:after"
        ]
    );
}

#[test]
fn rule_4_work_counted_between_messages_not_per_run() {
    // One run took three messages from the queue: each answer has its own work.
    let t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "a"),
            tool(2, 0, "x"),
            result(3, "x"),
            text(4, 0, "A"),
            user(5, "b"),
            tool(6, 0, "y"),
            result(7, "y"),
            tool(8, 0, "z"),
            result(9, "z"),
            text(10, 0, "B"),
        ],
        ..Default::default()
    });
    let rows = t.rows();
    assert_eq!(
        kinds(&rows),
        [
            "user:a", "work:1", "answer:A", "user:b", "work:2", "answer:B"
        ]
    );
    let Row::Work { from, to, .. } = &rows[4] else {
        panic!()
    };
    assert_eq!(
        (*from, *to),
        (Some(6000.), Some(9000.)),
        "its time is its own steps'"
    );
}

#[test]
fn rule_4_a_second_message_of_the_agent_is_its_own() {
    let t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "a"),
            text(2, 0, "Answer."),
            tool(3, 0, "p"),
            result(4, "p"),
            text(5, 0, "Plan."),
        ],
        ..Default::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        ["user:a", "answer:Answer.", "work:1", "answer:Plan."]
    );
}

#[test]
fn rule_5_inserts_where_put_removed_ones_gone() {
    let ins = |seq: u64, id: &str| ThreadItem::Insert {
        key: seq.to_string(),
        seq,
        id: id.into(),
        insert_key: "tools".into(),
    };
    let mut t = Thread::open(ThreadOpen {
        items: vec![ins(1, "i0"), user(2, "a"), ins(3, "i1"), text(4, 0, "x")],
        ..Default::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        ["insert:i0", "user:a", "insert:i1", "answer:x"]
    );
    t.apply(ThreadEvent::Inserts(
        serde_json::from_value(json!({"i1": {"key": "tools", "removed": true}})).unwrap(),
    ));
    assert_eq!(
        t.insert_list().iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["i0"],
        "the list the app keeps for the inserts' kinds is the journal's, less the removed"
    );
    assert_eq!(kinds(&t.rows()), ["insert:i0", "user:a", "answer:x"]);
}

#[test]
fn a_run_that_ends_drops_what_was_being_written_and_is_no_longer_live() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), tool(2, 0, "a")],
        running: true,
        ..Default::default()
    });
    t.apply(ThreadEvent::Stream(StreamKind::Text, "par".into()));
    t.apply(ThreadEvent::Run(false));
    assert_eq!(kinds(&t.rows()), ["user:go", "work:1"]);
}

#[test]
fn catch_up_after_a_reconnect_adds_only_what_was_missed() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        head: 1,
        ..Default::default()
    });
    let v = t.version();
    t.catch_up(ThreadOpen {
        items: vec![user(1, "go"), text(2, 0, "ok")],
        head: 2,
        ..Default::default()
    });
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok"]);
    assert_eq!(t.head(), 2);
    assert!(t.version() > v);
}

#[test]
fn three_thousand_steps_are_cheap() {
    let mut items = vec![];
    for k in 0..1000u64 {
        let s = k * 4;
        items.extend([
            user(s + 1, "m"),
            tool(s + 2, 0, "c"),
            result(s + 3, "c"),
            text(s + 4, 0, "a"),
        ]);
    }
    let t = Thread::open(ThreadOpen {
        items,
        ..Default::default()
    });
    let started = std::time::Instant::now();
    let rows = t.rows();
    assert_eq!(rows.len(), 3000);
    assert!(
        started.elapsed().as_millis() < 50,
        "rows of 4000 items in {:?}",
        started.elapsed()
    );
}

#[test]
fn rule_4_a_sent_message_shows_at_once_and_is_never_doubled() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), text(2, 0, "ok")],
        ..Default::default()
    });
    t.sent("r1".into(), "next".into());
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok", "user:next"]);
    t.apply(ThreadEvent::Append(vec![placed(3, "next", 30, "r1")]));
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok", "user:next"]);
    t.sent("r2".into(), "lost".into());
    t.unsent("r2");
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok", "user:next"]);
}

#[test]
fn rule_4_the_same_words_twice_are_two_messages() {
    // Matched by request, never by words: "ok" sent twice is two rows until each is placed.
    let mut t = Thread::open(ThreadOpen::default());
    t.sent("r1".into(), "ok".into());
    t.sent("r2".into(), "ok".into());
    assert_eq!(kinds(&t.rows()), ["user:ok", "user:ok"]);
    t.apply(ThreadEvent::Append(vec![placed(1, "ok", 10, "r1")]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:ok", "user:ok"],
        "one placed, one still on its way"
    );
    t.apply(ThreadEvent::Append(vec![placed(2, "ok", 11, "r2")]));
    assert_eq!(kinds(&t.rows()), ["user:ok", "user:ok"]);
}

#[test]
fn rule_4_a_message_with_files_is_one_message_once_they_are_uploaded() {
    // Shown with the file where it was; the journal has it where it was put for the agent —
    // other words, the same request.
    let mut t = Thread::open(ThreadOpen::default());
    t.sent("r1".into(), "look\n[attachment:/Users/me/shot.png]".into());
    t.apply(ThreadEvent::Append(vec![placed(
        1,
        "look\n[attachment:/tmp/bench-attachments/1-shot.png]",
        10,
        "r1",
    )]));
    assert_eq!(t.rows().len(), 1, "{:?}", kinds(&t.rows()));
}

#[test]
fn rule_5_an_insert_put_while_a_message_waited_goes_after_that_message() {
    let ins = |seq: u64, id: &str| ThreadItem::Insert {
        key: seq.to_string(),
        seq,
        id: id.into(),
        insert_key: "tools".into(),
    };
    // Put while "b" was still in the composer: its entry is before "b", its place after it.
    let mut t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "a"),
            text(2, 0, "A"),
            ins(3, "i1"),
            user(4, "b"),
            text(5, 0, "B"),
            user(6, "c"),
        ],
        inserts: serde_json::from_value(json!({"i1": {"key": "tools", "after": 2}})).unwrap(),
        ..Default::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:a",
            "answer:A",
            "user:b",
            "answer:B",
            "insert:i1",
            "user:c"
        ]
    );
    // Until "b" is in the journal, it waits at the end.
    t = Thread::open(ThreadOpen {
        items: vec![user(1, "a"), ins(3, "i1")],
        inserts: serde_json::from_value(json!({"i1": {"key": "tools", "after": 2}})).unwrap(),
        ..Default::default()
    });
    assert_eq!(kinds(&t.rows()), ["user:a", "insert:i1"]);
}

#[test]
fn reasoning_streams_as_one_line_not_as_text() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        running: true,
        ..Default::default()
    });
    t.apply(ThreadEvent::Stream(
        StreamKind::Thought,
        "Let me check".into(),
    ));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:0+"],
        "thinking is a live line of work"
    );
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(
        steps,
        &[Step::Thought("Let me check".into())],
        "folded in it, there when opened"
    );
    t.apply(ThreadEvent::Append(vec![tool(2, 0, "a")]));
    t.apply(ThreadEvent::Stream(StreamKind::Thought, "more".into()));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1+"],
        "and stays that line while tools run"
    );
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(
        steps.last(),
        Some(&Step::Thought("more".into())),
        "the thinking now, after the steps"
    );
    t.apply(ThreadEvent::Stream(StreamKind::Text, "Here".into()));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1+", "writing:Here"],
        "only the answer is written out"
    );
}

#[test]
fn the_agent_s_marks_decide_without_guessing() {
    // Mid a run, the journal already says which words are the agent's narrative and which
    // are its answer — no flash of an answer that folds away, no note that waits to be the
    // last entry before it shows.
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), tool(2, 0, "a"), narr(2, 1, "Now the tests.")],
        running: true,
        ..Default::default()
    });
    assert_eq!(kinds(&t.rows()), ["user:go", "work:1+"]);
    t.apply(ThreadEvent::Append(vec![text(3, 0, "All green.")]));
    // The answer of a message that called nothing shows at once, though the run is live.
    // The closed line of work is done; the answer shows at once, though the run is live.
    assert_eq!(kinds(&t.rows()), ["user:go", "work:1", "answer:All green."]);
}

#[test]
fn words_reclassified_mid_stream_slide_into_the_work() {
    // Text streams as a possible answer; the model then begins a call: Pi's sign that the
    // words are narration. They leave the answer's seat and enter the work line as its
    // note — and the entry, when it commits, changes nothing: same place, same words.
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        running: true,
        ..Default::default()
    });
    t.apply(ThreadEvent::Stream(
        StreamKind::Text,
        "Reading the log".into(),
    ));
    assert_eq!(kinds(&t.rows()), ["user:go", "writing:Reading the log"]);
    t.apply(ThreadEvent::Stream(StreamKind::Narration, String::new()));
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!("the work line holds the words now")
    };
    assert_eq!(steps.last(), Some(&Step::Note("Reading the log".into())));
    assert!(
        !t.rows().iter().any(|r| matches!(r, Row::Writing { .. })),
        "no answer's seat any more"
    );
    t.apply(ThreadEvent::Append(vec![
        narr(2, 0, "Reading the log"),
        tool(2, 1, "a"),
    ]));
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(
        steps.first(),
        Some(&Step::Note("Reading the log".into())),
        "the committed note sits exactly where the stream left them"
    );
    assert_eq!(kinds(&t.rows()), ["user:go", "work:1+"]);
}

#[test]
fn words_after_a_begun_call_keep_streaming_into_the_work() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        running: true,
        ..Default::default()
    });
    t.apply(ThreadEvent::Stream(StreamKind::Narration, "".into()));
    t.apply(ThreadEvent::Stream(
        StreamKind::Narration,
        "and then ".into(),
    ));
    t.apply(ThreadEvent::Stream(StreamKind::Narration, "the fix".into()));
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(steps.last(), Some(&Step::Note("and then the fix".into())));
}

#[test]
fn words_before_a_call_are_notes_on_the_work_line() {
    // The model says what it's about to do, then calls: those words belong to the line of
    // work — its narrative — not to an answer row.
    let t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "fix it"),
            narr(2, 0, "Reading the test first."),
            tool(2, 1, "a"),
        ],
        ..ThreadOpen::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        ["user:fix it", "work:1"],
        "the turn is one line of work"
    );
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(
        steps.first(),
        Some(&Step::Note("Reading the test first.".into())),
        "the words are the line's narrative, standing before the call"
    );
}

#[test]
fn a_turn_that_only_answers_stays_an_answer() {
    let t = Thread::open(ThreadOpen {
        items: vec![user(1, "hi"), text(2, 0, "Hello!")],
        ..ThreadOpen::default()
    });
    assert_eq!(kinds(&t.rows()), ["user:hi", "answer:Hello!"]);
}

#[test]
fn words_appended_mid_run_join_their_turn_as_a_note() {
    // The journal commits the model's words as its own entry between the calls: they belong
    // to the running turn — the line's narrative — never a half-answer.
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "fix it")],
        ..ThreadOpen::default()
    });
    t.apply(ThreadEvent::Run(true));
    t.apply(ThreadEvent::Append(vec![thought(2, 0, "hm")]));
    t.apply(ThreadEvent::Append(vec![tool(2, 1, "a")]));
    t.apply(ThreadEvent::Append(vec![narr(
        2,
        9,
        "Reading the test first.",
    )]));
    let Row::Work { steps, .. } = &t.rows()[1] else {
        panic!()
    };
    assert!(
        steps
            .iter()
            .any(|s| matches!(s, Step::Note(t) if t == "Reading the test first.")),
        "the words ride the work line: {steps:?}"
    );
}

#[test]
fn pressed_queue_shows_at_once_and_never_twice() {
    // Enter while the run lives: the row waits under the work at once (no id yet), and the
    // queue's answer — or the agent taking the message up — folds the echo into one row.
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        ..ThreadOpen::default()
    });
    t.apply(ThreadEvent::Run(true));
    t.queued_now("r1".into(), "later".into());
    t.queued_now("r2".into(), "later".into());
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:0+", "queued:later", "queued:later"],
        "the same words twice are two messages"
    );
    // The agent's queue answers with the first: it takes that row's place, by its request.
    t.apply(ThreadEvent::Queue(vec![queued_req(9, "later", "r1")]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:0+", "queued:later", "queued:later"]
    );
    let Row::Queued { sub, .. } = &t.rows()[2] else {
        panic!()
    };
    assert_eq!(sub, "9", "the queue's own row takes over");
    // The agent takes it up mid-run: it joins the journal and leaves the queue in one step,
    // by its submission; the second, still on its way, stays.
    t.apply(ThreadEvent::Append(vec![placed(2, "later", 9, "r1")]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "user:later", "work:0+", "queued:later"]
    );
}

#[test]
fn pressed_queue_clears_with_the_rest_when_the_send_fails() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        ..ThreadOpen::default()
    });
    t.apply(ThreadEvent::Run(true));
    t.queued_now("r1".into(), "gone".into());
    assert_eq!(kinds(&t.rows()), ["user:go", "work:0+", "queued:gone"]);
    t.clear_sent();
    assert_eq!(kinds(&t.rows()), ["user:go", "work:0+"]);
}

#[test]
fn a_held_run_stands_still_and_goes_on_from_there() {
    use protocol::Hold;
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), tool(2, 0, "c1")],
        ..ThreadOpen::default()
    });
    t.apply(ThreadEvent::Run(true));
    assert!(!t.paused());
    // Held at 5 s: the work's clock stops there however long the hold lasts.
    t.apply(ThreadEvent::Holds(vec![Hold {
        from: 5000.,
        to: None,
    }]));
    assert!(t.paused());
    let work = t
        .rows()
        .into_iter()
        .find(|r| matches!(r, Row::Work { .. }))
        .unwrap();
    assert!(work.held());
    let Row::Work { from, holds, .. } = &work else {
        panic!()
    };
    let from = from.unwrap();
    assert_eq!(crate::worked_ms(from, 9000., holds), 3000.);
    assert_eq!(crate::worked_ms(from, 60000., holds), 3000.);
    // Let go at 20 s: it counts on from where it stood.
    t.apply(ThreadEvent::Holds(vec![Hold {
        from: 5000.,
        to: Some(20000.),
    }]));
    assert!(!t.paused());
    let Some(Row::Work { holds, .. }) =
        t.rows().into_iter().find(|r| matches!(r, Row::Work { .. }))
    else {
        panic!()
    };
    assert_eq!(crate::worked_ms(from, 22000., &holds), 5000.);
    // The run's end lets every hold go: finished work keeps no holds.
    t.apply(ThreadEvent::Run(false));
    assert!(!t.paused());
    assert!(t.rows().iter().all(|r| !r.held()));
}
