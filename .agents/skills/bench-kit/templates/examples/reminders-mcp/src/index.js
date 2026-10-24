#!/usr/bin/env node
// reminders-mcp: Apple Reminders as the task list, for Hermes agents. Tasks are reminders in
// iCloud (the Reminders app, iPhone, Calendar; Bench shows them like Things). Tags, "This
// Evening", Someday and the time spent are plain lines at the end of the notes (src/notes.js);
// "important" is Reminders' high priority; a task's day is the reminder's date (no time, and
// no alarm is ever added, so nothing rings on the phone).
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import { helper } from "./helper.js";
import { parseDay, parseTags, readNotes, showSpent, writeNotes, ymd } from "./notes.js";

const server = new McpServer({ name: "reminders-mcp", version: "1.0.0" });

const READ = { readOnlyHint: true, openWorldHint: false };
const WRITE = { readOnlyHint: false, openWorldHint: false };
const DESTRUCTIVE = { readOnlyHint: false, destructiveHint: true, openWorldHint: false };

/** A reminder (helper row) → a task. */
function task(r) {
  const n = readNotes(r.notes);
  return {
    id: r.id,
    title: r.title,
    notes: n.notes,
    tags: n.tags,
    day: r.due ?? null,
    evening: n.evening,
    someday: n.someday,
    important: r.priority >= 1 && r.priority <= 4,
    completed: r.completed ?? null,
    spent_minutes: n.spentMinutes,
    ...(n.timer ? { timer_since: new Date(n.timer * 1000).toISOString() } : {}),
    ...(r.url ? { url: r.url } : {}),
    created: r.created ?? null,
    _raw: r,
  };
}

const VIEWS = ["today", "evening", "upcoming", "anytime", "someday"];

function inView(t, view, today) {
  switch (view) {
    case "today": return t.day && t.day <= today;
    case "evening": return t.day && t.day <= today && t.evening;
    case "upcoming": return t.day && t.day > today;
    case "anytime": return !t.someday && (!t.day || t.day <= today);
    case "someday": return t.someday && !t.day;
    default: return true;
  }
}

function clean(t) {
  const { _raw, ...rest } = t;
  return rest;
}

function text(v) {
  return { content: [{ type: "text", text: typeof v === "string" ? v : JSON.stringify(v, null, 1) }] };
}

function reg(name, title, description, inputSchema, annotations, handler) {
  server.registerTool(name, { title, description, inputSchema, annotations }, async (params) => {
    try {
      return text(await handler(params));
    } catch (e) {
      return { isError: true, ...text(e instanceof Error ? e.message : String(e)) };
    }
  });
}

async function byId(id) {
  const all = await helper({ op: "list", status: "all" });
  const r = all.find((x) => x.id === id);
  if (!r) throw new Error(`no task ${id} (open tasks and all completed ones were searched)`);
  return task(r);
}

/** Fields of a task (current values overridden by the request) → a helper save command. */
function saveCmd(base, p) {
  const tags = p.tags ? parseTags(p.tags.join(",")) : [...(base?.tags ?? [])];
  for (const t of p.add_tags ?? []) if (!tags.some((x) => x.toLowerCase() === t.toLowerCase())) tags.push(t);
  let spent = p.spent_minutes ?? base?.spent_minutes ?? 0;
  if (p.add_spent_minutes) spent += p.add_spent_minutes;
  const cmd = {
    op: "save",
    ...(base ? { id: base.id } : {}),
    title: p.title ?? base?.title ?? "",
    notes: writeNotes({
      notes: p.notes ?? base?.notes ?? "",
      tags,
      evening: p.evening ?? (p.someday ? false : base?.evening ?? false),
      someday: p.someday ?? (p.evening ? false : base?.someday ?? false),
      spentMinutes: Math.max(0, Math.round(spent)),
      timer: base?._raw ? readNotes(base._raw.notes).timer : null,
    }),
  };
  if (p.day !== undefined) cmd.due = p.day === null || p.day === "" ? null : parseDay(p.day);
  else if (p.evening && !base?.day) cmd.due = parseDay("today");
  if (p.someday) cmd.due = null;
  if (p.important !== undefined) cmd.priority = p.important ? 1 : 0;
  if (p.completed !== undefined) cmd.completed = p.completed === false ? null : parseDay(p.completed === true ? "today" : p.completed);
  return cmd;
}

const dayArg = z
  .string()
  .nullable()
  .optional()
  .describe('Day the task is planned for: "today", "tomorrow", "+3", "YYYY-MM-DD", "DD.MM"; null removes it. Tasks with a day up to today are in Today.');
const tagsArg = z.array(z.string()).optional().describe('Tag names as they are, e.g. ["client-a", "Домашние дела"]. Replaces all tags.');

