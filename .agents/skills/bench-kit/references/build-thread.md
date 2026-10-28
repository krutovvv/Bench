# The thread: Pi's journal as rows on a list

The core of Bench. Design and history: `docs/thread-rewrite.md`. This page is how it works and
how to change it without breaking what it promises.

## The pipeline

```
Pi's journal (agent)          agent/src/runtime/journal.ts     entries → ThreadItem (user, thought, tool, result, text, insert, fail)
  thread.open {since}         agent/src/modules/threads.ts   items after a head + the queue + the inserts' state
  thread.append/stream/…      from Pi's commits (subscribeCommits), one event per change
        ↓
crates/protocol               the types (ThreadItem, QueuedItem, ThreadEvent); TypeScript generated from them
        ↓
crates/thread                 Thread::open / apply / rows(): the rows — User, Work, Answer, Insert, Fail, Writing, Queued
        ↓                     the rules of order are tests (crates/thread/src/tests.rs)
src/thread/                ThreadView (view.rs), an entity: its ActiveThread, composer, events;
                              its list (list.rs) on gpui::list: sync rows by diff, render each row (mod.rs)
                              message.rs (user, answer), work.rs ("N actions"), queued.rs, extras.rs (tail)
```

What the thread shows and in what order is decided in `crates/thread` and nowhere else; the view
only draws rows. Which words are the agent's answer and which its narration is decided by Pi
itself: an assistant message that also called a tool is narration (`ThreadItem::Text
{narration: true}` — the agent reads it off the message's blocks; mid-stream, Pi's `toolcall_start`
says it before the entry exists, `thread.stream {kind:"narration"}`). Marked words are notes on
the work line wherever they sit; unmarked ones are the answer at once, mid-run included — no
guessing from position, so an answer never flashes and folds back. The live work line opens on its
own while the run goes (its notes, thinking and steps show where they will settle) and folds at the
run's end, unless the person clicked it over. A new kind of row: a `Row` variant + its rule in `rows.rs` + a test + its
renderer in `src/thread/`.

## What the view promises (keep it)

- **Nothing moves the thread but the person.** No following: an answer growing, a run ending, a
  row changing never scroll it. Only the person's own acts do: sending (the composer is brought
  up once, after the new rows are laid out, and only if they pushed it off), "Jump to latest",
  the scrollbar. `View::reveal`, `to_end` in `mod.rs`.
- **Rows keep their place while measured again.** Changed rows are `remeasure_items`d, only the
  count changes by `splice`, at the end (`thread::diff`). Splicing a range resets the list's
  offset inside it — that was the jitter.
- **The look** (`src/thread/look.rs`): `Chat` (the default — no avatars, the person's messages
  as bubbles, the composer pinned under the list, no room past the end) or `Document` (avatars,
  the composer as the list's last row, the room below). One line, `pub type Current`, picks it.
- **The room past the end** (the document's) (an editor's scroll past its last line): exactly as tall as puts the
  composer, scrolled to the end, where it stands in a new thread (`room_past_end`). Recomputed
  each frame and remeasured when it changes.
- **"Jump to latest" goes to the very end** by `scroll_to_end` with the room at zero for one
  frame (the list's reveal sums unmeasured rows as 0 and stops short in a long thread).
- **A row never flashes when its entry arrives.** The answer being written passes its parsed text
  to the answer that replaces it, the live thinking its fold to its work (`carry_over`); texts are
  kept in `ui::media::Texts` (a long one is parsed in the background, blank meanwhile).
- **Never ask the list during `render_item`** (`viewport_bounds`, `bounds_for_item`): the list is
  borrowed — work it out before layout (`sync_rows`).

## Seeing it

- `BENCH_DEMO=landing` (a thread at its end), `top` (at its top), `long` (60 exchanges), `new`.
- `BENCH_DEMO_RUN=1` plays a run (message, thinking, a tool, the answer streamed, its entry, the
  end) in the open demo thread.
- `BENCH_DEMO_TYPE=1` writes into the composer two seconds in; `BENCH_DEMO_SEND=1` with it
  presses Send on the running thread (the queue's echo row must show the same frame).
- `BENCH_DEMO_SCROLL=end` or `<item>:<px>` opens it scrolled; `BENCH_DEMO_JUMP=1` presses "Jump
  to latest"; `BENCH_DEMO_RENAME=<thread id>` opens a thread's rename.
- `BENCH_TRACE_SCROLL=1` logs, every frame, the scroll top, the composer's bottom and every row's
  height: a jerk is a number that changes when nothing should.
- A real run on your own copy: a copy of `~/.bench` without bots, schedule and projects as
  `BENCH_HOME`, its agent from the repository (`node agent/src/server.ts`), driven over its
  WebSocket (`session.create`, `prompt.submit`) — see `build-verification.md`.
