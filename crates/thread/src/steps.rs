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
//! 5. The work reads as sentences (`sentences`): neighbouring calls told together in order.
//! 6. The agent's own words between calls head the sentences after them (`chapters`).
//! 8. Inside a milestone a call is what it did (`detail`: the agent's echoed label, its
//!    AppleScript, its script's file) beside what came back (`result_line`), the reasoning's
//!    first sentence a beat between them (`beat`).
//! 7. The work's strips are milestones (`milestones`): the agent's words, or a sentence where
//!    it wrote none; inside, its calls as a chain of chips in order (`chips`).
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
        (Kind::Search, "session_search") => said(format!("Search past threads {}", for_(arg("query"))).trim().to_string(), None),
        (Kind::Search, "tool_search") => said("Look for tools".into(), None),
        (Kind::Search, "glob") => said("Find files".into(), place_of(arg("path"))),
        (Kind::Search, "lsp") => said(format!("Look up {}", arg("symbol").max(arg("action"))), None),
        (Kind::Search, _) => {
            let q = quote(arg("pattern").max(arg("query")));
            let at = place_of(arg("path"));
            let action = match (q.is_empty(), &at) {
                (false, _) => format!("Search for {q}"),
                (true, Some(p)) => format!("Search {p}"),
                (true, None) => "Search the files".to_string(),
            };
            said(action, None)
        }
        (Kind::Plan, _) => said("Update the plan".into(), None),
        (Kind::Web, t) if t.contains("search") => {
            said(format!("Search the web {}", for_(arg("query"))).trim().to_string(), None)
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
        (_, t) => match name.strip_prefix("mcp__").and_then(|n| n.split("__").next()) {
            Some(server) => said(format!("Ask {} to {}", capital(server), t.replace('_', " ")), None),
            None => said(format!("Use {}", t.replace('_', " ")), None),
        },
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
    let cd = command
        .lines()
        .find_map(cd_dir)
        .and_then(plain_place);
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
    // A variable (`$F`) names nothing a person can read.
    let target = if target.starts_with('$') { String::new() } else { target };
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
    let second = plain
        .get(1)
        .map(|w| base(&unquote(w)))
        .filter(|w| !w.starts_with('$'))
        .unwrap_or_default();
    let action = match prog.as_str() {
        "ls" | "tree" => format!("List {}", or(&name, "the folder")),
        "cat" | "head" | "tail" | "less" | "bat" | "nl" => format!("Show {}", or(&name, "a file")),
        "sed" if given.iter().any(|w| w.starts_with("-i")) => {
            format!("Edit {}", or(&second, "a file"))
        }
        "sed" | "awk" => format!("Show part of {}", or(&second, "a file")),
        "grep" | "rg" | "ag" => {
            let q = quote(&plain.first().map(|w| unquote(w)).unwrap_or_default());
            let what = plain.get(1).map(|w| base(&unquote(w))).unwrap_or_default();
            match (q.is_empty(), what.is_empty()) {
                (false, _) => format!("Search for {q}"),
                (true, false) => format!("Search {what}"),
                (true, true) => "Search the output".to_string(),
            }
        }
        "find" | "fd" | "mdfind" => format!(
            "Find files in {}",
            or(&plain_place(name.clone()).unwrap_or_default(), "the folder")
        ),
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

/// A sentence of the opened work: neighbouring calls told together in the order they ran
/// ("Ran a Python script in .bench, looked for files in backups and searched two notes."), or
/// the agent's own words between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Sentence {
    pub text: String,
    /// The steps it tells, in order: what it unfolds to.
    pub steps: Vec<usize>,
    pub failed: usize,
    pub judged: usize,
    /// The agent's words, not calls.
    pub words: bool,
}

/// The most clauses a sentence takes before the next one starts.
const CLAUSES: usize = 4;

/// The listed steps as sentences (rule 5): calls join the sentence before them until it has
/// `CLAUSES` clauses; a failure is counted, not a break. Places stay with the steps. The
/// same call again is counted ("searched SKILL.md twice"), the same verb on another thing joins
/// the clause ("read a.md and b.md"). Nothing is reordered.
pub fn sentences(steps: &[Step]) -> Vec<Sentence> {
    struct Clause {
        verb: String,
        objects: Vec<String>,
        count: usize,
    }
    struct Open {
        clauses: Vec<Clause>,
        steps: Vec<usize>,
        failed: usize,
        judged: usize,
    }
    fn close(open: &mut Option<Open>, out: &mut Vec<Sentence>) {
        let Some(o) = open.take() else { return };
        let mut said: Vec<String> = vec![];
        for c in &o.clauses {
            let mut t = c.verb.clone();
            let objs = match c.objects.len() {
                0 => String::new(),
                1 => c.objects[0].clone(),
                2 => format!("{} and {}", c.objects[0], c.objects[1]),
                3 => format!("{}, {} and {}", c.objects[0], c.objects[1], c.objects[2]),
                n => format!("{}, {} and {} more", c.objects[0], c.objects[1], n - 2),
            };
            if !objs.is_empty() {
                t = format!("{t} {objs}");
            }
            match c.count {
                1 => {}
                2 => t.push_str(" twice"),
                n => t.push_str(&format!(" {n} times")),
            }
            said.push(t);
        }
        let text = match said.len() {
            0 => String::new(),
            1 => said[0].clone(),
            n => format!("{} and {}", said[..n - 1].join(", "), said[n - 1]),
        };
        out.push(Sentence {
            text: format!("{}.", capital(&text)),
            steps: o.steps,
            failed: o.failed,
            judged: o.judged,
            words: false,
        });
    }
    let mut out = vec![];
    let mut open: Option<Open> = None;
    for k in listed(steps) {
        let step = &steps[k];
        let Step::Tool { failed, output, .. } = step else {
            close(&mut open, &mut out);
            out.push(Sentence {
                text: first_line(match step { Step::Note(t) | Step::Thought(t) => t, _ => "" }, 160),
                steps: vec![k],
                failed: 0,
                judged: 0,
                words: true,
            });
            continue;
        };
        let said = say(step);
        let (verb, object) = past(&said.action);
        if let Some(o) = &open {
            let same = o.clauses.last().is_some_and(|c| c.verb == verb);
            if !same && o.clauses.len() >= CLAUSES {
                close(&mut open, &mut out);
            }
        }
        let o = open.get_or_insert_with(|| Open { clauses: vec![], steps: vec![], failed: 0, judged: 0 });
        o.steps.push(k);
        o.failed += usize::from(*failed);
        o.judged += usize::from(output.as_deref().and_then(evaluator_word).is_some());
        match o.clauses.last_mut() {
            Some(c) if c.verb == verb => {
                if c.objects.last() == Some(&object) || object.is_empty() {
                    c.count += 1;
                } else if c.count == 1 {
                    c.objects.push(object);
                } else {
                    o.clauses.push(Clause { verb, objects: vec![object], count: 1 });
                }
            }
            _ => o.clauses.push(Clause {
                verb,
                objects: [object].into_iter().filter(|x| !x.is_empty()).collect(),
                count: 1,
            }),
        }
    }
    close(&mut open, &mut out);
    out
}

/// A chapter of the opened work: the agent's own words between its calls, standing over the
/// sentences that follow until its next words — as the work line's narrative does while the
/// work runs. Work before any words has no heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    /// The words' step and their first line.
    pub heading: Option<(usize, String)>,
    pub sentences: Vec<Sentence>,
}