// ─── Reading ───
reg("list_tasks", "List tasks",
  "Tasks from Apple Reminders. status=open (default) / completed / all. For completed tasks from/to filter by the completion day (inclusive, YYYY-MM-DD or today/yesterday/+N) — use it for reports. view (open tasks): today (planned up to today, incl. overdue), evening, upcoming, anytime (no day or due), someday. tag = exact tag name (case-insensitive); query = text in title or notes. Returns count, total spent minutes and the tasks (id, title, notes, tags, day, evening, someday, important, completed, spent_minutes).",
  {
    status: z.enum(["open", "completed", "all"]).optional(),
    view: z.enum(VIEWS).optional(),
    from: z.string().optional(),
    to: z.string().optional(),
    tag: z.string().optional(),
    query: z.string().optional(),
    limit: z.number().int().positive().max(5000).optional().describe("Default 500"),
    with_notes: z.boolean().optional().describe("Include notes (default true); false for long reports"),
  },
  READ,
  async (p) => {
    const status = p.status ?? "open";
    const from = p.from ? parseDay(p.from) : undefined;
    const to = p.to ? parseDay(p.to) : undefined;
    const rows = await helper({ op: "list", status, ...(from ? { from } : {}), ...(to ? { to } : {}) });
    const today = ymd(new Date());
    const q = p.query?.toLowerCase();
    let tasks = rows.map(task).filter((t) =>
      (!p.view || (!t.completed && inView(t, p.view, today))) &&
      (!p.tag || t.tags.some((g) => g.toLowerCase() === p.tag.toLowerCase())) &&
      (!q || t.title.toLowerCase().includes(q) || t.notes.toLowerCase().includes(q)));
    tasks.sort((a, b) => (a.completed ?? "").localeCompare(b.completed ?? "") || (a.day ?? "9").localeCompare(b.day ?? "9"));
    const total = tasks.reduce((s, t) => s + t.spent_minutes, 0);
    const count = tasks.length;
    tasks = tasks.slice(0, p.limit ?? 500);
    return {
      count,
      ...(count > tasks.length ? { shown: tasks.length } : {}),
      spent_minutes_total: total,
      spent_total: showSpent(total),
      tasks: tasks.map((t) => {
        const c = clean(t);
        if (p.with_notes === false) delete c.notes;
        return c;
      }),
    };
  });

reg("get_task", "Get a task", "One task by id, with its notes.", { id: z.string() }, READ,
  async ({ id }) => clean(await byId(id)));

reg("list_tags", "List tags",
  "Tag names in use with how many open and recently completed tasks have them — the live list to pick from (write a tag exactly as it is listed; a new tag is just a new name).",
  { days: z.number().int().positive().optional().describe("Completed tasks of the last N days count too (default 365)") },
  READ,
  async ({ days }) => {
    const from = ymd(new Date(Date.now() - (days ?? 365) * 86400000));
    const rows = await helper({ op: "list", status: "all", from });
    const counts = new Map();
    for (const t of rows.map(task)) {
      for (const g of t.tags) {
        const k = [...counts.keys()].find((x) => x.toLowerCase() === g.toLowerCase()) ?? g;
        const c = counts.get(k) ?? { open: 0, completed: 0 };
        t.completed ? c.completed++ : c.open++;
        counts.set(k, c);
      }
    }
    return [...counts.entries()]
      .sort((a, b) => b[1].open + b[1].completed - (a[1].open + a[1].completed))
      .map(([tag, c]) => ({ tag, ...c }));
  });

// ─── Writing ───
reg("add_task", "Add a task",
  "Create a task. Without day it is an Anytime task (no date); day=today puts it in Today, evening=true in Today's This Evening, someday=true in Someday. important=true is the red flag (high priority). spent_minutes records time spent. completed=\"YYYY-MM-DD\"/\"today\" adds it already done (back-dating works). Returns the task with its id.",
  {
    title: z.string().min(1),
    notes: z.string().optional(),
    tags: tagsArg,
    day: dayArg,
    evening: z.boolean().optional(),
    someday: z.boolean().optional(),
    important: z.boolean().optional(),
    spent_minutes: z.number().nonnegative().optional(),
    completed: z.string().optional(),
  },
  WRITE,
  async (p) => clean(task(await helper(saveCmd(null, p)))));

reg("update_task", "Update a task",
  "Change a task: only the fields given change. tags replaces all tags, add_tags appends. day=null removes the day. spent_minutes sets the time spent, add_spent_minutes adds to it. completed=\"YYYY-MM-DD\"/\"today\"/true marks it done on that day, false reopens it.",
  {
    id: z.string(),
    title: z.string().optional(),
    notes: z.string().optional().describe("Replaces the notes text (tags/time lines are kept separately)"),
    tags: tagsArg,
    add_tags: z.array(z.string()).optional(),
    day: dayArg,
    evening: z.boolean().optional(),
    someday: z.boolean().optional(),
    important: z.boolean().optional(),
    spent_minutes: z.number().nonnegative().optional(),
    add_spent_minutes: z.number().optional(),
    completed: z.union([z.string(), z.boolean()]).optional(),
  },
  WRITE,
  async (p) => clean(task(await helper(saveCmd(await byId(p.id), p)))));

reg("complete_tasks", "Complete tasks",
  "Mark tasks done (on date, default today — any past day works) or, with reopen=true, not done. Optionally set the time spent for all of them.",
  {
    ids: z.array(z.string()).min(1),
    date: z.string().optional(),
    reopen: z.boolean().optional(),
    spent_minutes: z.number().nonnegative().optional(),
  },
  WRITE,
  async ({ ids, date, reopen, spent_minutes }) => {
    const all = (await helper({ op: "list", status: "all" })).map(task);
    const out = [];
    for (const id of ids) {
      const base = all.find((t) => t.id === id);
      if (!base) {
        out.push({ id, error: "no such task" });
        continue;
      }
      const p = { completed: reopen ? false : date ?? "today", ...(spent_minutes !== undefined ? { spent_minutes } : {}) };
      const t = task(await helper(saveCmd(base, p)));
      out.push({ id, title: t.title, completed: t.completed });
    }
    return out;
  });

reg("delete_task", "Delete a task",
  "Remove a task from Reminders (it stays 30 days in Reminders' Recently Deleted). Prefer complete_tasks for finished work.",
  { id: z.string() },
  DESTRUCTIVE,
  async ({ id }) => helper({ op: "delete", id }));

await server.connect(new StdioServerTransport());
