// What Reminders has no field for lives as plain lines at the end of a reminder's notes, the
// same lines Bench reads and writes:
//
//   Whatever the notes say.
//
//   Tags: client-a, Дом
//   When: evening            (or: someday)
//   Spent: 1h 30m
//
// A line made only of #words counts as tags too (typed on the phone). "Timer: <unix>" belongs
// to Bench's running timer and is kept as is.

/** "1h 30m", "90m", "1:30", "45" (minutes), "2ч" → minutes; "" → 0; null when not a duration. */
export function parseSpent(s) {
  const t = String(s ?? "").trim().toLowerCase().replace(/\s+/g, "");
  if (!t) return 0;
  let m = t.match(/^(\d+):(\d{1,2})$/);
  if (m) return Number(m[1]) * 60 + Number(m[2]);
  if (/^\d+$/.test(t)) return Number(t);
  let total = 0;
  const re = /(\d+(?:[.,]\d+)?)(h|ч|m|м)/g;
  let rest = t;
  while ((m = re.exec(t))) {
    const n = Number(m[1].replace(",", "."));
    total += m[2] === "h" || m[2] === "ч" ? n * 60 : n;
    rest = rest.replace(m[0], "");
  }
  return rest === "" ? Math.round(total) : null;
}

/** Minutes → "1h 30m", "45m", "2h". */
export function showSpent(min) {
  const h = Math.floor(min / 60), m = min % 60;
  if (!h) return `${m}m`;
  return m ? `${h}h ${m}m` : `${h}h`;
}

/** "a, b ,a" → ["a", "b"] (first spelling wins, case-insensitive duplicates dropped). */
export function parseTags(s) {
  const out = [];
  for (const t of String(s ?? "").split(",").map((x) => x.trim()).filter(Boolean)) {
    if (!out.some((x) => x.toLowerCase() === t.toLowerCase())) out.push(t);
  }
  return out;
}

/** Stored notes → { notes, tags, evening, someday, spentMinutes, timer }. */
export function readNotes(stored) {
  const keep = [];
  const r = { notes: "", tags: [], evening: false, someday: false, spentMinutes: 0, timer: null };
  const addTag = (t) => {
    if (t && !r.tags.some((x) => x.toLowerCase() === t.toLowerCase())) r.tags.push(t);
  };
  for (const line of String(stored ?? "").split("\n")) {
    const t = line.trim();
    const words = t.split(/\s+/).filter(Boolean);
    if (t.startsWith("Tags:")) parseTags(t.slice(5)).forEach(addTag);
    else if (t.startsWith("When:")) {
      const v = t.slice(5).trim().toLowerCase();
      if (v === "evening") r.evening = true;
      else if (v === "someday") r.someday = true;
      else keep.push(line);
    } else if (t.startsWith("Spent:")) r.spentMinutes = parseSpent(t.slice(6)) ?? 0;
    else if (t.startsWith("Timer:")) r.timer = Number(t.slice(6).trim()) || null;
    else if (words.length && words.every((w) => w.length > 1 && w.startsWith("#"))) {
      for (const w of words) {
        const tag = w.slice(1);
        if (tag.toLowerCase() === "evening") r.evening = true;
        else if (tag.toLowerCase() === "someday") r.someday = true;
        else addTag(tag);
      }
    } else keep.push(line);
  }
  r.notes = keep.join("\n").trim();
  return r;
}

/** { notes, tags, evening, someday, spentMinutes, timer } → stored notes. */
export function writeNotes({ notes = "", tags = [], evening = false, someday = false, spentMinutes = 0, timer = null }) {
  const lines = [];
  if (tags.length) lines.push(`Tags: ${tags.join(", ")}`);
  if (evening) lines.push("When: evening");
  else if (someday) lines.push("When: someday");
  if (spentMinutes > 0) lines.push(`Spent: ${showSpent(spentMinutes)}`);
  if (timer) lines.push(`Timer: ${timer}`);
  const text = String(notes).trimEnd();
  if (!lines.length) return text;
  return text ? `${text}\n\n${lines.join("\n")}` : lines.join("\n");
}

/** Local "YYYY-MM-DD" of a Date. */
export function ymd(d) {
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** "today", "tomorrow", "yesterday", "+3", "YYYY-MM-DD", "DD.MM", "DD.MM.YYYY" → "YYYY-MM-DD". */
export function parseDay(s, now = new Date()) {
  const t = String(s ?? "").trim().toLowerCase();
  const shift = (n) => ymd(new Date(now.getFullYear(), now.getMonth(), now.getDate() + n));
  if (t === "today" || t === "сегодня") return shift(0);
  if (t === "tomorrow" || t === "завтра") return shift(1);
  if (t === "yesterday" || t === "вчера") return shift(-1);
  let m = t.match(/^\+(\d+)$/);
  if (m) return shift(Number(m[1]));
  if (/^\d{4}-\d{2}-\d{2}$/.test(t)) return t;
  m = t.match(/^(\d{1,2})\.(\d{1,2})(?:\.(\d{4}))?$/);
  if (m) {
    const p = (n) => String(n).padStart(2, "0");
    return `${m[3] ?? now.getFullYear()}-${p(m[2])}-${p(m[1])}`;
  }
  throw new Error(`"${s}" is not a day (use today, tomorrow, +3, YYYY-MM-DD or DD.MM)`);
}
