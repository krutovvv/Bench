# How the owner wants Bench built

Written down on 2026-10-30 from the owner's own words, for any agent that picks the work up. These
hold over any habit of your own. Read with AGENTS.md and `.agents/skills/bench-structure/SKILL.md`.

## The base frameworks first

- **The agent is built on Pi Durable as its authors intend.** Before writing a mechanism, check
  whether Pi Durable, pi-ai, pi-mcp or code mode already has it (docs vendored in
  `.agents/skills/pi-durable-dev-rules/references/`; sources in `agent/node_modules/@earendil-works/`;
  live docs at pi.dev/docs/latest). Use theirs. Never patch Pi; a workaround for something Pi
  lacks says so in a comment.
- **Hooks are Pi's sanctioned extension point.** Our evaluator points in the agent loop
  (`agent/src/judge/`) stay: they are real gains. Pi's own guide shows `onYield` sending the agent
  back — exactly our use.
- **The same holds for the UI:** GPUI, gpui-kit and Zed's patterns before anything custom (Zed is
  the reference for organisation and names: workspace, docks, panels, items, `FakeFs`).

## Code mode, MCP, the evaluator (decided 2026-10-29)

- **Code mode is not a "mode for code".** It is Pi's way to do many steps in one: a QuickJS script
  calls the thread's tools, filters, counts, and only what it prints reaches the model. Every thread
  has it, always; there is no setting.
- **MCP follows Pi's `exposure`:** `codemode` by default (the tools are the scripts', found with
  `searchTools()`, a CallToolResult whole; not offered to the model, not listed), `direct`
  (offered to the model too), `deferred` (`tool_search`), `hidden`; `toolExposure` per tool. The
  owner moves old skills that expect direct tools one by one by setting `"exposure": "direct"` on
  that server — a legitimate per-server choice, not a hack. The `/browser` insert's server is
  `direct` by default (its tools are the insert's agent tool). Why it matters: the owner runs local
  models; every token of tool declarations in the prompt slows each request.
- **The evaluator** (a System One classifier, Jev's API; the owner's own open-source server is as
  good) is pi-ai's classifier model `bench/evaluator`: the gear lists servers, one main, one
  fallback (Make main / Make fallback, as the main model has). No global switch: no server, no
  evaluator. A project's switch decides use; **on in a new project**. Our hook points and code mode
  scripts (`models.classify()`) ask the same model. It is near free and answers in ~160 ms:
  call it at any potential benefit; avoid only duplicate questions. It must stay optional: the
  agent works the classic way without it, and both ways are tested.

## Structure

- **Modules are self-contained** and live in folders and files whose names tell a reader — a
  person or an agent — where the logic is from the file tree alone.
- **See the patterns and share them.** One thing written twice becomes one implementation. A family
  of similar features gets one scaffold: a widget has a frame, a head and a body, so there is one
  container and each widget brings its content; changing the container changes them all (the
  owner's example: an answer's code block and the code inside an action card are one widget).
- **Anticipate where the app grows.** When adding a feature, ask what else of its kind will come
  and lay the common ground (a registry, a container, a trait) from the first lines.
- **Symmetric, logical code.** Siblings are built alike (inserts, env sections, gear tabs).
- **Causes, not symptoms.** No flags threaded through layers to undo an effect, no special cases
  for one observed bug, no negative margins to cancel a padding, no `if self.demo` in product code
  (the demo is a fake backend, as Zed's `FakeFs`), no app-wide "click claimed" flag where GPUI has
  `stop_propagation`, no pixel values or bare spacing outside `crates/ui`.
- **The look is final.** Mac-like, pixel-tuned (AGENTS.md "The look"). Refactors keep pixels
  identical; do not redesign. If a mechanical cleanup shifts something slightly, the owner accepts
  fixing it later — but check with pictures.

## Documents

- Specs (`SPEC.md` beside each module, a tree by `parent`), docs and skills must match the code.
  Each fact lives in one place; others link to it. A change that moves a decision updates its spec
  in the same commit.
