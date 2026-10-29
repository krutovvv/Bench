//! A piece of work's steps as the opened work shows them: each call as an action in a few words
//! and where it happened, the reasoning that led to it, what the evaluator said about it. Only
//! what the journal already holds — a call's tool and arguments, its result, the thoughts before
//! it — so an old thread reads as a new one.
//!
//! The rules (each is a test below):
//! 1. A call is said as an action and a place (`say`): a command by what its program does, read
//!    from the command's own syntax (`cat package.json` in `supathings-mcp` is "Show package.json"
//!    there); a file tool by its file and folder; a search by its words.
//! 2. Reasoning goes with the call it led to (`reasoning_before`); the list is the calls and the
//!    agent's words (`listed`).
//! 3. The evaluator's words are the ones it put in a call's result (`evaluator_word`).
//! 4. The list folds into runs (`runs`): calls of one kind in a row are one line, told in words
//!    (`title`: "Ran 4 commands"); a single call of another kind between two of them stays
//!    inside; the agent's words are lines of their own.

use crate::Step;

/// What kind of call a tool makes: its icon in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    Read,
    Edit,
    Search,
    Web,
    Browser,
    Plan,
    Other,
}

impl Kind {
    pub fn of(name: &str) -> Kind {
        let n = tool_name(name);
        match n.as_str() {
            "bash" | "ssh_run" | "process" | "terminal" | "execute_code" => Kind::Command,
            "read" | "ls" | "skill_view" => Kind::Read,
            "edit" | "write" | "multi_edit" | "skill_manage" => Kind::Edit,
            "grep" | "glob" | "lsp" | "session_search" | "tool_search" => Kind::Search,
            "todo" => Kind::Plan,
            _ if name.starts_with("mcp__browser") || n.starts_with("browser") => Kind::Browser,
            _ if n.contains("search") || n.contains("extract") || n.contains("fetch") => Kind::Web,
            _ => Kind::Other,
        }
    }
}

/// The steps the list shows, by index: every call and the agent's words between them; reasoning
/// goes with its call, and stands alone only at the end, while the model is still thinking.
pub fn listed(steps: &[Step]) -> Vec<usize> {
    let last_call = steps.iter().rposition(|s| !matches!(s, Step::Thought(_)));
    steps
        .iter()
        .enumerate()
        .filter(|(k, s)| match s {
            Step::Thought(_) => last_call.is_none_or(|l| *k == l + 1),
            _ => true,
        })
        .map(|(k, _)| k)
        .collect()
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

/// A call said for a person: what was done, and where (a folder, a site), if it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub action: String,
    pub place: Option<String>,
}

/// A step said for a person (rule 1). The agent's words and reasoning are said by their text.
pub fn say(step: &Step) -> Said {
    let (name, args) = match step {
        Step::Tool { name, args, .. } => (name.as_str(), args),
        Step::Note(t) | Step::Thought(t) => {
            return Said {
                action: first_line(t, 120),
                place: None,
            };
        }
    };
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").trim();
    let tool = tool_name(name);
    let said = |action: String, place: Option<String>| Said { action, place };
    let file = |p: &str| said(base(p), parent(p));
    match (Kind::of(name), tool.as_str()) {
        (Kind::Command, "process") => match arg("action") {
            "start" => {
                let s = say_command(arg("command"));
                said(format!("Start in background: {}", s.action), s.place)
            }
            "log" => said("Check a background process".into(), None),
            "stop" => said("Stop a background process".into(), None),
            _ => said("List background processes".into(), None),
        },
        (Kind::Command, _) => say_command(arg("command")),
        (Kind::Read, "skill_view") => said(format!("Open the skill {}", arg("name")), None),
        (Kind::Read, "ls") => said(format!("List {}", base(arg("path"))), parent(arg("path"))),
        (Kind::Read, _) => {
            let f = file(arg("path"));
            said(format!("Read {}", f.action), f.place)
        }
        (Kind::Edit, "skill_manage") => said(format!("Update the skill {}", arg("name")), None),
        (Kind::Edit, "write") => {
            let f = file(arg("path"));
            said(format!("Write {}", f.action), f.place)
        }
        (Kind::Edit, _) => {
            let f = file(arg("path"));
            said(format!("Edit {}", f.action), f.place)
        }
        (Kind::Search, "session_search") => {
            said(format!("Search past threads for {}", quote(arg("query"))), None)
        }
        (Kind::Search, "tool_search") => {
            said(format!("Look for tools: {}", quote(arg("query"))), None)
        }
        (Kind::Search, "glob") => {
            said(format!("Find files {}", quote(arg("pattern"))), place_of(arg("path")))
        }
        (Kind::Search, "lsp") => said(format!("Look up {}", arg("symbol").max(arg("action"))), None),
        (Kind::Search, _) => {
            let q = arg("pattern").max(arg("query"));
            let action = if q.is_empty() {
                "Search".to_string()
            } else {
                format!("Search for {}", quote(q))
            };
            said(action, place_of(arg("path")))
        }
        (Kind::Plan, _) => said("Update the plan".into(), None),
        (Kind::Web, t) if t.contains("search") => {
            said(format!("Search the web for {}", quote(arg("query"))), None)
        }
        (Kind::Web, _) => said("Read a web page".into(), url_host(args)),
        (Kind::Browser, t) => {
            let what = t.trim_start_matches("browser_");
            let action = match what {
                "navigate" => "Open a page",
                "snapshot" => "Look at the page",
                "click" => "Click on the page",
                "type" | "fill_form" => "Type on the page",
                "evaluate" => "Run a script on the page",
                "tabs" => "Switch tabs",
                _ => what,
            };
            said(action.into(), url_host(args))
        }
        (_, "clarify") => said("Ask you".into(), None),
        (_, "memory") => said("Update memory".into(), None),
        (_, t) => {
            let server = name.strip_prefix("mcp__").and_then(|n| n.split("__").next());
            said(t.replace('_', " "), server.map(str::to_string))
        }
    }
}