/// The opened work as chapters (rule 6).
pub fn chapters(steps: &[Step]) -> Vec<Chapter> {
    let mut out: Vec<Chapter> = vec![];
    for s in sentences(steps) {
        let note = s.words && matches!(steps[s.steps[0]], Step::Note(_));
        if note {
            let line = match &steps[s.steps[0]] {
                Step::Note(t) => heading_of(t),
                _ => String::new(),
            };
            out.push(Chapter {
                heading: Some((s.steps[0], line)),
                sentences: vec![],
            });
            continue;
        }
        match out.last_mut() {
            Some(c) => c.sentences.push(s),
            None => out.push(Chapter {
                heading: None,
                sentences: vec![s],
            }),
        }
    }
    out
}

/// A milestone of the opened work, one of its strips: the agent's own words and the calls
/// after them until its next words; with no words, a sentence of the calls themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct Milestone {
    pub heading: String,
    /// The heading is the agent's words (its step first in `steps`), not a sentence of calls.
    pub words: bool,
    /// Its steps in order: the words' step, then the calls.
    pub steps: Vec<usize>,
}

/// The opened work as milestones (rule 7).
pub fn milestones(steps: &[Step]) -> Vec<Milestone> {
    let mut out = vec![];
    for c in chapters(steps) {
        match c.heading {
            Some((k, heading)) => out.push(Milestone {
                heading,
                words: true,
                steps: std::iter::once(k)
                    .chain(c.sentences.iter().flat_map(|s| s.steps.iter().copied()))
                    .collect(),
            }),
            None => out.extend(c.sentences.into_iter().map(|s| Milestone {
                heading: s.text,
                words: s.words,
                steps: s.steps,
            })),
        }
    }
    out
}

