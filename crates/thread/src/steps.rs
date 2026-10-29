//! A piece of work's steps as the opened work shows them: in order, calls of one kind in a row
//! as one line. Only what the journal already holds: the calls, their results, the
//! reasoning before them, the evaluator's words in a result (`[Evaluator…: …]`).
//!
//! The rules (each is a test below):
//! 1. The work reads in order as runs (`runs`): calls of one kind in a row are one line, a single
//!    call of another kind between two of them stays inside it, the agent's words between calls
//!    are a line of their own, and reasoning goes with the call it led to (`reasoning_before`).
//! 2. A kind of call (commands, reads…) is its tool's; errors and the evaluator cut across them.
//! 3. A run says what its calls acted on (`parts`): a command by its program, a read or an edit
//!    by its file, any other call by its tool — read from the call's own arguments.

use crate::Step;

/// A kind of call, and the two marks that cut across kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Filter {
    Commands,
    Reads,
    Edits,
    Searches,
    Browser,
    Tools,
    Errors,
    Evaluator,
}

impl Filter {
    pub fn label(self) -> &'static str {
        match self {
            Filter::Commands => "Commands",
            Filter::Reads => "Reads",
            Filter::Edits => "Edits",
            Filter::Searches => "Searches",
            Filter::Browser => "Browser",
            Filter::Tools => "Other tools",
            Filter::Errors => "Errors",
            Filter::Evaluator => "Evaluator",
        }
    }

    /// The kind of call a tool's name makes.
    pub fn of_tool(name: &str) -> Filter {
        let n = name.rsplit("__").next().unwrap_or(name);
        match n {
            "bash" | "ssh_run" | "process" | "terminal" | "execute_code" => Filter::Commands,
            "read" | "ls" | "skill_view" => Filter::Reads,
            "edit" | "write" | "multi_edit" | "skill_manage" => Filter::Edits,
            "grep" | "glob" | "lsp" | "session_search" | "tool_search" => Filter::Searches,
            _ if name.starts_with("mcp__browser") || n.starts_with("browser") => Filter::Browser,
            _ if n.contains("search") => Filter::Searches,
            _ => Filter::Tools,
        }
    }

    /// Whether `step` is of this kind.
    pub fn matches(self, step: &Step) -> bool {
        match (self, step) {
            (Filter::Errors, Step::Tool { failed, .. }) => *failed,
            (Filter::Evaluator, Step::Tool { output, .. }) => {
                output.as_deref().and_then(evaluator_word).is_some()
            }
            (f, Step::Tool { name, .. }) => {
                !matches!(f, Filter::Errors | Filter::Evaluator) && Filter::of_tool(name) == f
            }
            _ => false,
        }
    }
}

