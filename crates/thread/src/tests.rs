use protocol::{QueuedItem, StreamKind, ThreadEvent, ThreadItem, ThreadOpen};
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
    }
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
fn queued(sub: u64, t: &str) -> QueuedItem {
    QueuedItem {
        key: format!("q{sub}"),
        sub: sub.to_string(),
        mode: "followUp".into(),
        text: t.into(),
    }
}
fn kinds(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| match r {
            Row::User { text, .. } => format!("user:{text}"),
            Row::Work { steps, live, .. } => format!(
                "work:{}{}",
                steps.iter().filter(|s| s.is_tool()).count(),
                if *live { "+" } else { "" }
            ),
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
    // The answer's entry replaces what was being written, and still stands before the queue.
    t.apply(ThreadEvent::Append(vec![text(3, 0, "Half done.")]));
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
fn rule_5_an_insert_put_while_a_message_waited_goes_after_that_message() {
    let ins = |seq: u64, id: &str| ThreadItem::Insert {
        key: seq.to_string(),
        seq,
        id: id.into(),
        insert_key: "files".into(),
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
        inserts: json!({"i1": {"key": "files", "after": 2}})
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
        inserts: json!({"i1": {"key": "files", "after": 2}})
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
    t.apply(ThreadEvent::Append(vec![tool(2, 0, "a")]));
    t.apply(ThreadEvent::Stream(StreamKind::Thought, "more".into()));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1+"],
        "and stays that line while tools run"
    );
    t.apply(ThreadEvent::Stream(StreamKind::Text, "Here".into()));
    assert_eq!(
        kinds(&t.rows()),
        ["user:go", "work:1+", "writing:Here"],
        "only the answer is written out"
    );
}
