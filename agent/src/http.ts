// What every module of the server shares: a request, a route, an error with its status, and the
// small helpers routes use.

import fs from "node:fs";
import type http from "node:http";
import os from "node:os";
import path from "node:path";
import type { Event, Methods } from "./protocol.gen.ts";
import type { Runtime } from "./runtime.ts";

export type Req = http.IncomingMessage & { body?: any; query: URLSearchParams; path: string };
export type Route = (req: Req) => Promise<any> | any;
/** A route with a path parameter: method, its pattern, the handler with the match. */
export type Dynamic = [string, RegExp, (req: Req, m: RegExpMatchArray) => any];
/** The JSON-RPC methods a module answers, typed by crates/protocol (`Methods`): each gets its
 * params and returns its answer, as Bench reads them. */
export type Handlers = { [M in keyof Methods]?: (params: Methods[M][0]) => Methods[M][1] | Promise<Methods[M][1]> };
/** Tell every connected Bench something (crates/protocol `Event`). */
export type Publish = (event: Event) => void;

/** What a module gets from the server. */
export interface Ctx {
	rt: Runtime;
	publish: Publish;
}

/** What a module gives the server: JSON-RPC methods, fixed REST routes ("GET /api/x"), routes with a parameter. */
export interface Module {
	rpc?: Handlers;
	routes?: Record<string, Route>;
	dynamic?: Dynamic[];
}

export class HttpError extends Error {
	status: number;
	constructor(status: number, message: string) {
		super(message);
		this.status = status;
	}
}

export const expand = (p: string) => (p.startsWith("~") ? path.join(os.homedir(), p.slice(1)) : p);
/** The agent settings a REST call is about: "default" (the general ones) or a project's `project:<id>`. */
export const agentOf = (req: Req) => req.query.get("agent") || "default";

export const required = (v: unknown, name: string): string => {
	if (typeof v !== "string" || !v.trim()) throw new HttpError(400, `${name} is required`);
	return v;
};

export function dataUrl(url: string): Buffer {
	const m = String(url).match(/^data:[^;,]*(;base64)?,(.*)$/s);
	if (!m) throw new HttpError(400, "bad data_url");
	return m[1] ? Buffer.from(m[2], "base64") : Buffer.from(decodeURIComponent(m[2]));
}

export function isDir(p: string): boolean {
	try {
		return fs.statSync(p).isDirectory();
	} catch {
		return false;
	}
}

export function gitRoot(cwd: string): string | null {
	for (let d = cwd; d && d !== path.dirname(d); d = path.dirname(d)) if (fs.existsSync(path.join(d, ".git"))) return d;
	return null;
}

export async function readBody(req: http.IncomingMessage): Promise<any> {
	const chunks: Buffer[] = [];
	for await (const c of req) chunks.push(c as Buffer);
	const text = Buffer.concat(chunks).toString("utf8");
	if (!text) return undefined;
	try {
		return JSON.parse(text);
	} catch {
		return text;
	}
}