/// What exactly a call did, in the agent's own words where it wrote some (rule 8): the label it
/// echoed (`echo "=== 1. может ли процесс управлять Things ==="`), what its AppleScript asks
/// (`make new to do «ZZZ проверка»`), the file a script works on.
pub fn detail(step: &Step) -> Option<String> {
    let Step::Tool { name, args, .. } = step else {
        return None;
    };
    if Kind::of(name) != Kind::Command {
        return None;
    }
    let command = args.get("command").and_then(|c| c.as_str()).unwrap_or("");
    let label = echo_label(command);
    let script = apple_script(command).or_else(|| script_file(command));
    match (label, script) {
        (Some(l), Some(s)) => Some(format!("{l} — {s}")),
        (l, s) => l.or(s),
    }
}

/// The first line of what came back, for a person: past the labels the command echoed and the
/// evaluator's words; an error without its position (`116:120: execution error:`); a long path as
/// its file's name.
pub fn result_line(step: &Step) -> Option<String> {
    let Step::Tool { output: Some(o), .. } = step else {
        return None;
    };
    let line = o.lines().map(str::trim).find(|l| {
        !l.is_empty()
            && !l.contains("[Evaluator")
            && !l.starts_with("===")
            && !l.starts_with("---")
            && !l.starts_with("###")
            && !l.starts_with("<harness")
            && !l.starts_with("</harness")
            && l.chars().any(char::is_alphanumeric)
    })?;
    let mut l = line.to_string();
    // `116:120: execution error: Переменная …` — the position says nothing to a person.
    if let Some(i) = l.find("execution error:") {
        l = l[i + "execution error:".len()..].trim().to_string();
    }
    l = l.trim_start_matches("Error:").trim_start_matches("error:").trim().to_string();
    let l = l
        .split(' ')
        .map(|w| if w.matches('/').count() >= 3 { format!("…/{}", base(w)) } else { w.to_string() })
        .collect::<Vec<_>>()
        .join(" ");
    Some(first_line(&l, 140))
}

/// The reasoning before a call, as a beat between the rows: its first sentence.
pub fn beat(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line = line.replace(['`', '*'], "");
    let end = line
        .match_indices(". ")
        .map(|(i, _)| i + 1)
        .find(|&i| i >= 20)
        .unwrap_or(line.len());
    Some(first_line(&line[..end], 160)).filter(|b| !b.is_empty())
}

/// The first label a command echoes: words, not a separator or a variable.
fn echo_label(command: &str) -> Option<String> {
    let mut rest = command;
    while let Some(i) = rest.find("echo ") {
        rest = &rest[i + 5..];
        let r = rest.trim_start();
        let q = r.chars().next()?;
        let text = if q == '"' || q == '\'' {
            r[1..].split(q).next().unwrap_or("")
        } else {
            r.split([';', '&', '|', '\n']).next().unwrap_or("")
        };
        let t = text
            .trim()
            .trim_matches(|c: char| "=-#*:>".contains(c) || c.is_whitespace())
            .trim();
        if t.chars().filter(|c| c.is_alphabetic()).count() >= 3 && !t.contains('$') {
            return Some(first_line(t, 70));
        }
    }
    None
}

