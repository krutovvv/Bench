#!/usr/bin/env python3
"""Can a weak model work with this repository? Runs real tasks on any OpenAI-compatible server and scores them.

    AGENT_CHECK_URL=http://host:port/v1 AGENT_CHECK_KEY=... AGENT_CHECK_MODEL=name \
        scripts/agent-check/run.py [--tree DIR] [--only code|mcp] [--out result.json]

`change` tasks: the model edits a throwaway copy of the code until it builds, and a shell check judges the result.
`code` tasks: the model finds where a change goes, with only ls, read and grep, starting from AGENTS.md.
A task passes when the files it names really hold the symbols that change needs.
`mcp` tasks: the model does a job through the MCP server on a throwaway copy of the made-up database.
A task passes when an SQL check on that copy holds. Nothing here touches real data.
The tasks are in tasks.json. `mcp` tasks need the debug binary and target/e2e/demo.sqlite.
"""
import argparse, json, os, pathlib, re, shutil, subprocess, sqlite3, tempfile, time, urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
READ_LIMIT = 400  # Lines one read returns: what a small context can afford.
SKIP = {".git", "target", "vendor", "web", "backups", "node_modules", ".DS_Store", "assets"}


def chat(messages, tools):
    body = {"model": os.environ["AGENT_CHECK_MODEL"], "messages": messages, "tools": tools,
            "temperature": 0.2, "max_tokens": 2048}
    request = urllib.request.Request(
        os.environ["AGENT_CHECK_URL"].rstrip("/") + "/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer " + os.environ.get("AGENT_CHECK_KEY", "")},
    )
    with urllib.request.urlopen(request, timeout=300) as answer:
        return json.load(answer)


def function(name, description, properties, required=()):
    return {"type": "function", "function": {"name": name, "description": description, "parameters": {
        "type": "object", "properties": properties, "required": list(required)}}}


def loop(system, task, tools, call, max_steps, nudge=None):
    """Runs one task until the model stops calling tools. Returns the steps and tokens it took.
    `nudge` is said once if it stops early: a model that answers in prose still gets to file its answer."""
    messages = [{"role": "system", "content": system}, {"role": "user", "content": task}]
    steps, tokens, trace = 0, 0, []
    while steps < max_steps:
        reply = chat(messages, tools)
        tokens += reply.get("usage", {}).get("total_tokens", 0)
        message = reply["choices"][0]["message"]
        calls = message.get("tool_calls") or []
        clean = {"role": "assistant", "content": message.get("content") or ""}
        if calls:
            clean["tool_calls"] = calls
        messages.append(clean)
        if not calls and nudge:
            messages.append({"role": "user", "content": nudge})
            nudge = None
            continue
        if not calls:
            return {"steps": steps, "tokens": tokens, "trace": trace, "final": message.get("content") or ""}
        for one in calls:
            steps += 1
            try:
                arguments = json.loads(one["function"]["arguments"] or "{}")
                result = call(one["function"]["name"], arguments)
            except Exception as error:  # The model made a bad call: it reads why and corrects itself.
                result = f"Error: {error}"
            if result is None:
                return {"steps": steps, "tokens": tokens, "trace": trace, "final": arguments}
            trace.append({"call": one["function"]["name"], "arguments": one["function"]["arguments"], "result": str(result)[:300]})
            messages.append({"role": "tool", "tool_call_id": one["id"], "content": str(result)[:30000]})
    if nudge:  # Out of steps: what it knows by now still counts, as it would for a person asked to stop.
        messages.append({"role": "user", "content": nudge})
        reply = chat(messages, [t for t in tools if t["function"]["name"] == "answer"])
        tokens += reply.get("usage", {}).get("total_tokens", 0)
        for one in reply["choices"][0]["message"].get("tool_calls") or []:
            return {"steps": steps, "tokens": tokens, "trace": trace, "final": json.loads(one["function"]["arguments"] or "{}"),
                    "out_of_steps": True}
    return {"steps": steps, "tokens": tokens, "trace": trace, "final": None, "out_of_steps": True}


# ---- code tasks ----

CODE_SYSTEM = """You are a coding agent in a repository. The current folder is its root. You cannot edit yet: \
find where a change belongs. Read AGENTS.md first and follow what it points to. Paths are relative to the root. \
When you know, call `answer` with every file that must change and one line on what to do in each. \
Name only files you opened and saw the right code in."""

