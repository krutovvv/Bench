use protocol::{PlanStep, QueuedItem, StreamKind, ThreadEvent, ThreadItem, ThreadOpen};
use serde_json::json;

use crate::{Row, Step, Thread};

fn user(seq: u64, text: &str) -> ThreadItem {
    ThreadItem::User {
        key: format!("{seq}.0"),
        seq,
        text: text.into(),
        at: Some(seq as f64 * 1000.),
    }
}
fn text(seq: u64, sub: u32, t: &str) -> ThreadItem {
    ThreadItem::Text {
        key: format!("{seq}.{sub}"),
        seq,
        text: t.into(),
        at: Some(seq as f64 * 1000.),
        error: None,
        narration: None,
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
        mode: "followUp".into(),
        text: t.into(),
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
                if !plan.is_empty() {
                    s += &format!("·p{}", plan.len());
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
            text(2, 0, "Looking."),
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

/// The plan lives in the work that wrote it: journal items, merged by id into one card that
/// ticks in place as newer snapshots come. No dates, no floating row, nothing to move.
#[test]
fn rule_1_the_plan_lives_in_the_work_that_wrote_it() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![
            user(1, "go"),
            tool(2, 0, "a"),
            plan(3, "p", &[("1", "dig", "in_progress")]),
            tool(4, 0, "b"),
        ],
        running: true,
        queue: vec![queued(9, "later")],
        ..Default::default()
    });
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:2+·p1", "queued:later"],
        "the plan is part of its work, not a row of its own"
    );
    // Ticking: a newer snapshot of the same plan stamps the same card, in place.
    t.apply(ThreadEvent::Append(vec![plan(
        5,
        "p",
        &[("1", "dig", "completed"), ("2", "haul", "in_progress")],
    )]));
    assert_eq!(kinds(&t.rows()), ["user:go", "work:2+·p1", "queued:later"]);
    let Row::Work { plan: cards, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(cards[0].1.len(), 2, "the card shows the newest snapshot");
    // A message taken up mid-run splits the work: the plan keeps living on the card it was
    // born on, ticking there — it does not repeat below.
    t.apply(ThreadEvent::Append(vec![
        user(7, "and hurry"),
        tool(8, 0, "c"),
        plan(
            9,
            "p",
            &[("1", "dig", "completed"), ("2", "haul", "completed")],
        ),
    ]));
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:go",
            "work:2·p1",
            "user:and hurry",
            "work:1+",
            "queued:later"
        ],
        "one plan, one card, where it was born"
    );
    let Row::Work { plan: cards, .. } = &t.rows()[1] else {
        panic!()
    };
    assert_eq!(cards[0].1[1].status, "completed");
    // Another plan, sharing no steps: a card of its own, on the work going on now.
    t.apply(ThreadEvent::Append(vec![plan(
        10,
        "q",
        &[("1", "another", "pending")],
    )]));
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:go",
            "work:2·p1",
            "user:and hurry",
            "work:1+·p1",
            "queued:later"
        ]
    );
    // The run ends: the cards stay — they say what those turns worked through.
    t.apply(ThreadEvent::Run(false));
    assert_eq!(
        kinds(&t.rows()),
        [
            "user:go",
            "work:2·p1",
            "user:and hurry",
            "work:1·p1",
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
    // The answer's entry, while the run is still live, rides the work line: another call
    // may follow it, and a half-answer that later turns into notes must not flash as one.
    t.apply(ThreadEvent::Append(vec![text(3, 0, "Half done.")]));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1+", "queued:later"],
        "the words are on the way"
    );
    // The run ends: those same words are the answer, and the work above it is done.
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
    // Pi places it: the journal entry arrives first, the queue's event after.
    t.apply(ThreadEvent::Append(vec![user(3, "next")]));
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
        insert_key: "tasks".into(),
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
        json!({"i1": {"key": "tasks", "removed": true}})
            .as_object()
            .unwrap()
            .clone(),
    ));
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
fn rule_6_a_sent_message_shows_at_once_and_is_never_doubled() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go"), text(2, 0, "ok")],
        ..Default::default()
    });
    t.sent("next".into());
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok", "user:next"]);
    t.apply(ThreadEvent::Append(vec![user(3, "next")]));
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok", "user:next"]);
    t.sent("lost".into());
    t.unsent("lost");
    assert_eq!(kinds(&t.rows()), ["user:go", "answer:ok", "user:next"]);
}

#[test]
fn rule_6_a_message_with_files_is_one_message_once_they_are_uploaded() {
    // Shown with the file where it was; the journal has it where it was put for the agent.
    let mut t = Thread::open(ThreadOpen::default());
    t.sent("look\n[attachment:/Users/me/shot.png]".into());
    t.resent(
        "look\n[attachment:/Users/me/shot.png]",
        "look\n[attachment:/tmp/bench-attachments/1-shot.png]".into(),
    );
    t.apply(ThreadEvent::Append(vec![user(
        1,
        "look\n[attachment:/tmp/bench-attachments/1-shot.png]",
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
        inserts: json!({"i1": {"key": "tools", "after": 2}})
            .as_object()
            .unwrap()
            .clone(),
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
        inserts: json!({"i1": {"key": "tools", "after": 2}})
            .as_object()
            .unwrap()
            .clone(),
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
    let mut marked: ThreadItem = text(3, 0, "All green.");
    if let ThreadItem::Text { narration, .. } = &mut marked {
        *narration = Some(false);
    }
    t.apply(ThreadEvent::Append(vec![marked]));
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
            text(2, 0, "Reading the test first."),
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
    t.apply(ThreadEvent::Append(vec![text(
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
    t.queued_now("later".into());
    assert_eq!(kinds(&t.rows()), ["user:go", "work:0+", "queued:later"]);
    // The agent's queue answers with the same words: one row, now with its id.
    t.apply(ThreadEvent::Queue(vec![queued(9, "later")]));
    assert_eq!(kinds(&t.rows()), ["user:go", "work:0+", "queued:later"]);
    let Row::Queued { sub, .. } = &t.rows()[2] else {
        panic!()
    };
    assert_eq!(sub, "9", "the queue's own row takes over");
    // The agent takes it up mid-run: it joins the journal, the queue empties, no ghost stays.
    t.apply(ThreadEvent::Append(vec![user(2, "later")]));
    t.apply(ThreadEvent::Queue(vec![]));
    assert_eq!(kinds(&t.rows()), ["user:go", "user:later", "work:0+"]);
}

#[test]
fn pressed_queue_clears_with_the_rest_when_the_send_fails() {
    let mut t = Thread::open(ThreadOpen {
        items: vec![user(1, "go")],
        ..ThreadOpen::default()
    });
    t.apply(ThreadEvent::Run(true));
    t.queued_now("gone".into());
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
