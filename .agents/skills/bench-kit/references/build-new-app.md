# A Bench for another person: a copy, then their tools

There is one app: Bench. Another person's Bench is a copy of this one — the same agent client,
the same look, the same icon, the same agent (`agent/`) — with the tools they don't use hidden
and theirs added as threads. Don't propose another stack, theme, layout or a separate app; don't
ask about them.

## 1. Copy it

```bash
~/Projects/bench-kit/.agents/skills/bench-kit/scripts/new-bench.sh <crate-name>   # e.g. anna-bench
cd ~/Projects/<crate-name> && cargo build && ./target/debug/<crate-name>
```

- `crate-name`: lowercase-with-dashes; it is also the binary, the bundle id `local.<crate>`
  (stable — macOS privacy grants follow it) and the state folders. The window and the Dock keep
  the name "Bench" and Bench's icon.
- It copies the committed files of `~/Projects/bench-kit` (`git ls-files`, the kit included),
  so a new Bench starts from the latest commit; lands in `~/Projects/<crate>` with `git init`
  done; refuses an existing folder.
- Builds go to one shared folder for all copies (`~/Library/Caches/gpui-apps/target`, set in
  the project's `.cargo/config.toml`; `./target` links to it). gpui compiles once: the first
  build takes minutes, the next copies' seconds.
- Build and run it untouched before changing anything. If that fails, fix it first — your own
  code is not the problem yet.
- The agent comes with it: Bench installs and starts `agent/` in `~/.bench` on first run
  (Node.js 22.18+), or on a server over SSH (`build-pi-backend.md`).

## 2. Hide what they don't use

`src/bench.rs` has one flag per built-in tool. `false` hides its sidebar group and shortcut; the
code stays and still builds, so bringing it back is one word:

```rust
pub const SSH: bool = true;    // Servers: ~/.ssh/config hosts and this Mac's terminal as threads (⌘2)
pub const IDE: bool = true;    // the file tree and editor beside a thread (⌘B, ⌘P)
pub const TOOLS: bool = true;  // Tools: everyday utilities, one thread per tool (⌘3)
pub const TASKS: bool = true;  // Tasks: the lists as threads, the agent's tasks tool (⌘4)
```

From the interview (`start-interview.md`): no servers → `SSH = false`; none of the 19 tools fits
→ `TOOLS = false` (when some fit, remove the others from `src/devtools/`'s `GROUPS`); tasks live
elsewhere and they want them there → `TASKS = false`, or move them in (`build-native-data.md`).
Files shows only when there are hosts. Browser works only with a server browser and a key
(`~/.bench/ui/browser.json`, `build-pi-backend.md`); without one, its threads say so.

## 3. Add theirs

Each thing they work on that the five don't cover becomes a new kind of thread:
`build-insert.md`. One at a time, through the change loop (`build-change-loop.md`).

## 4. Build, check, ship

```bash
cargo build && ./target/debug/<crate>      # dev run
scripts/bundle-macos.sh                    # dist/Bench.<commit>.app — runs the release build itself
```

`bundle-macos.sh` builds release on its own: don't run `cargo build --release` before it. In the
user's own Bench use `scripts/ship.sh` instead (one app per commit, opened next to the running
one). Then `build-workflow.md`: zero warnings, `cargo fmt`, verify by measuring your own window,
commit, report honestly what was checked.