/// What an AppleScript asks of its app: the statement inside `tell application "…"`, its
/// `set x to` dropped, a record's `name:"…"` said as «…».
fn apple_script(command: &str) -> Option<String> {
    if !command.contains("osascript") {
        return None;
    }
    let after = |s: &str| -> Option<String> {
        let i = s.find("tell application")?;
        let rest = &s[i..];
        let stmt = match rest.find(" to ") {
            Some(j) if !rest[..j].contains('\n') => rest[j + 4..].to_string(),
            _ => rest
                .lines()
                .skip(1)
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with("--") && !l.starts_with("end"))?
                .to_string(),
        };
        let stmt = stmt.split(['\'', '\n']).next().unwrap_or("").trim().to_string();
        let stmt = match stmt.strip_prefix("set ").and_then(|r| r.split_once(" to ")) {
            Some((_, r)) => r.to_string(),
            None => stmt,
        };
        Some(stmt)
    };
    let stmt = after(command)?;
    let stmt = match stmt.find("name:\"") {
        Some(i) => {
            let rest = &stmt[i + 6..];
            let name = rest.split('"').next().unwrap_or("");
            let head = stmt[..i].trim_end_matches(['{', ' ']).trim_end_matches("with properties").trim();
            format!("{head} «{}»", first_line(name, 40))
        }
        None => stmt,
    };
    Some(first_line(&stmt, 80)).filter(|s| !s.is_empty())
}

/// The file a Python or Node script works on: the first quoted path in it, by its name.
fn script_file(command: &str) -> Option<String> {
    if !(command.contains("python") || command.contains("node")) || !command.contains("<<") {
        return None;
    }
    let body = command.split_once("<<")?.1;
    body.split(['\'', '"'])
        .skip(1)
        .step_by(2)
        .find(|q| q.contains('/') && q.contains('.') && !q.contains(' '))
        .map(base)
}

/// A chip of a milestone's chain: calls of one kind in a row, in the order they ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub kind: Kind,
    pub calls: Vec<usize>,
    pub label: String,
    pub failed: usize,
}

/// A milestone's calls as a chain of chips (rule 7): each run of one kind in a row is a chip, said
/// as the one call it holds ("Edit things-do") or as how many ("3 commands", "Edit things-do ×2"
/// when they are on one thing).
pub fn chips(steps: &[Step], of: &[usize]) -> Vec<Chip> {
    let mut out: Vec<Chip> = vec![];
    for &k in of {
        let Step::Tool { name, failed, .. } = &steps[k] else {
            continue;
        };
        let kind = Kind::of(name);
        match out.last_mut() {
            Some(c) if c.kind == kind => {
                c.calls.push(k);
                c.failed += usize::from(*failed);
            }
            _ => out.push(Chip {
                kind,
                calls: vec![k],
                label: String::new(),
                failed: usize::from(*failed),
            }),
        }
    }
    for c in &mut out {
        let said: Vec<Said> = c.calls.iter().map(|k| say(&steps[*k])).collect();
        let n = said.len();
        c.label = if n == 1 || said.iter().all(|s| s.action == said[0].action) {
            let one = first_line(&said[0].action, 40);
            if n == 1 { one } else { format!("{one} ×{n}") }
        } else {
            match c.kind {
                Kind::Command => format!("{n} commands"),
                Kind::Read => format!("{n} reads"),
                Kind::Edit => format!("{n} edits"),
                Kind::Search => format!("{n} searches"),
                Kind::Web => format!("{n} web searches"),
                Kind::Browser => format!("{n} browser steps"),
                Kind::Plan => format!("Plan ×{n}"),
                Kind::Other => format!("{n} tools"),
            }
        };
    }
    out
}

/// The agent's words as a heading: their first line that says something, without Markdown.
fn heading_of(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("```"))
        .unwrap_or("");
    first_line(line.trim_start_matches(['#', '-', '>', ' ']), 160)
}

