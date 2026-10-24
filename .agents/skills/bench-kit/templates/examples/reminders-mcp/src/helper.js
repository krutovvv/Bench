// The EventKit half: "Reminders Bridge.app" (helper/main.swift, built by `npm run build:helper`),
// started through `open` so macOS treats it as its own app and asks for Reminders access once,
// whoever runs the MCP (the Hermes gateway, a terminal, cron). One JSON command in a file, one
// JSON answer in another.
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const APP = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "bin", "Reminders Bridge.app");

export async function helper(cmd) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "reminders-mcp-"));
  const inFile = path.join(dir, "in.json"), outFile = path.join(dir, "out.json");
  try {
    await fs.writeFile(inFile, JSON.stringify(cmd));
    await new Promise((resolve, reject) =>
      execFile("/usr/bin/open", ["-W", "-g", "-n", "-a", APP, "--args", inFile, outFile], { timeout: 150000 },
        (e) => (e ? reject(new Error(`Reminders Bridge: ${e.message}`)) : resolve())));
    let text;
    try {
      text = await fs.readFile(outFile, "utf8");
    } catch {
      throw new Error("Reminders Bridge gave no answer (is it built? npm run build:helper)");
    }
    const v = JSON.parse(text);
    if (v && !Array.isArray(v) && v.error) throw new Error(v.error);
    return v;
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}
