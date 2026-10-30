---
id: thread-view
type: submodule-design
status: draft
title: src/thread — a thread on screen
parent: app
depends-on: [thread-core, ui, inserts]
---

# A thread on screen

Draws one thread — the rows `thread-core` gives, the cards that hang around them, the composer —
and sends what the person does back to the thread's agent. It also keeps every agent's threads
(the sidebar's lists, drafts, journals seen in this run: `store.rs`) and opens, creates and
deletes threads (`open.rs`).

## Boundary

- **Order is not decided here.** Which rows exist and in what order is `thread-core`'s; this
  module draws a `Row` and never reorders, merges or hides rows itself.
- **The thread is its own entity**, `ThreadView` (as Zed's `AcpThreadView`): its state
  (`ActiveThread`), its composer, its run, its queue, its events. The center shows one; the right
  dock a second beside it (`beside.rs`, kept in code, not offered), fully independent.
- **Two rules keep the view and the app from being borrowed twice.** What a view asks of the
  app names the thread with a `ThreadRef` and never reads a view (the view asking may be drawing
  at that moment). What the app asks of a view never reads the app; when a view's click needs
  the app to come back to it, the call is deferred.
- A thread's meters (context, speed, fallback) are the thread's, shown under its composer with
  its project and repo (`composer/foot.rs`) — not the window's.

## Invariants

- **Nothing moves the list but the person.** A run that ends, a row that grows, an answer that
  arrives never scroll the thread, as code in an editor does not scroll while it changes. The one
  exceptions: after the person sends, the composer is brought back into view if the new rows
  pushed it past the bottom edge; and a thread the person opens lands at its end (its last
  message and the composer in view) and stays there while its rows find their heights, until its
  height holds still for a second (ten at most); the wheel takes it back at once.
- Rows reach the list by diff (`thread::diff`): unchanged rows keep their measured heights, only
  the count is spliced at the end, so the reader keeps their place even inside a row being
  written. Another thread opens at its end.
- `gpui::list` lays out only what is on screen; a thread of thousands of rows costs what a
  screenful costs.
- Only the thread scrolls (with a Mac overlay scrollbar); a block inside it does not scroll on
  its own, except a terminal, an editor, or a tool pane with its own wheel. Where a block does
  have its wheel, a `ScrollableMask` over it is the wheel's owner, and the thread behind it
  holds still.

## Decisions

- **The look is one type**, chosen in `look.rs`: `Chat` (the default — the person's grey bubbles,
  no avatars, the composer pinned under the list) or `Document` (avatars in a gutter, the
  composer as the last row, an editor's room past the end). Switching is one line; nothing else
  branches on it.
- **A click on the thread itself gives the composer the focus; what handles a click stops it first.**
  `ThreadView::press` holds the press from mouse down to an unmoved mouse up, and acts only when the
  focus did not move. A fold, a button or an insert's terminal stops its own propagation
  (`bench_ui::own_press`), so it never gets here.
- **The composer sends on Enter** (Shift+Enter or ⌘Enter a new line) and grows to
  `COMPOSER_MAX_LINES`, then scrolls inside.
- Each kind of row is a file in `rows/`; the composer's parts are files in `composer/`. A new kind
  of agent message is a new row file (the `BlockKind` extension point, with an unknown kind drawn
  as text, is the intent of `architecture` §6, not yet cut out — unconfirmed how unknown kinds
  are drawn today).
- Messages carry code, problems and terminal output as references (chips) that become fenced
  blocks with `path:lines` under the message's words — what the agent reads is what the thread
  shows (`explorer`).
- **The person works in the chat; the Explorer is where they go when they choose to.** What a
  turn changed shows under its answer as one folded line ("Changed files · N files +a −d",
  `rows/changed_files.rs`, from `Row::Changes`); a file's diff opens in place under its row, and
  only "Open in Code" on its hover takes it to a tab. Anything new the thread shows starts as such
  a quiet line and unfolds on a click: no panels, no badges that ask for attention.
- A review comment is a reference too (`Reference::Comment`, ⌘⇧L in the Explorer): `Comment <id>
  on \`path:lines\`: <note>` with the lines fenced; the agent closes each with `resolve_comment`.
- **A turn is reviewed when the person asks**, from "Review" at the far edge of its changes' head:
  one call to the thread's model reads what was asked, the plan, the answer and the turn's diff
  (`agent/src/runtime/review.ts`); its findings fold under the files and go to the agent only
  through "Send to the agent", as review comments. Nothing reaches the agent unasked.

## Что живёт в треде, и откуда оно берётся

Тред показывает журнал Pi (`src/thread/SPEC.md`). Кроме сообщений и работы агента в нём
живут план, вопросы агента, вставки, пауза, очередь и фоновые процессы. Каждый раз, когда
что-то из этого бралось из второго источника — своя таблица, своё событие, отдельная запись
«для Bench», — оно начинало прыгать: вставало под очередью, не обновлялось, терялось после
перезапуска. Поэтому одно правило на всё.

### Правило

**Место — запись журнала. Что сейчас — то, что Pi уже хранит. Своё — только документ Pi.**

1. Место в треде даёт запись журнала Pi, и только она. Ничего не ставится по времени, по
   «концу треда» или по счёту сообщений. Порядок записей — порядок коммитов Pi.
2. Если факт уже есть в журнале (вызов инструмента, его аргументы, его результат) — Bench
   читает его оттуда. Вторую запись «о том же» агент не пишет.
3. Состояние, которого в журнале нет и которое должно пережить перезапуск, — документ беседы
   Pi (`defineDoc`), меняется в коммитах. Его семантика объявлена: что видит форк.
4. Живое состояние прогона, которое Pi строит заново после перезапуска (кто ждёт ответа, что
   сейчас пишется), агент держит в памяти и отдаёт в `thread.open` и событиями. Оно не ставит
   строк — только оживляет те, что уже стоят.
5. Ни SQLite агента, ни память агента не хранят того, что видно в треде.

### Что откуда

| Что | Место | Что сейчас | Пережить перезапуск |
|---|---|---|---|
| Сообщение, работа, ответ | `pi.user`, `pi.assistant`, `pi.tool-result` | сама запись | журнал |
| План | первый вызов `todo` в этой работе | аргументы последнего вызова `todo` в ней | журнал |
| Вопрос (`clarify`) | вызов `clarify` | результат вызова: ответ или отказ | журнал; ждущий вызов Pi запускает заново (`replay: "safe"`) |
| Разрешение команды | вызов, который ждёт разрешения | результат вызова (отказ — `block`) | хук `beforeTool` спрашивает заново |
| Кто ждёт ответа человека | — (оживляет вопрос или вызов) | список ждущих у агента, id = id вызова | строится заново при повторе вызова |
| Вставка | `bench.insert {id}` | документ `bench.inserts` | документ |
| Пауза | — (её нет в треде, это состояние прогона) | документ `bench.hold`: отрезки «с — по» | документ; гейт перед вызовом читает его |
| Очередь | после журнала и живой работы | `InboxDoc` Pi | Pi |
| Сообщение, нажатое до ответа агента | строка очереди | сверяется по `requestId`, который дал Bench | Pi (`submissionByRequest`) |
| Фоновый процесс | вызов `process start`, который его запустил | фоновая задача беседы `bench.process`: хвост вывода в её чекпойнте, конец — её исход | запись задачи; сама команда — нет: после перезапуска «остановлен перезапуском» (`abandonOnRestart`) |

### Как это читать

- **План** — это не отдельная вещь, а то, что агент написал инструментом `todo`. Карточка стоит
  на первом вызове `todo` в своей работе и показывает аргументы последнего вызова в ней же.
  Новая работа (после следующего сообщения) — новая карточка; старая остаётся такой, какой
  была в конце своей работы. Ни «плана сессии», ни угадывания «новый план или тот же».
- **Вопрос** — вызов `clarify`. Пока у него нет результата и агент его ждёт, карточка
  принимает ответ. Результат вызова — это и есть ответ: журнал хранит его сам.
- **Пауза** решается человеком и должна пережить перезапуск агента — поэтому документ. Гейт
  перед каждым вызовом инструмента читает документ; Pi вызывает `beforeTool` до записи
  намерения, поэтому после перезапуска прогон снова встаёт на паузу. Часы работы вычитают
  отрезки паузы.
- **Фоновый процесс** — фоновая задача беседы Pi (`builtin/process.ts`): «Стоп» прогона и
  конец прогона её не трогают, останавливает только её собственный Stop. Вывод она раз в
  секунду кладёт в свой чекпойнт, выход — в исход; Bench получает запись задачи из коммитов
  Pi (`thread.process`) и ничего не опрашивает. Перезапуск агента убивает команду вместе с
  ним, и Pi помечает задачу причиной `restart` — заново она не запускается.
- **Форк и откат** — `conversation.fork(at)` Pi. Всё, что выводится из журнала, форкается само.
  Документ объявляет, что видит форк: пауза — `fork: "initial"` (новый тред не на паузе),
  вставки — состояние на момент форка.

### Правила порядка (тесты в `crates/thread/src/tests.rs`)

1. Строка стоит на своей записи журнала; обновление её состояния не двигает ни её, ни соседей.
2. План и вопрос стоят внутри работы, которая их написала, даже когда после неё пришли
   сообщения и ответы.
3. Очередь всегда после журнала и живой работы.
4. Нажатое сообщение показывается сразу строкой очереди и становится строкой Pi с тем же
   `requestId` — никогда не двумя строками, даже если два сообщения одинаковы.
5. Пауза не ставит строк: она останавливает часы и оживление живой работы.
6. Фоновый процесс стоит в работе, чей вызов его запустил; его конец меняет карточку на
   месте и ничего не двигает.
