// One life of the agent for `parked.test.ts`: it opens, makes the thread on its first life, says
// what it finds, and waits to be killed, closed (SIGTERM) or told to retry.

const { Runtime } = await import("../src/runtime/runtime.ts");
const say = (word: string) => process.stdout.write(`${word}\n`);

const rt = new Runtime(() => {});
await rt.open();
let row = rt.store.list({ limit: 1 })[0];
if (!row) {
	const made = await rt.create("default", undefined, "local");
	row = made.row;
	await rt.submit(row.id, "job");
}
say(rt.parked.of(row.conversation) ? "parked" : "not-parked");
process.stdin.on("data", async () => {
	await rt.parked.retry(row.conversation);
	say(rt.parked.of(row.conversation) ? "parked" : "not-parked");
});
process.on("SIGTERM", async () => {
	await rt.shutdown();
	process.exit(0);
});
