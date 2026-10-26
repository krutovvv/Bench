//! What the block says about a project's MCP servers: the state of one server, the line about it,
//! and the line about the set. Pure, and tested below, because the whole point is that a server
//! nobody has asked yet must never be drawn as one that works.
//!
//! A file of its own rather than a few functions at the foot of `project_env.rs`: with the tests
//! there, the `#[test]` expansion on top of that file's deep GPUI builder chains went past the
//! compiler's recursion limit while the same code built fine without tests. Here the logic sits
//! with its own tests and the big state file stays about state.

use super::project_env::McpInfo;
/// What the block knows of one MCP server, in the order the facts deserve telling: an off switch
/// and a refused handshake come before anything else, and a server that nothing has asked for
/// anything is not a server that works — hiding that behind "connects with the project's first
/// thread" is what made the list unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTone {
    /// Turned off: its tools go to nobody.
    Off,
    /// Tried, and it refused; the note carries why.
    Error,
    /// Tried, and it asked to sign in (OAuth) first.
    Auth,
    /// Tried, answers, and hands over its tools.
    Live,
    /// Tried, but nothing came of it and it gave no reason.
    Dead,
    /// Nobody has asked it for anything yet: its state is unknown, which is not "fine".
    Untested,
}

pub fn mcp_tone(m: &McpInfo) -> McpTone {
    if !m.enabled {
        McpTone::Off
    } else if m.error.is_some() {
        McpTone::Error
    } else if m.needs_auth {
        McpTone::Auth
    } else if m.connected {
        McpTone::Live
    } else if m.checked {
        McpTone::Dead
    } else {
        McpTone::Untested
    }
}

/// A row's line: what the server is, and what came of asking it.
pub fn mcp_note(m: &McpInfo) -> String {
    let kind = if m.kind.is_empty() { "server" } else { &m.kind };
    match mcp_tone(m) {
        McpTone::Off => format!("{kind} · off, its tools are not given"),
        McpTone::Error => format!("{kind} · {}", m.error.clone().unwrap_or_default()),
        McpTone::Auth => format!("{kind} · it asks you to sign in first"),
        McpTone::Live if m.tools > 0 => format!("{kind} · answers, {} tools", m.tools),
        McpTone::Live => format!("{kind} · answers, but gives no tools"),
        McpTone::Dead => format!("{kind} · gave nothing the last time it was asked"),
        McpTone::Untested => format!("{kind} · not tried yet — Check asks it now"),
    }
}

/// How a project's MCP servers stand as a set, for the block's head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSet {
    /// A check is in flight.
    Checking,
    /// At least one refused or wants signing in.
    Trouble,
    /// None is broken, but at least one has never been asked.
    Untested,
    /// Every one is turned off.
    AllOff,
    /// Every one that is on answers.
    Answering,
}

/// The head's line and the toast after a check: the same counts, said once.
pub fn mcp_tally(mcp: &[McpInfo], checking: bool) -> (McpSet, String) {
    let (mut live, mut trouble, mut untested, mut off) = (0, 0, 0, 0);
    for m in mcp {
        match mcp_tone(m) {
            McpTone::Live => live += 1,
            McpTone::Error | McpTone::Auth | McpTone::Dead => trouble += 1,
            McpTone::Untested => untested += 1,
            McpTone::Off => off += 1,
        }
    }
    let state = if checking {
        McpSet::Checking
    } else if trouble > 0 {
        McpSet::Trouble
    } else if untested > 0 {
        McpSet::Untested
    } else if live == 0 && off > 0 {
        McpSet::AllOff
    } else {
        McpSet::Answering
    };
    let mut parts: Vec<String> = Vec::new();
    if live > 0 {
        parts.push(format!("{live} answer"));
    }
    if trouble > 0 {
        parts.push(format!("{trouble} with trouble"));
    }
    if untested > 0 {
        parts.push(format!("{untested} not tried"));
    }
    if off > 0 {
        parts.push(format!("{off} off"));
    }
    let text = if checking {
        "asking…".to_string()
    } else if parts.is_empty() {
        "nothing to ask".to_string()
    } else {
        parts.join(", ")
    };
    (state, text)
}

#[cfg(test)]
mod mcp_status_tests {
    use super::*;

    fn m(enabled: bool, checked: bool, connected: bool, tools: u64, error: Option<&str>, needs_auth: bool) -> McpInfo {
        McpInfo {
            name: "s".into(),
            kind: "http".into(),
            enabled,
            checked,
            connected,
            tools,
            error: error.map(str::to_string),
            needs_auth,
            spec: "{}".into(),
        }
    }

    #[test]
    fn a_server_nobody_asked_is_never_shown_as_working() {
        // The old bug in one line: no entry in the agent's map looked exactly like a live server.
        let never_asked = m(true, false, false, 0, None, false);
        assert_eq!(mcp_tone(&never_asked), McpTone::Untested);
        assert!(mcp_note(&never_asked).contains("not tried yet"));
        // Asked, handshake live: the honest good news, with what it brought.
        let live = m(true, true, true, 12, None, false);
        assert_eq!(mcp_tone(&live), McpTone::Live);
        assert_eq!(mcp_note(&live), "http · answers, 12 tools");
        // Asked, alive, no tools of its own: said as such, not as an error.
        assert!(mcp_note(&m(true, true, true, 0, None, false)).contains("no tools"));
    }

    #[test]
    fn the_worse_fact_wins() {
        // Off beats everything: its tools go to nobody, whatever the last handshake said.
        assert_eq!(mcp_tone(&m(false, true, true, 3, Some("boom"), false)), McpTone::Off);
        // A refusal beats a live client: a server that died mid-session must not show as fine.
        assert_eq!(mcp_tone(&m(true, true, true, 3, Some("connect timed out"), false)), McpTone::Error);
        assert!(mcp_note(&m(true, true, true, 3, Some("connect timed out"), false)).contains("connect timed out"));
        // Signing in outranks both, since it is the one thing the person can do.
        assert_eq!(mcp_tone(&m(true, true, false, 0, None, true)), McpTone::Auth);
        // Tried, silent, no stated reason.
        assert_eq!(mcp_tone(&m(true, true, false, 0, None, false)), McpTone::Dead);
    }

    #[test]
    fn the_head_line_names_trouble_before_calm() {
        let bad = m(true, true, false, 0, Some("401"), false);
        let good = m(true, true, true, 4, None, false);
        let new = m(true, false, false, 0, None, false);
        let off = m(false, false, false, 0, None, false);
        // A broken server is what the head must say first, even among healthy ones.
        assert_eq!(mcp_tally(&[good.clone(), bad.clone()], false).0, McpSet::Trouble);
        // Nothing broken, something untried: not "all answer".
        assert_eq!(mcp_tally(&[good.clone(), new.clone()], false).0, McpSet::Untested);
        // All healthy, or all deliberately off, are the two calm states.
        assert_eq!(mcp_tally(&[good.clone(), good.clone()], false).0, McpSet::Answering);
        assert_eq!(mcp_tally(&[off.clone(), off.clone()], false).0, McpSet::AllOff);
        // While the agent is shaking hands, the head says so rather than quoting stale facts.
        assert_eq!(mcp_tally(&[new.clone()], true), (McpSet::Checking, "asking…".to_string()));
        // The counts read as one plain line.
        assert_eq!(
            mcp_tally(&[good.clone(), bad.clone(), new.clone(), off.clone()], false).1,
            "1 answer, 1 with trouble, 1 not tried, 1 off"
        );
        // An empty set is nothing to ask, not an error.
        assert_eq!(mcp_tally(&[], false).1, "nothing to ask");
    }
}
