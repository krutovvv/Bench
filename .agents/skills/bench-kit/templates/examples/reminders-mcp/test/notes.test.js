import { test } from "node:test";
import assert from "node:assert/strict";
import { parseDay, parseSpent, readNotes, showSpent, writeNotes } from "../src/notes.js";

test("notes lines round-trip, the same as Bench writes them", () => {
  const stored = writeNotes({ notes: "Call first.\nThen write.", tags: ["work", "Client X"], evening: true, spentMinutes: 90 });
  assert.equal(stored, "Call first.\nThen write.\n\nTags: work, Client X\nWhen: evening\nSpent: 1h 30m");
  const back = readNotes(stored);
  assert.deepEqual(back.tags, ["work", "Client X"]);
  assert.equal(back.notes, "Call first.\nThen write.");
  assert.ok(back.evening && back.spentMinutes === 90);
  const phone = readNotes("#home\nBuy milk #2 today\nTags: Домашние дела, home\n");
  assert.deepEqual(phone.tags, ["home", "Домашние дела"]);
  assert.equal(phone.notes, "Buy milk #2 today");
  assert.equal(writeNotes({ notes: "Plain" }), "Plain");
});

test("durations and days as typed", () => {
  assert.equal(parseSpent("1h 30m"), 90);
  assert.equal(parseSpent("4h"), 240);
  assert.equal(parseSpent("1:30"), 90);
  assert.equal(parseSpent("45"), 45);
  assert.equal(parseSpent("2ч"), 120);
  assert.equal(parseSpent("abc"), null);
  assert.equal(showSpent(90), "1h 30m");
  assert.equal(showSpent(5), "5m");
  const now = new Date(2026, 9, 23);
  assert.equal(parseDay("today", now), "2026-10-23");
  assert.equal(parseDay("tomorrow", now), "2026-10-24");
  assert.equal(parseDay("+3", now), "2026-10-26");
  assert.equal(parseDay("25.10", now), "2026-10-25");
  assert.equal(parseDay("2026-12-31", now), "2026-12-31");
  assert.throws(() => parseDay("someday soon", now));
});