/// A shell command said for a person: what its program does to what, and the folder it ran in
/// (`cd` before it, or the file's own folder).
pub fn say_command(command: &str) -> Said {
    let line = command.lines().find(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#')
    });
    let Some(line) = line else {
        return Said {
            action: "Run a command".into(),
            place: None,
        };
    };
    let cd = cd_dir(line);
    // `echo $PATH | tr ':' '\n'`: showing a variable, whatever filters it.
    if let Some(var) = line
        .trim_start()
        .strip_prefix("echo ")
        .filter(|r| r.contains('|'))
        .and_then(|r| r.split('|').next()?.split_once('$').map(|(_, v)| v))
        .map(|r| r.trim_start_matches('{'))
        .map(|r| r.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or(""))
        .filter(|v| !v.is_empty())
    {
        return Said {
            action: format!("Show {var}"),
            place: None,
        };
    }
    let Some((prog, from)) = command_from(command) else {
        return Said {
            action: match &cd {
                Some(d) => format!("Go to {d}"),
                None => format!("Run {}", first_line(line, 40)),
            },
            place: None,
        };
    };
    let given: Vec<&str> = words(from)
        .into_iter()
        .skip(1)
        .map(|(_, w)| w)
        .take_while(|w| !matches!(*w, ";" | "&" | "|"))
        .filter(|w| !w.starts_with('<') && !w.starts_with('>') && !w.starts_with("2>"))
        .collect();
    let plain: Vec<&str> = given.iter().copied().filter(|w| !w.starts_with('-')).collect();
    let unquote = |w: &str| w.trim_matches(['"', '\'']).to_string();
    let target = plain
        .iter()
        .find(|w| w.contains('/') || w.contains('.'))
        .or(plain.first())
        .map(|w| unquote(w))
        .unwrap_or_default();
    let name = base(&target);
    let place = cd.or_else(|| parent(&target));
    let script = from.contains("<<") || given.iter().any(|w| *w == "-c" || *w == "-e");
    let lang = |l: &str| {
        if script || name.is_empty() {
            format!("Run a {l} script")
        } else {
            format!("Run {name}")
        }
    };
    let second = plain.get(1).map(|w| base(&unquote(w))).unwrap_or_default();
    let action = match prog.as_str() {
        "ls" | "tree" => format!("List {}", or(&name, "the folder")),
        "cat" | "head" | "tail" | "less" | "bat" | "nl" => format!("Show {}", or(&name, "a file")),
        "sed" if given.iter().any(|w| w.starts_with("-i")) => {
            format!("Edit {}", or(&second, "a file"))
        }
        "sed" | "awk" => format!("Show part of {}", or(&second, "a file")),
        "grep" | "rg" | "ag" => format!(
            "Search for {}",
            quote(&plain.first().map(|w| unquote(w)).unwrap_or_default())
        ),
        "find" | "fd" | "mdfind" => format!("Find files in {}", or(&name, "the folder")),
        "mkdir" => format!("Create the folder {name}"),
        "mv" => format!("Move {}", or(&name, "files")),
        "cp" | "rsync" => format!("Copy {}", or(&name, "files")),
        "rm" => format!("Delete {}", or(&name, "files")),
        "chmod" => format!("Make {} runnable", or(&second, "a file")),
        "touch" => format!("Create {}", or(&name, "a file")),
        "wc" => format!("Count lines in {}", or(&name, "files")),
        "diff" | "cmp" => format!("Compare {}", or(&name, "files")),
        "git" => format!("Git {}", git_sub(&given)),
        "python3" | "python" => lang("Python"),
        "node" | "bun" | "deno" | "tsx" => lang("JavaScript"),
        "osascript" if script || name.is_empty() => "Run AppleScript".into(),
        "osascript" => format!("Run AppleScript {name}"),
        "bash" | "sh" | "zsh" => lang("shell"),
        "curl" | "wget" => format!("Fetch {}", host(&target).unwrap_or_else(|| or(&name, "a page"))),
        "open" => format!("Open {}", or(&name, "it")),
        "defaults" => format!("Read the setting {second}"),
        "sqlite3" => format!("Query {}", or(&name, "a database")),
        "jq" => "Read JSON".into(),
        "date" => "Show the date".into(),
        "whoami" | "id" => "Show the user".into(),
        "pwd" => "Show the folder".into(),
        "uname" | "sw_vers" => "Show the system".into(),
        "which" | "type" | "whereis" => format!("Find the program {name}"),
        "stat" | "file" | "xattr" | "mdls" => format!("Look at {}", or(&name, "a file")),
        "plutil" => format!("Read {}", or(&name, "a property list")),
        "codesign" | "spctl" => format!("Check the signature of {}", or(&name, "an app")),
        "launchctl" => format!("Services: {}", plain.first().copied().unwrap_or("")),
        "tr" | "sort" | "uniq" | "cut" | "xargs" => "Filter text".into(),
        "du" | "df" => "Check disk space".into(),
        "ps" | "pgrep" | "lsof" => "Look at running programs".into(),
        "kill" | "pkill" | "killall" => format!("Stop {}", or(&name, "a program")),
        "sleep" => format!("Wait {name} s"),
        "cargo" | "npm" | "pnpm" | "yarn" | "make" | "swift" | "brew" | "pip" | "pip3" | "uv"
        | "docker" | "xcodebuild" => {
            format!("{} {}", capital(&prog), plain.first().copied().unwrap_or(""))
        }
        _ => format!("Run {prog}"),
    };
    Said {
        action: action.trim().to_string(),
        place,
    }
}