/// An action ("Show package.json") told as done, its verb and its object apart: ("looked at",
/// "package.json").
fn past(action: &str) -> (String, String) {
    const TWO: &[(&str, &str)] = &[
        ("Show the date", "checked the date"),
        ("Show the user", "checked the user"),
        ("Show the folder", "checked the folder"),
        ("Show the system", "checked the system"),
        ("Show part of", "looked at part of"),
        ("Find files in", "looked for files in"),
        ("Find files", "looked for files"),
        ("Find the program", "looked for the program"),
        ("Look for tools", "looked for tools"),
        ("Look at", "looked at"),
        ("Look up", "looked up"),
        ("Search the web", "searched the web"),
        ("Search past threads", "searched past threads"),
        ("Search for", "searched for"),
        ("Read the setting", "read the setting"),
        ("Read a web page", "read a page"),
        ("Run a", "ran a"),
        ("Run AppleScript", "ran AppleScript"),
        ("Open the skill", "opened the skill"),
        ("Update the plan", "updated the plan"),
        ("Update the skill", "updated the skill"),
        ("Update memory", "updated memory"),
        ("Create the folder", "created the folder"),
        ("Check the signature of", "checked the signature of"),
        ("Go to", "went to"),
        ("Ask you", "asked you"),
        ("Ask", "asked"),
        ("Use", "used"),
        ("Start in background:", "started in the background:"),
    ];
    for (from, to) in TWO {
        if let Some(rest) = action.strip_prefix(from) {
            return (to.to_string(), rest.trim().to_string());
        }
    }
    let (first, rest) = action.split_once(' ').unwrap_or((action, ""));
    let verb = match first {
        "Run" => "ran",
        "Show" => "looked at",
        "List" => "listed",
        "Read" => "read",
        "Edit" => "edited",
        "Write" => "wrote",
        "Search" => "searched",
        "Find" => "found",
        "Create" => "created",
        "Move" => "moved",
        "Copy" => "copied",
        "Delete" => "deleted",
        "Make" => "made",
        "Count" => "counted lines in",
        "Compare" => "compared",
        "Fetch" => "fetched",
        "Query" => "queried",
        "Stop" => "stopped",
        "Wait" => "waited",
        "Filter" => "filtered",
        "Check" => "checked",
        "Git" => "ran git",
        _ => return ("ran".into(), action.to_lowercase()),
    };
    let rest = if first == "Count" { rest.trim_start_matches("lines in ") } else { rest };
    (verb.to_string(), rest.trim().to_string())
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
    // Assignments seen so far: `$TD` is the program `TD=…/things-do` names.
    let mut vars: Vec<(String, String)> = vec![];
    for line in command.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(found) = program_in(line, &mut vars) {
            return Some(found);
        }
    }
    None
}

