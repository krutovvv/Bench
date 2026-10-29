// This process as the one agent of its home. On every machine the agent is the system's service
// (launchd on macOS, systemd on Linux: src/client/service.rs), which already runs one; the lock
// here makes sure of it whatever started the process — a second agent on one pi.sqlite would
// resume the same runs twice. Also the build it runs and its log's cap.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

/** The sources this agent runs: the `.build` Bench writes beside them when it installs them
 * (none when started from the repository). */
export const BUILD: string | null = (() => {
	try {
		return fs.readFileSync(new URL("../.build", import.meta.url), "utf8").trim() || null;
	} catch {
		return null;
	}
})();

/** A process that is an agent: alive, and its command is the server (a pid from before a
 * reboot may name another process now). */
function isAgent(pid: number): boolean {
	try {
		process.kill(pid, 0);
		return execFileSync("ps", ["-o", "command=", "-p", String(pid)], { encoding: "utf8" }).includes("server.ts");
	} catch {
		return false;
	}
}

/** Takes `agent.pid` in `home`, or exits saying which agent holds it. Released at exit. */
export function holdHome(home: string): void {
	fs.mkdirSync(home, { recursive: true });
	const file = path.join(home, "agent.pid");
	for (let attempt = 0; attempt < 3; attempt++) {
		try {
			fs.writeFileSync(file, String(process.pid), { flag: "wx" });
			process.on("exit", () => {
				try {
					if (fs.readFileSync(file, "utf8") === String(process.pid)) fs.unlinkSync(file);
				} catch {}
			});
			return;
		} catch (e: any) {
			if (e.code !== "EEXIST") throw e;
		}
		// Another start may have made the file and not yet written its pid.
		Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 200);
		const holder = Number(fs.readFileSync(file, "utf8"));
		if (holder && holder !== process.pid && isAgent(holder)) {
			console.error(`[agent] another agent runs on ${home} (pid ${holder}): this one exits`);
			process.exit(1);
		}
		// Left by an agent that is gone.
		fs.rmSync(file, { force: true });
	}
	throw new Error(`could not take ${file}`);
}

/** The service appends to `server.log`; a start past this size begins it again. */
const LOG_CAP = 4 * 1024 * 1024;

export function capLog(home: string): void {
	const file = path.join(home, "server.log");
	try {
		if (fs.statSync(file).size > LOG_CAP) fs.truncateSync(file, 0);
	} catch {}
}