CODE_TOOLS = [
    function("ls", "List a folder.", {"path": {"type": "string"}}, ["path"]),
    function("read", f"Read a file, at most {READ_LIMIT} lines from `offset` (1-based), with line numbers.",
             {"path": {"type": "string"}, "offset": {"type": "integer"}}, ["path"]),
    function("grep", "Search files under a folder with a regular expression. Returns path:line:text, at most 40.",
             {"pattern": {"type": "string"}, "path": {"type": "string"}}, ["pattern"]),
    function("answer", "Finish the task.",
             {"files": {"type": "array", "items": {"type": "string"}}, "plan": {"type": "string"}}, ["files", "plan"]),
]


def inside(tree, path):
    target = (tree / path).resolve()
    if tree.resolve() not in [target, *target.parents]:
        raise ValueError("outside the repository")
    return target


def code_tool(tree):
    def call(name, args):
        if name == "answer":
            return None
        if name == "ls":
            target = inside(tree, args.get("path") or ".")
            return "\n".join(sorted(e.name + ("/" if e.is_dir() else "") for e in target.iterdir() if e.name not in SKIP))
        if name == "read":
            lines = inside(tree, args["path"]).read_text().split("\n")
            start = max(int(args.get("offset") or 1), 1) - 1
            part = lines[start:start + READ_LIMIT]
            text = "\n".join(f"{start + n + 1}\t{line}" for n, line in enumerate(part))
            if start + READ_LIMIT < len(lines):
                text += f"\n... {len(lines)} lines in all: read on with offset {start + READ_LIMIT + 1}"
            return text
        if name == "grep":
            pattern = re.compile(args["pattern"])
            found = []
            base = inside(tree, args.get("path") or ".")
            for path in ([base] if base.is_file() else sorted(base.rglob("*"))):
                if not path.is_file() or SKIP & set(path.relative_to(tree).parts):
                    continue
                try:
                    for n, line in enumerate(path.read_text().split("\n"), 1):
                        if pattern.search(line):
                            found.append(f"{path.relative_to(tree)}:{n}:{line.strip()[:160]}")
                except UnicodeDecodeError:
                    continue
            return "\n".join(found[:40]) or "nothing found"
        raise ValueError(f"no tool {name}")
    return call


def run_code(task, tree):
    outcome = loop(CODE_SYSTEM, task["ask"], CODE_TOOLS, code_tool(tree), 25, "Call `answer` now with the files.")
    answer = outcome["final"] if isinstance(outcome["final"], dict) else {}
    texts = []
    files = answer.get("files", [])
    if isinstance(files, str):  # Weak models often send a list as JSON text. It is still their answer.
        try:
            files = json.loads(files)
        except ValueError:
            files = [files]
    for name in files:
        try:
            texts.append(inside(tree, name).read_text())
        except (OSError, ValueError):
            pass
    missing = [group for group in task["needs"] if not any(any(symbol in text for symbol in group) for text in texts)]
    return {**outcome, "final": files, "passed": bool(answer) and not missing, "missing": missing}


# ---- change tasks ----

CHANGE_SYSTEM = """You are a coding agent in a repository. The current folder is a copy of it: change it freely. Read AGENTS.md first \
and follow what it points to. Use `edit` to change a file and `build` to compile. When `build` passes and the change is complete, \
stop calling tools and say what you did."""

CHANGE_TOOLS = CODE_TOOLS[:3] + [
    function("edit", "Replace an exact, unique piece of a file. An empty `old` creates the file with `new`.",
             {"path": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}, ["path", "old", "new"]),
    function("build", "cargo test: builds and runs the tests. Returns the first errors and failures, or `ok`.", {}),
]


def change_tool(tree, env):
    read = code_tool(tree)

    def call(name, args):
        if name == "edit":
            path = inside(tree, args["path"])
            if not args["old"]:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(args["new"])
                return "created"
            text = path.read_text()
            if text.count(args["old"]) != 1:
                raise ValueError(f"`old` appears {text.count(args['old'])} times, it must appear once")
            path.write_text(text.replace(args["old"], args["new"]))
            return "edited"
        if name == "build":
            done = subprocess.run(["cargo", "test", "--message-format=short"], cwd=tree, env=env,
                                  capture_output=True, text=True)
            lines = [line for line in (done.stderr + done.stdout).split("\n") if "error" in line or "FAILED" in line or "panicked" in line]
            return "ok" if done.returncode == 0 else "\n".join(lines[:15])
        return read(name, args)
    return call