/// The program a line runs (`command_from`), learning its assignments on the way.
fn program_in<'a>(line: &'a str, vars: &mut Vec<(String, String)>) -> Option<(String, &'a str)> {
    const PASS: &[&str] = &[
        "cd", "export", "set", "source", ".", "echo", "printf", "true", "unset", "if", "then",
        "do", "for", "while", "!", "{", "(", "fi", "done", "else",
    ];
    const WRAP: &[&str] = &["time", "sudo", "env", "nohup", "exec", "command", "xargs"];
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
        if let Some((name, value)) = w.split_once('=').filter(|(n, _)| {
            !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')
        }) {
            vars.push((name.to_string(), value.trim_matches(['"', '\'']).to_string()));
            continue;
        }
        if w.is_empty() || (w.contains('=') && !w.starts_with('-')) {
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
        let bare = w.trim_start_matches('$').trim_matches(['{', '}']);
        let name = match w.starts_with('$') {
            // A variable runs what it was set to; one set elsewhere names nothing.
            true => match vars.iter().rev().find(|(n, _)| n == bare) {
                Some((_, v)) => base(v),
                None => continue,
            },
            false => base(bare),
        };
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

/// Words for a person in quotes; a pattern (`^#\\|^###`, `(?i)foo|bar`) or anything else not
/// meant to be read says nothing: the call's card has it whole.
fn quote(text: &str) -> String {
    let human = text.chars().any(char::is_alphabetic)
        && !text.chars().any(|c| "^$\\|[]{}()*+?<>=~`".contains(c));
    if !human {
        return String::new();
    }
    let t = first_line(text, 40);
    if t.is_empty() {
        String::new()
    } else {
        format!("“{t}”")
    }
}

/// "for “words”", or nothing when the words are not for a person (`quote`).
fn for_(text: &str) -> String {
    let q = quote(text);
    if q.is_empty() { q } else { format!("for {q}") }
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
    plain_place(base(path))
}

/// A place said only when it is a plain name: not the home folder, not `.`, not a piece of a
/// command (spaces, quotes, `$`).
fn plain_place(p: String) -> Option<String> {
    let home = std::env::var("HOME").ok().map(|h| base(&h));
    let odd = p.is_empty()
        || p == "."
        || p == "~"
        || home.as_deref() == Some(p.as_str())
        || p.contains(|c: char| c.is_whitespace() || "\\\"'$`*?".contains(c));
    (!odd).then_some(p)
}

/// The folder a path is in, by its name; inside an app bundle, the app.
fn parent(path: &str) -> Option<String> {
    let p = path.trim_end_matches('/');
    if let Some(i) = p.find(".app/") {
        return plain_place(base(&p[..i + 4]));
    }
    let (dir, _) = p.rsplit_once('/')?;
    plain_place(base(dir))
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

    /// `BENCH_SENTENCES=<thread.open json>`: every piece of work of a real thread as sentences,
    /// printed — to read them, not to check them.
    #[test]
    #[ignore]
    fn print_sentences_of_a_real_thread() {
        let file = std::env::var("BENCH_SENTENCES").expect("BENCH_SENTENCES");
        let open: protocol::ThreadOpen =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        let thread = crate::Thread::open(open);
        for row in thread.rows() {
            if let crate::Row::Work { steps, .. } = row {
                let calls = steps.iter().filter(|s| s.is_tool()).count();
                let ch = chapters(&steps);
                let lines: usize = ch.iter().map(|c| c.sentences.len() + usize::from(c.heading.is_some())).sum();
                println!("\n== {calls} calls -> {lines} lines");
                for c in ch {
                    if let Some((_, h)) = c.heading {
                        println!("  ## {h}");
                    }
                    for x in c.sentences {
                        let mark = if x.words { "     ~" } else { "      " };
                        let fail = if x.failed > 0 { format!("  [{} failed]", x.failed) } else { String::new() };
                        println!("{mark} {}{fail}", x.text);
                    }
                }
            }
        }
    }

    #[test]
    fn rule_7_milestones_by_the_agents_words_chips_in_order() {
        let bash = |c: &str| call("bash", json!({ "command": c }));
        let steps = vec![
            bash("ls /a"),
            bash("ls /b"),
            Step::Note("Found the cause.".into()),
            call("edit", json!({"path": "/x/things-do"})),
            call("edit", json!({"path": "/x/things-do"})),
            bash("cat /x/things-do"),
            call("read", json!({"path": "/x/report"})),
        ];
        let m = milestones(&steps);
        assert_eq!(m.len(), 2);
        assert!(!m[0].words);
        assert_eq!((m[1].heading.as_str(), m[1].words), ("Found the cause.", true));
        let labels: Vec<String> = chips(&steps, &m[1].steps).into_iter().map(|c| c.label).collect();
        assert_eq!(labels, vec!["Edit things-do ×2", "Show things-do", "Read report"]);
    }

    #[test]
    fn rule_8_what_a_call_did_and_what_came_back() {
        let bash = |c: &str, out: &str| Step::Tool {
            call_id: String::new(),
            name: "bash".into(),
            args: json!({ "command": c }),
            preview: String::new(),
            output: Some(out.into()),
            failed: false,
            diff: None,
        };
        let a = bash(
            "echo \"=== 1. AppleScript: может ли мой процесс управлять Things ===\"; osascript -e 'tell application \"Things3\" to get name' 2>&1",
            "=== 1. AppleScript: может ли мой процесс управлять Things ===\nThings3\n",
        );
        assert_eq!(
            detail(&a).as_deref(),
            Some("1. AppleScript: может ли мой процесс управлять Things — get name")
        );
        assert_eq!(result_line(&a).as_deref(), Some("Things3"));
        let b = bash(
            "osascript <<'OSA' 2>&1 | head -5\ntell application \"Things3\"\n\tset t to make new to do with properties {name:\"ZZZ проверка\", notes:x}\nend tell\nOSA",
            "116:120: execution error: Переменная todo не определена. (-2753)",
        );
        assert_eq!(detail(&b).as_deref(), Some("make new to do «ZZZ проверка»"));
        assert_eq!(result_line(&b).as_deref(), Some("Переменная todo не определена. (-2753)"));
        let c = bash("python3 - <<'PY'\nsrc='/Users/v/files/task-notes.md'\nprint(1)\nPY", "pkg: /Users/v/.cache/uv/lib/site-packages/things_mcp");
        assert_eq!(detail(&c).as_deref(), Some("task-notes.md"));
        assert_eq!(result_line(&c).as_deref(), Some("pkg: …/things_mcp"));
        assert_eq!(
            beat("Big finding: AppleScript works right now. That means more.").as_deref(),
            Some("Big finding: AppleScript works right now.")
        );
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
