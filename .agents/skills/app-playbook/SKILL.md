---
name: app-playbook
description: "Use to start a new native app from Bench, to bring a sibling app to the family's standard, or to learn how such apps are built and checked. The shared base of the family, for weak agents too."
---

# How the family's apps are built

Bench is the first of a family of native apps in Rust and GPUI, made by one person with agents. Each sibling, Ferry for one,
is built the same way, so an agent that knows one can find its way in all of them, as in an encyclopedia: the same
names, in the same places. A weak local model must be able to work in any of them.

Each sibling sends what it learned back here. Bench's own skills say how Bench does a thing; this one says what all of them share.

## What every app has

| Thing | Where | What it does |
|---|---|---|
| The rules for agents | `AGENTS.md` | The words of the app, the skills to read, Never, the four lines before a fix, signs of a patch, the change loop |
| A map | skill `<app>-map` | "You are asked to X: open Y", and the whole tree. Read first for any change |
| The words | skill `<app>-concepts` | One word, one thing, each with its type in the code |
| Where new things go | skill `<app>-structure` | The tree's rule, three questions before a change, real patches and what replaced them |
| Worked steps | skill `<app>-recipes` or a skill per area | Common changes with code to copy, and names that are only examples said to be so |
| How to know it works | skill or section | The check, how to take a picture of your own copy, how to reach a state without a mouse |
| Why it is so | `SPEC.md`, one per module | A graph with `parent` links, checked by `scripts/specs-check.mjs`. Only what the code cannot say |
| Shared elements | `crates/ui` | Theme, every size, buttons, fields. Knows nothing about the app's subject |
| One check | a script, or the list in `AGENTS.md` | Ends in one clear line. `scripts/tree-check.sh` holds the tree's rules |
| A demo with made-up data | `web/`, `scripts/build-web.sh`, a workflow | The real interface in a browser, full window, at the repository's Pages address |
| Honest face | `README.md` | First line says what it is, second that it is deep alpha, then "Is this for you?" |

## The rules of the code

1. **The tree reads by its names.** In the root of `src/` only `main.rs` and `app.rs`. Folders are named for a place in the
   window, a feature, or a service. "Fix the status bar" must lead to the folder by its name alone.
2. **Small files.** At most about 500 lines, cut by what the parts do. Never `utils`, `helpers`, `misc`. The first line is a
   `//!` comment saying what is here.
3. **One of each.** An element used twice lives in `crates/ui`. One way to save, ask, report a failure, reach the outside.
4. **Sizes have names.** Spacing is a `Gap` step, other sizes constants in `metrics.rs`. No number of pixels in a screen.
5. **State knows whose it is.** An area's state and actions are in its folder. The app object has one field per area.
6. **The demo is a stand-in at a seam,** as `FakeFs` is in Zed. Product code has no `if demo` and almost no `#[cfg(wasm)]`.
7. **Abstractions wait for a second user.**
8. **Words come from Rust and Zed:** workspace, dock, panel, pane, item. No private terms.

## Starting a sibling

1. Copy from Bench: `.cargo/config.toml`, `vendor/gpui-pre-*`, `[patch.crates-io]`, `scripts/specs-check.mjs`,
   `scripts/tree-check.sh`, `web/index.html`, `scripts/build-web.sh`, `.github/workflows/web-demo.yml`.
   Replace the app's name in each; the wasm file's name is the crate's.
2. Write `AGENTS.md` and the root `SPEC.md` first, with the app's words.
3. Make `crates/ui`: it is a workspace member from the start.
4. Give it its own icon, drawn from shapes by a script, and its own name. No other app's name, icon or artwork,
   anywhere: not in the code, the docs, a test or a commit. Look at the first commit before the first push.
5. Check the repository is free of real data before it is ever pushed: the history is published with it.

## Common cases, in every app

| Case | The rule |
|---|---|
| A blank window: icons but no text, right-sized buttons | `gpui_platform` lacks the `font-kit` feature, so a stand-in text system measures and draws nothing. It builds and passes every test. Bench has it through `gpui-base`; an app on bare GPUI must ask for it, and its check should look: `cargo tree -e features -i gpui-pre-macos \| grep 'feature "font-kit"'` |
| Shaders fail on a new Xcode | `runtime_shaders` on `gpui_platform`: Xcode no longer ships the Metal toolchain by default |
| A browser build that needs nightly or server headers | `vendor/gpui-pre-platform` with `default = []`: single-threaded, plain GitHub Pages |
| The demo has no fonts | The browser has no system fonts: add Inter and Noto Sans with `add_fonts`, and name a font in the root style |
| `std::time::Instant` panics in the browser | `web_time::Instant` |
| A test folder for the real database | A data directory the tests and scripts override. Never run the app on real data |
| A port is taken in a preview | Another project's server may hold it. Do not kill it: use another port |
| A shell script with a variable called `path` | In zsh it is tied to `PATH`, and the script stops finding commands. Prove a check fails on a break before trusting it |
| A refactor | Take pictures before and after and compare pixels. Differences allowed: window focus, a caret, clocks, a hover from the real mouse |
| Moving code | Move it with a script, let the compiler list what is missing, let `cargo fix` remove unused imports, then check the browser target too |

## A green check does not see the screen

The worst bug of Ferry's first web port was a window with no text, with every test passing. Anything visible is not done until an
agent has read a picture of it. Turn each such lesson into a line in a check, if a script can see it.

## Testing with a weak agent

Start a fresh agent on the weakest model at hand, with only the repository, read-only, and give it eight requests in plain
words: a size, a menu item, a setting, a stored field, an agent tool, a bug report, a question about hiding something, and a request to
delete the person's data. For each it says what it read, which files and functions it would change, what it would ask first, and
where the documents left it guessing. It must check each name with grep.

Fix what it finds in the documents or the tree, not the agent. What Ferry's first run found:

- an example reads as if its names already exist: say "not in the code";
- a number in the documents disagrees with a list beside it: do not count, or count again;
- a recipe stops at the data and never uses it: end at what the person sees;
- a claim about the code is false in one timing: read the code before writing a rule;
- the map names where a thing acts but not where it is triggered: name both;
- nothing says what to do when asked to delete the person's data: write the rule.

`scripts/tree-check.sh` also fails on a path in a document that does not exist. A stale document misleads a weak model more
than a missing one.

## Sending a lesson back

When an app teaches something any other app could use, add it here, in the words of that case, with no client's name. Add a
line to the app's own skills only if it is the app's alone. If a rule here proved wrong in a sibling, change it here and say why.

| App | What it brought |
|---|---|
| Ferry | The rules as a script (`scripts/tree-check.sh`), one check script, recipes with code, the weak agent test, the blank window |
