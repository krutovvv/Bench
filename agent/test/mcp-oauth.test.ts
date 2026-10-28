// What a server's sign-in error says to the person.
import assert from "node:assert/strict";
import { test } from "node:test";

process.env.BENCH_AGENT_NO_MCP = "1";
const { explain } = await import("../src/mcp/mcp.ts");

test("a service that refuses self-registration says how to sign in to it", () => {
	assert.match(explain("OAuth dynamic client registration failed with status 403: Forbidden"), /"oauth": \{"client_id"/);
	assert.equal(explain("connect timed out"), "connect timed out");
});
