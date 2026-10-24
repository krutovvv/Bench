// Manual end-to-end check through a real MCP client (touches Reminders: creates and deletes one
// throwaway task). Run: node test/e2e.mjs
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const c = new Client({ name: "e2e", version: "1" });
await c.connect(new StdioClientTransport({ command: "node", args: [new URL("../src/index.js", import.meta.url).pathname] }));
const call = async (name, args) => {
  const t0 = Date.now();
  const r = await c.callTool({ name, arguments: args });
  const v = r.content[0].text;
  if (r.isError) throw new Error(`${name}: ${v}`);
  return [JSON.parse(v), Date.now() - t0];
};
console.log("tools:", (await c.listTools()).tools.map((t) => t.name).join(", "));
let [v, ms] = await call("list_tags", {});
console.log(`list_tags ${ms}ms:`, v.slice(0, 6));
[v, ms] = await call("list_tasks", { view: "today", with_notes: false });
console.log(`today ${ms}ms: ${v.count}`, v.tasks.map((t) => t.title).slice(0, 3));
[v, ms] = await call("list_tasks", { status: "completed", from: "2026-10-01", to: "2026-10-31", tag: "client-a", with_notes: false });
console.log(`client-a October done ${ms}ms: ${v.count} tasks, ${v.spent_total}`);
[v] = await call("add_task", { title: "MCP test — delete me", notes: "test", tags: ["client-a"], day: "today", important: true });
const id = v.id;
console.log("added:", v.title, v.tags, v.day, v.important);
[v] = await call("update_task", { id, add_tags: ["Домашние дела"], add_spent_minutes: 45, evening: true });
console.log("updated:", v.tags, v.spent_minutes, v.evening);
[v] = await call("complete_tasks", { ids: [id], date: "2026-10-20" });
console.log("completed:", v);
[v] = await call("get_task", { id });
console.log("read back:", v.completed, v.spent_minutes, v.tags, v.evening);
[v] = await call("delete_task", { id });
console.log("deleted:", v);
await c.close();