/// The reasoning right before the call at `k`: the thoughts between it and the step before them.
pub fn reasoning_before(steps: &[Step], k: usize) -> Vec<&str> {
    let mut out: Vec<&str> = steps[..k]
        .iter()
        .rev()
        .map_while(|s| match s {
            Step::Thought(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    out.reverse();
    out
}

/// What the evaluator said in a result, without its bracket: the agent appends its words to a
/// call's output as `[Evaluator: …]` (`agent/src/judge/`), and a refused call's output is its
/// words alone.
pub fn evaluator_word(output: &str) -> Option<&str> {
    let start = output.find("[Evaluator")?;
    let rest = &output[start..];
    let body = &rest[rest.find(':')? + 1..];
    let end = body.find(']').unwrap_or(body.len());
    Some(body[..end].trim()).filter(|w| !w.is_empty())
}

/// What a call acts on, to part a run by: a command's program, a read's or an edit's file, a
/// search's or another tool's name. Reasoning and the agent's words have none.
pub fn facet(step: &Step) -> Option<String> {
    let Step::Tool { name, args, .. } = step else {
        return None;
    };
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("");
    match Filter::of_tool(name) {
        Filter::Commands => program(arg("command")).or_else(|| Some(tool_name(name))),
        Filter::Reads | Filter::Edits if !arg("path").is_empty() => Some(base(arg("path"))),
        _ => Some(tool_name(name)),
    }
}

/// The program a shell command runs, as its syntax says: the first command of its first line
/// that does something — past `cd`, `export`, `echo`, variable assignments and the wrappers
/// (`time`, `sudo`, `env`, `nohup`, `timeout N`) — by its file's name (`./run.sh` is `run.sh`,
/// `$TD` is `TD`). Quotes are respected: a `;` inside `'…'` separates nothing.
pub fn program(command: &str) -> Option<String> {
    command_from(command).map(|(p, _)| p)
}

/// The program (`program`) and the first line from it on: what the call runs, without the
/// `cd …;` and the assignments before it.
pub fn command_from(command: &str) -> Option<(String, &str)> {
    const PASS: &[&str] = &[
        "cd", "export", "set", "source", ".", "echo", "printf", "true", "unset", "if", "then",
        "do", "for", "while", "!", "{", "(",
    ];
    const WRAP: &[&str] = &["time", "sudo", "env", "nohup", "exec", "command", "xargs"];
    let line = command.lines().find(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#')
    })?;
    let mut skip_next = false;
    let mut segment_passed = false;
    for (start, word) in words(line) {
        if word == ";" || word == "&" || word == "|" {
            segment_passed = false;
            continue;
        }
        if segment_passed {
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        let w = word.trim_matches(|c| c == '"' || c == '\'' || c == '(' || c == ')');
        if w.is_empty() || (w.contains('=') && !w.starts_with('=') && !w.starts_with('-')) {
            continue;
        }
        if w == "timeout" {
            skip_next = true;
            continue;
        }
        if WRAP.contains(&w) || w.starts_with('-') {
            continue;
        }
        if PASS.contains(&w) {
            segment_passed = true;
            continue;
        }
        let name = base(w.trim_start_matches('$').trim_matches(['{', '}']));
        return (!name.is_empty()).then(|| (name, &line[start..]));
    }
    None
}

/// A shell line's words with where each starts, quotes kept whole; `;`, `&` and `|` outside
/// quotes are words of their own.
fn words(line: &str) -> Vec<(usize, &str)> {
    let mut out = vec![];
    let mut start: Option<usize> = None;
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => {
                quote = Some(c);
                start.get_or_insert(i);
            }
            (None, ';' | '&' | '|') => {
                if let Some(s) = start.take() {
                    out.push((s, &line[s..i]));
                }
                out.push((i, &line[i..i + 1]));
            }
            (None, c) if c.is_whitespace() => {
                if let Some(s) = start.take() {
                    out.push((s, &line[s..i]));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start {
        out.push((s, &line[s..]));
    }
    out
}

fn base(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

/// A tool's own name: an MCP server's tool without the server (`mcp__ozon__ozon_search` is
/// `ozon_search`).
fn tool_name(name: &str) -> String {
    name.rsplit("__").next().unwrap_or(name).to_string()
}

/// A line of the opened work, in order: calls of one kind in a row, or the agent's words.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// Calls, by their index in the work's steps; `kind` is most of them.
    Run { kind: Filter, calls: Vec<usize> },
    /// The agent's words between calls (a `Note`), or reasoning no call followed.
    Step(usize),
}

impl Line {
    /// The line's first step: its key in the view.
    pub fn first(&self) -> usize {
        match self {
            Line::Run { calls, .. } => calls[0],
            Line::Step(k) => *k,
        }
    }
}

/// The opened work as lines in order (rule 5).
pub fn runs(steps: &[Step]) -> Vec<Line> {
    let mut lines: Vec<Line> = vec![];
    for (k, s) in steps.iter().enumerate() {
        match s {
            Step::Tool { name, .. } => {
                let kind = Filter::of_tool(name);
                match lines.last_mut() {
                    Some(Line::Run { kind: last, calls }) if *last == kind => calls.push(k),
                    _ => lines.push(Line::Run { kind, calls: vec![k] }),
                }
            }
            // Reasoning waits for its call; at the end, with none after it, it is a line.
            Step::Thought(_) => {}
            Step::Note(_) => lines.push(Line::Step(k)),
        }
    }
    if let Some(k) = steps.iter().rposition(|s| !matches!(s, Step::Thought(_))).map_or(
        (!steps.is_empty()).then_some(0),
        |last| (last + 1 < steps.len()).then_some(last + 1),
    ) {
        lines.push(Line::Step(k));
    }
    // A single call between two runs of one kind stays inside them.
    let mut i = 1;
    while i + 1 < lines.len() {
        let joins = matches!(
            (&lines[i - 1], &lines[i], &lines[i + 1]),
            (Line::Run { kind: a, .. }, Line::Run { calls, .. }, Line::Run { kind: b, .. })
                if a == b && calls.len() == 1
        );
        if joins {
            let Line::Run { calls: mid, .. } = lines.remove(i) else { unreachable!() };
            let Line::Run { calls: next, .. } = lines.remove(i) else { unreachable!() };
            if let Line::Run { calls, .. } = &mut lines[i - 1] {
                calls.extend(mid);
                calls.extend(next);
            }
        } else {
            i += 1;
        }
    }
    lines
}

/// What the calls of a run acted on, the most first (`facet`): only the calls of its kind, not
/// a single other one it took in.
pub fn parts(steps: &[Step], kind: Filter, calls: &[usize]) -> Vec<(String, usize)> {
    let mut count: Vec<(String, usize)> = vec![];
    for f in calls
        .iter()
        .filter(|k| kind.matches(&steps[**k]))
        .filter_map(|k| facet(&steps[*k]))
    {
        match count.iter_mut().find(|(n, _)| *n == f) {
            Some((_, c)) => *c += 1,
            None => count.push((f, 1)),
        }
    }
    count.sort_by(|a, b| b.1.cmp(&a.1));
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(name: &str, failed: bool, output: &str) -> Step {
        Step::Tool {
            call_id: String::new(),
            name: name.into(),
            args: json!({}),
            preview: String::new(),
            output: Some(output.into()),
            failed,
            diff: None,
        }
    }

    #[test]
    fn rule_2_errors_and_the_evaluator_cut_across_kinds() {
        let failed = call("bash", true, "No such file");
        let judged = call(
            "grep",
            false,
            "[Evaluator: the likeliest places for the request first]\nsrc/a.rs:1",
        );
        assert!(Filter::Commands.matches(&failed));
        assert!(Filter::Errors.matches(&failed));
        assert!(!Filter::Errors.matches(&judged));
        assert!(Filter::Searches.matches(&judged));
        assert!(Filter::Evaluator.matches(&judged));
        assert_eq!(
            evaluator_word("Tool call blocked: [Evaluator, for this step: a repeat. Not run.]"),
            Some("a repeat. Not run.")
        );
        assert_eq!(evaluator_word("plain output"), None);
    }

    #[test]
    fn rule_3_a_run_parts_by_program_or_file() {
        for (cmd, prog) in [
            ("cd /tmp && python3 - <<'EOF'\nprint(1)\nEOF", "python3"),
            ("TD=~/.things/bin/things-do; $TD add 'x'", "TD"),
            ("time cargo build 2>&1 | tail -3", "cargo"),
            ("echo \"=== a ===\"; ls -la ~/bin", "ls"),
            ("cd ~/x && ./scripts/ship.sh", "ship.sh"),
            ("PATH=\"$HOME/.cargo/bin:$PATH\" cargo check", "cargo"),
            ("timeout 30 osascript -e 'tell app'", "osascript"),
            ("git -C /x log --oneline -5", "git"),
            ("cd /tmp/x; UA='Mozilla/5.0 (Macintosh; Intel)'; curl -A \"$UA\" x", "curl"),
            ("set -e; cd /tmp; mkdir -p repos; cd repos", "mkdir"),
        ] {
            assert_eq!(program(cmd).as_deref(), Some(prog), "{cmd}");
        }
        assert_eq!(
            command_from("cd /tmp/thingsre && osascript probes/verbs.applescript 2>&1 | head"),
            Some(("osascript".into(), "osascript probes/verbs.applescript 2>&1 | head"))
        );
        let steps = vec![
            Step::Tool {
                call_id: String::new(),
                name: "bash".into(),
                args: json!({"command": "git status"}),
                preview: String::new(),
                output: None,
                failed: false,
                diff: None,
            },
            Step::Tool {
                call_id: String::new(),
                name: "edit".into(),
                args: json!({"path": "/a/b/things-do"}),
                preview: String::new(),
                output: None,
                failed: false,
                diff: None,
            },
        ];
        assert_eq!(parts(&steps, Filter::Commands, &[0, 1]), vec![("git".to_string(), 1)]);
        assert_eq!(parts(&steps, Filter::Edits, &[0, 1]), vec![("things-do".to_string(), 1)]);
    }

    #[test]
    fn rule_1_the_work_reads_in_order_as_runs() {
        let steps = vec![
            Step::Thought("why".into()),
            call("bash", false, ""),
            Step::Thought("why".into()),
            call("bash", false, ""),
            call("read", false, ""),
            call("bash", true, ""),
            Step::Note("found it".into()),
            call("edit", false, ""),
            call("edit", false, ""),
            Step::Thought("thinking now".into()),
        ];
        assert_eq!(
            runs(&steps),
            vec![
                Line::Run { kind: Filter::Commands, calls: vec![1, 3, 4, 5] },
                Line::Step(6),
                Line::Run { kind: Filter::Edits, calls: vec![7, 8] },
                Line::Step(9),
            ]
        );
    }


}