/// A line of the folded list: calls of one kind in a row, or one step on its own (the agent's
/// words, reasoning still waiting for its call, or a call alone).
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Run { kind: Kind, calls: Vec<usize> },
    Step(usize),
}

/// The listed steps folded into runs (rule 4).
pub fn runs(steps: &[Step]) -> Vec<Line> {
    let mut lines: Vec<Line> = vec![];
    for k in listed(steps) {
        match &steps[k] {
            Step::Tool { name, .. } => {
                let kind = Kind::of(name);
                match lines.last_mut() {
                    Some(Line::Run { kind: last, calls }) if *last == kind => calls.push(k),
                    _ => lines.push(Line::Run { kind, calls: vec![k] }),
                }
            }
            _ => lines.push(Line::Step(k)),
        }
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
    // A run of one call is that call's own line.
    lines
        .into_iter()
        .map(|l| match l {
            Line::Run { calls, .. } if calls.len() == 1 => Line::Step(calls[0]),
            l => l,
        })
        .collect()
}

/// A run in words: what was done and how many times.
pub fn title(steps: &[Step], kind: Kind, calls: &[usize]) -> String {
    let n = calls
        .iter()
        .filter(|k| matches!(&steps[**k], Step::Tool { name, .. } if Kind::of(name) == kind))
        .count();
    match kind {
        Kind::Command => format!("Ran {n} commands"),
        Kind::Read => format!("Read {n} files"),
        Kind::Edit => format!("Made {n} edits"),
        Kind::Search => format!("Searched {n} times"),
        Kind::Web => format!("Searched the web {n} times"),
        Kind::Browser => format!("{n} steps in the browser"),
        Kind::Plan => format!("Updated the plan {n} times"),
        Kind::Other => format!("Used tools {n} times"),
    }
}

/// The plan's item a `todo` call put under way: the first in progress, else the first not done.
pub fn plan_item(step: &Step) -> Option<String> {
    let Step::Tool { args, .. } = step else {
        return None;
    };
    let todos = args.get("todos")?.as_array()?;
    let status =
        |t: &serde_json::Value, s: &str| t.get("status").and_then(|v| v.as_str()) == Some(s);
    todos
        .iter()
        .find(|t| status(t, "in_progress"))
        .or_else(|| todos.iter().find(|t| !status(t, "completed")))
        .and_then(|t| t.get("content")?.as_str())
        .map(str::to_string)
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
        // `DB=$(ls -t …)`: the command inside the substitution is what runs.
        let w = match w.find("$(") {
            Some(i) => w[i + 2..].trim_matches(|c| c == '"' || c == '\''),
            None => w,
        };
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
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        // `\ ` and the like belong to the word: `Group\ Containers` is one path.
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && quote.is_none() {
            escaped = true;
            start.get_or_insert(i);
            continue;
        }
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

/// The folder a line `cd`s into before its program, by its name.
fn cd_dir(line: &str) -> Option<String> {
    let ws = words(line);
    let i = ws.iter().position(|(_, w)| *w == "cd")?;
    let dir = ws.get(i + 1)?.1.trim_matches(['"', '\'']);
    (!dir.is_empty() && !matches!(dir, ";" | "&" | "|")).then(|| base(dir))
}

fn git_sub(given: &[&str]) -> String {
    let mut it = given.iter();
    while let Some(w) = it.next() {
        if *w == "-C" || *w == "-c" {
            it.next();
            continue;
        }
        if !w.starts_with('-') {
            return w.to_string();
        }
    }
    String::new()
}

fn url_host(args: &serde_json::Value) -> Option<String> {
    let url = ["url", "urls"].iter().find_map(|k| match args.get(*k)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => a.first()?.as_str().map(str::to_string),
        _ => None,
    })?;
    host(&url)
}

fn host(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let h = rest.split(['/', '?', '#', ':']).next()?;
    Some(h.trim_start_matches("www.").to_string()).filter(|h| !h.is_empty())
}

fn quote(text: &str) -> String {
    let t = first_line(text, 40);
    if t.is_empty() {
        String::new()
    } else {
        format!("“{t}”")
    }
}

fn first_line(text: &str, n: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = line.replace(['`', '*'], "");
    if line.chars().count() > n {
        format!("{}…", line.chars().take(n).collect::<String>().trim_end())
    } else {
        line
    }
}

fn or(text: &str, otherwise: &str) -> String {
    if text.is_empty() {
        otherwise.to_string()
    } else {
        text.to_string()
    }
}

fn capital(word: &str) -> String {
    let mut c = word.chars();
    c.next()
        .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

fn place_of(path: &str) -> Option<String> {
    (!path.is_empty()).then(|| base(path))
}

/// The folder a path is in, by its name; inside an app bundle, the app.
fn parent(path: &str) -> Option<String> {
    let p = path.trim_end_matches('/');
    if let Some(i) = p.find(".app/") {
        return Some(base(&p[..i + 4]));
    }
    let (dir, _) = p.rsplit_once('/')?;
    Some(base(dir)).filter(|d| !d.is_empty() && d != "~")
}

fn base(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .replace("\\ ", " ")
}

/// A tool's own name: an MCP server's tool without the server (`mcp__ozon__ozon_search` is
/// `ozon_search`).
fn tool_name(name: &str) -> String {
    name.rsplit("__").next().unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(name: &str, args: serde_json::Value) -> Step {
        Step::Tool {
            call_id: String::new(),
            name: name.into(),
            args,
            preview: String::new(),
            output: None,
            failed: false,
            diff: None,
        }
    }

    fn said(action: &str, place: Option<&str>) -> Said {
        Said {
            action: action.into(),
            place: place.map(str::to_string),
        }
    }

    #[test]
    fn rule_1_a_command_is_said_by_what_its_program_does() {
        for (cmd, action, place) in [
            (
                "cd ~/.npm-global/lib/node_modules/supathings-mcp && { echo \"=== pkg ===\"; cat package.json | head -60; }",
                "Show package.json",
                Some("supathings-mcp"),
            ),
            ("ls /Applications 2>/dev/null | head -100", "List Applications", None),
            ("cd /x && sed -n '120,400p' README.md", "Show part of README.md", Some("x")),
            ("grep -n \"LIST_NAMES\" things-do | head -8", "Search for “LIST_NAMES”", None),
            ("cd /tmp && python3 - <<'EOF'\nprint(1)\nEOF", "Run a Python script", Some("tmp")),
            (
                "osascript probes/verbs.applescript 2>&1",
                "Run AppleScript verbs.applescript",
                Some("probes"),
            ),
            (
                "cd /tmp/x; UA='Mozilla/5.0 (Macintosh; Intel)'; curl -A \"$UA\" https://github.com/a/b",
                "Fetch github.com",
                Some("x"),
            ),
            ("git -C /x log --oneline -5", "Git log", None),
            ("time cargo build 2>&1 | tail -3", "Cargo build", None),
            ("cd /tmp/thingsre/repos", "Go to repos", None),
            ("DB=$(ls -t ~/Library/Group\\ Containers/X/Things.* 2>/dev/null | head -3); echo \"$DB\"", "List Things.*", Some("X")),
            ("ls ~/Library/Group\\ Containers/ 2>&1 | grep -i things", "List Group Containers", Some("Library")),
            ("echo \"PATH=$PATH\" | tr ':' '\\n'", "Show PATH", None),
            ("echo $PATH | tr ':' '\\n' | grep -n bin", "Show PATH", None),
            ("date '+%A %Y-%m-%d'", "Show the date", None),
            (
                "defaults read /Applications/Things3.app/Contents/Info.plist CFBundleShortVersionString",
                "Read the setting Info.plist",
                Some("Things3.app"),
            ),
        ] {
            assert_eq!(say_command(cmd), said(action, place), "{cmd}");
        }
    }

    #[test]
    fn rule_1_a_file_tool_by_its_file_a_search_by_its_words() {
        assert_eq!(
            say(&call("read", json!({"path": "/Users/v/.bench/projects/0e26/mcp.json"}))),
            said("Read mcp.json", Some("0e26"))
        );
        assert_eq!(
            say(&call(
                "mcp__tavily__tavily_search",
                json!({"query": "Things 3 MCP server"})
            )),
            said("Search the web for “Things 3 MCP server”", None)
        );
        assert_eq!(
            say(&call("skill_view", json!({"name": "deep-research"}))),
            said("Open the skill deep-research", None)
        );
    }

    #[test]
    fn rule_2_reasoning_goes_with_its_call() {
        let steps = vec![
            Step::Thought("first".into()),
            Step::Thought("second".into()),
            call("bash", json!({"command": "ls"})),
            Step::Note("words".into()),
            Step::Thought("thinking now".into()),
        ];
        assert_eq!(listed(&steps), vec![2, 3, 4]);
        assert_eq!(reasoning_before(&steps, 2), vec!["first", "second"]);
    }

    #[test]
    fn rule_4_the_list_folds_into_runs() {
        let bash = || call("bash", json!({"command": "ls"}));
        let steps = vec![
            Step::Thought("why".into()),
            bash(),
            bash(),
            call("read", json!({"path": "/a/b"})),
            bash(),
            Step::Note("found it".into()),
            call("edit", json!({"path": "/a/b"})),
            Step::Thought("thinking now".into()),
        ];
        assert_eq!(
            runs(&steps),
            vec![
                Line::Run { kind: Kind::Command, calls: vec![1, 2, 3, 4] },
                Line::Step(5),
                Line::Step(6),
                Line::Step(7),
            ]
        );
        assert_eq!(title(&steps, Kind::Command, &[1, 2, 3, 4]), "Ran 3 commands");
    }

    #[test]
    fn rule_3_the_evaluators_words_in_a_result() {
        assert_eq!(
            evaluator_word("Tool call blocked: [Evaluator, for this step: a repeat. Not run.]"),
            Some("a repeat. Not run.")
        );
        assert_eq!(evaluator_word("plain output"), None);
    }
}
