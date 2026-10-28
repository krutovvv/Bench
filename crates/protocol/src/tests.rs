//! The wire read as the agent writes it, and the agent's copy of the types kept current.

use super::*;
use serde_json::json;

#[test]
fn reads_thread_open() {
    let open: ThreadOpen = serde_json::from_value(json!({
        "items": [
            {"key": "5.0", "seq": 5, "kind": "user", "text": "fix it", "at": 1005},
            {"key": "9.2", "seq": 9, "kind": "tool", "call_id": "c1", "name": "bash", "args": {"command": "ls"}},
            {"key": "10.0", "seq": 10, "kind": "result", "call_id": "c1", "name": "bash", "output": "a", "failed": false},
            {"key": "11", "seq": 11, "kind": "insert", "id": "i1", "insert_key": "tools"},
            {"key": "12", "seq": 12, "kind": "fail", "text": "no model"}
        ],
        "queue": [{"key": "q31", "sub": "31", "mode": "followUp", "text": "next"}],
        "inserts": {"i1": {"key": "tools", "state": {"tool": "json"}, "folded": false, "removed": false}},
        "running": true,
        "asks": [{"id": "c7", "kind": "approve", "command": "rm -rf build", "choices": ["once", "deny"]}],
        "info": {"effort": "high"},
        "head": 12
    }))
    .unwrap();
    assert_eq!(open.items.len(), 5);
    assert_eq!(open.items[1].order(), (9, 2));
    assert_eq!(open.items[3].order(), (11, 0));
    assert_eq!(open.queue[0].text, "next");
    assert_eq!(open.inserts["i1"].state["tool"], "json");
    assert_eq!(open.asks[0].kind, AskKind::Approve);
    assert_eq!(open.info.effort, "high");
    assert!(open.running);
}

#[test]
fn reads_events() {
    let ev = |v| serde_json::from_value::<Event>(v).unwrap();
    assert_eq!(
        ev(json!({"type": "thread", "thread": "t1", "event": {"kind": "stream", "value": ["text", "Hel"]}})),
        Event::Thread { thread: "t1".into(), event: ThreadEvent::Stream(StreamKind::Text, "Hel".into()) }
    );
    assert_eq!(
        ev(json!({"type": "thread", "thread": "t1", "event": {"kind": "holds", "value": [{"from": 5.0}]}})),
        Event::Thread { thread: "t1".into(), event: ThreadEvent::Holds(vec![Hold { from: 5., to: None }]) }
    );
    assert_eq!(ev(json!({"type": "threads_changed"})), Event::ThreadsChanged);
    assert!(serde_json::from_value::<Event>(json!({"type": "message.delta"})).is_err());
}

#[test]
fn a_request_names_its_method_and_answer() {
    assert_eq!(CreateThread::METHOD, "thread.create");
    let made: <CreateThread as Request>::Response = serde_json::from_value(json!({"thread": "t9"})).unwrap();
    assert_eq!(made.thread.as_str(), "t9");
    // A command's answer: printed text, or the message it became.
    let r: CommandResult = serde_json::from_value(json!({"kind": "send", "text": "do it"})).unwrap();
    assert_eq!(r, CommandResult::Send("do it".into()));
    // Optional params may be left out.
    let p: SendMessage = serde_json::from_value(json!({"thread": "t", "text": "hi"})).unwrap();
    assert!(!p.queue && p.request.is_none());
}

/// The agent's copy of the types is current; when it is not, it is written again and the test
/// fails once, so the new file is seen and committed.
#[test]
fn agent_types_are_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../agent/src/protocol.gen.ts");
    let want = super::typescript();
    let have = std::fs::read_to_string(path).unwrap_or_default();
    if have != want {
        std::fs::write(path, &want).unwrap();
        panic!("agent/src/protocol.gen.ts was out of date: written again, commit it");
    }
}