def run_change(task, tree):
    with tempfile.TemporaryDirectory() as folder:
        copy = pathlib.Path(folder).resolve() / "repo"
        subprocess.run(f"mkdir -p {copy} && git -C {tree} archive HEAD | tar x -C {copy}", shell=True, check=True)
        # The repository's own target folder: the disk is tight and dependencies need no second build.
        env = {**os.environ, "CARGO_TARGET_DIR": str(ROOT / "target")}
        outcome = loop(CHANGE_SYSTEM, task["ask"], CHANGE_TOOLS, change_tool(copy, env), 40)
        check = subprocess.run(task["check"], shell=True, cwd=copy, env=env, capture_output=True, text=True)
        return {**outcome, "final": str(outcome["final"])[:300], "passed": check.returncode == 0,
                "check": (check.stdout + check.stderr)[-800:]}


# ---- mcp tasks ----

class Server:
    """The app's real MCP server on a throwaway database."""

    def __init__(self, binary, db):
        self.process = subprocess.Popen([binary, "--mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        text=True, env={**os.environ, "DOINGS_DB": str(db)})
        self.id = 0
        init = self.send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                        "clientInfo": {"name": "agent-check", "version": "1"}})
        self.instructions = init.get("instructions", "")
        self.tools = [function(t["name"], t["description"], t["inputSchema"].get("properties", {}),
                               t["inputSchema"].get("required", ())) for t in self.send("tools/list", {})["tools"]]

    def send(self, method, params):
        self.id += 1
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n")
        self.process.stdin.flush()
        return json.loads(self.process.stdout.readline())["result"]

    def call(self, name, args):
        result = self.send("tools/call", {"name": name, "arguments": args})
        return "\n".join(block.get("text", "[picture]") for block in result["content"])

    def close(self):
        self.process.stdin.close()
        self.process.wait()


def run_mcp(task, binary, demo):
    with tempfile.TemporaryDirectory() as folder:
        db = pathlib.Path(folder) / "check.sqlite"
        source = sqlite3.connect(demo)
        target = sqlite3.connect(db)
        source.backup(target)
        target.close()
        source.close()
        server = Server(binary, db)
        system = "You manage the user's tasks through tools. Today's date is in get_overview. " + server.instructions
        outcome = loop(system, task["ask"], server.tools, server.call, 15)
        server.close()
        check = sqlite3.connect(db)
        passed = bool(check.execute(task["check"]).fetchone()[0])
        check.close()
        return {**outcome, "final": str(outcome["final"])[:300], "passed": passed}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=str(ROOT), help="The repository the code tasks read.")
    parser.add_argument("--only", choices=["code", "mcp", "change"])
    parser.add_argument("--binary", default=str(ROOT / "target/debug" / ROOT.name.lower()), help="The app the mcp tasks start.")
    parser.add_argument("--name", action="append", help="Run only this task. May be repeated.")
    parser.add_argument("--out")
    options = parser.parse_args()
    tree = pathlib.Path(options.tree).resolve()
    tasks = json.loads((ROOT / "scripts/agent-check/tasks.json").read_text())
    binary = options.binary
    demo = ROOT / "target/e2e/demo.sqlite"
    results = []
    for kind in ("code", "mcp", "change"):
        if options.only not in (None, kind):
            continue
        for task in tasks.get(kind, []):
            if options.name and task["name"] not in options.name:
                continue
            started = time.time()
            try:
                result = {"code": lambda: run_code(task, tree), "mcp": lambda: run_mcp(task, binary, demo),
                          "change": lambda: run_change(task, tree)}[kind]()
            except Exception as error:  # A server hiccup fails this task, not the whole run.
                result = {"passed": False, "steps": 0, "tokens": 0, "final": f"harness: {error}"}
            result.update(kind=kind, name=task["name"], seconds=round(time.time() - started))
            results.append(result)
            mark = "ok  " if result["passed"] else "FAIL"
            print(f"{mark} {kind}/{task['name']}: {result['steps']} steps, {result['tokens']} tokens, "
                  f"{result['seconds']} s -> {str(result['final'])[:150]}", flush=True)
    passed = sum(r["passed"] for r in results)
    print(f"\n{passed}/{len(results)} passed, {sum(r['steps'] for r in results)} steps, "
          f"{sum(r['tokens'] for r in results)} tokens")
    if options.out:
        pathlib.Path(options.out).write_text(json.dumps(results, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
