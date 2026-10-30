# Bench

Bench is a native Mac app for working with an AI agent: Rust and GPUI (gpui-kit 0.8), with its own
agent in `agent/`, a Node server on Pi Durable. The person writes in Russian: answer in Russian.

The basics:

- A **thread** is one conversation with the agent: messages, the agent's steps, and **inserts**, live
  tools the person puts in with `/`.
- A **project** is the sidebar's unit. It belongs to an **agent** (this Mac's or a server's) and says
  where its work is. **Host** is an agent's own environment.
- The **Explorer** shows a project's files as an IDE does, beside its threads.

Everything else you need is in skills. Read one with `skill_view` before you need it:

- `bench-map`: which file to open for a change. Read this first for any change.
- `bench-concepts`: the exact meaning of each word above.
- `bench-kit`: UI and insert work, and how to look at a change.
- `bench-structure`: where a new feature, module or file goes, and how to refactor.
- `pi-durable-dev-rules`: anything in the agent that touches Pi Durable, and how Bench maps onto it.
- `gpui-kit`, `rust-best-practices`: the framework and Rust, once the above have no answer.

Why a module is built as it is: its `SPEC.md`, beside its code. The first time you touch a file in a
module, a note names its spec: read it then. A change that moves a decision updates the spec in the
same commit.

## Before you fix or add anything

The first idea is usually a patch at the place where the problem shows. Look wider first. Write
these four lines in your message before the first edit, and stop reading once they are filled.
Skip them only when the person named the exact change.

1. SEEN: the command you ran or the picture you took that shows the problem.
2. CAUSE: the file and line where the wrong state first appears, not where it shows on screen.
3. AROUND: what you read around it and what you searched with grep. Does the same problem exist in
   another place? Does a sibling already solve it? Say how many places.
4. ALREADY THERE: what you searched in the framework for this mechanism and what you found. Use
   it, or say in one line why it does not fit.
   - UI: `crates/ui`, then `vendor/gpui-base` and
     `~/.cargo/registry/src/index.crates.io-*/gpui-component-0.8.0/src`. Zed is the model for names
     and organisation.
   - Agent: Pi's docs in `.agents/skills/pi-durable-dev-rules/references/`, its sources in
     `agent/node_modules/@earendil-works/`.

If AROUND found more than one place, the fix goes in one shared place. If the general fix is much
bigger than the local one, say both and ask the person.

A real example:

    SEEN: BENCH_DEMO_DRAG picture: a tab carried to the right shows its line on the left of the
    tab under it, and lands on the right of it.
    CAUSE: src/explorer/pane/view.rs: drag_over always draws border_l_2.
    AROUND: two lists drag, tabs and projects. Projects take the side from the pointer, tabs from
    where the tab came from. Both are right for their list, and only the tabs' line is wrong.
    ALREADY THERE: GPUI's drag_over hands over the dragged value, so the tab can compare places,
    as Zed's tabs do.

## Signs of a patch

If your change has one of these, stop and look at the cause again, or say in one line why it is
right here:

- a flag passed through layers to undo an effect, or a special case for one observed bug;
- a timer, sleep, retry or polling where something could say it changed;
- a copy of code that exists, where it should become one shared thing;
- a negative margin, a `px(…)` or a bare `gap_2` in a screen (sizes are `bench_ui` constants and
  `Gap` steps, a missing piece goes to `crates/ui` first);
- `if demo` in product code, or a side effect in `render`.

Two fixes on one assumption failed: write the assumption down and test it before a third. Forty
tool calls without a running result: stop and say what is hard.

Pi Durable is used as its authors intend and is never patched. What it lacks is worked around in
our code, with a comment saying so.

## The change loop

1. Change, `cargo build`, then look at your own copy:
   `.agents/skills/bench-kit/scripts/shot.sh target/debug/bench /tmp/after.png 10 BENCH_DEMO=landing`.
   Read the picture. Anything the person sees is not done until you have looked at it.
2. Run the checks: `cargo build` with zero warnings, `cargo test --workspace`, and for the agent
   `cd agent && node --test --test-timeout=60000 test/*.test.ts`.
3. One change, one commit.
4. `scripts/bundle-macos.sh` rebuilds `/Applications/Bench.app`, at once, with no need to ask.
5. Tell the person: "собрано — перезапусти Bench".

## Never

- Never quit, kill or restart Bench: you run inside it. Never rebuild the app to see a change.
- Never take a picture of the person's window: only your own copy.
- Never push without asking. The repository is public: no private names, tokens or the person's
  data in commits.
- Never delete the person's data without a backup in `~/.bench/backups/`.
- Never fill the disk: it is tight. `cargo clean --profile dev` before `target/` fills, and no
  second target directory.
- Never redesign the look: it is final, a Mac app's
  (`.agents/skills/bench-kit/references/design-look-rules.md`). A refactor keeps pixels identical.

Before the first edit of a fix or a feature, the four lines: SEEN, CAUSE, AROUND, ALREADY THERE.
