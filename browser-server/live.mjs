// The live view of the browser's tabs for Bench: wss://browser.example.com/live (proxy → :8932).
//
// Why apart from the MCP: the MCP is the agent's channel (tool calls); this is the picture for the
// people, to see what the agent does and to step in. Raw CDP is never exposed: it can do anything
// (cookies, running code), so here there is a narrow set of operations, personal keys and a log.
//
// Access: ?token=… — a person's own key; only its sha256 is kept (/data/live-users.json).
//   give one:   docker compose exec browser node /opt/live.mjs add <name>   (the key prints once)
//   revoke:     docker compose exec browser node /opt/live.mjs remove <name>
//   list:       docker compose exec browser node /opt/live.mjs list
// Every operation goes to /data/live-audit.log (time, person, operation, detail).
//
// Protocol (JSON over the WebSocket):
//   → {op:"tabs"} | {op:"watch",tab} | {op:"wake",tab} | {op:"unwatch"} | {op:"open",url} | {op:"close",tab}
//     {op:"mouse",kind:"mousePressed"|"mouseReleased"|"mouseMoved",x,y,button,clicks}
//     {op:"wheel",x,y,dx,dy} | {op:"text",text} | {op:"key",key,code,vk,modifiers}
//   ← {tabs:[{id,url,title,asleep?}]} (and on every change) | {opened:id} | {woke:id, as:id}
//     {frame:base64 jpeg,w,h,tab,asleep?}
//     {error:"…"}
// Frames: no more often than FRAME_GAP_MS (the frame's ack is held back), and only while someone watches.

import { createHash, randomBytes } from "node:crypto";
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";

const require = createRequire("/usr/local/lib/node_modules/");
const { WebSocketServer } = require("ws");

const PORT = Number(process.env.LIVE_PORT || 8932);
const CDP = "http://127.0.0.1:9222";
const USERS = "/data/live-users.json";
const AUDIT = "/data/live-audit.log";
const FRAME_GAP_MS = Number(process.env.LIVE_FRAME_GAP_MS || 500);
const MAX_CLIENTS = Number(process.env.LIVE_MAX_CLIENTS || 50);
// Memory is the bottleneck (a tab is 200–450 MB of 1 GB). At most LIVE_TABS tabs are awake; an extra
// one — the least recently touched that nobody watches — falls asleep: a screenshot and its address
// are kept, the tab is closed, and it opens again on request (sign-ins live in the profile). A tab
// touched within BUSY_MS is left alone.
const LIVE_TABS = Number(process.env.LIVE_TABS || 3);
const HARD_TABS = Number(process.env.LIVE_HARD_TABS || 5);
const BUSY_MS = Number(process.env.LIVE_BUSY_MS || 60000);
const SLEEP_DIR = "/data/asleep";
// Ads and counters: on news sites they are most of a page's weight.
const BLOCK = (process.env.LIVE_BLOCK ||
	"*doubleclick.net*,*googlesyndication.com*,*googleadservices.com*,*google-analytics.com*,*googletagmanager.com*,*adservice.google.*,*an.yandex.ru*,*yandexadexchange.net*,*adfox.ru*,*mc.yandex.ru*,*top-fwz1.mail.ru*,*ad.mail.ru*,*ads.vk.com*,*criteo.*,*adriver.ru*,*betweendigital.com*,*sape.ru*,*facebook.net*,*tiktok.com/i18n/pixel*,*hotjar.com*,*mediametrics.ru*,*buzzoola.com*,*otm-r.com*").split(",");

const sha = (s) => createHash("sha256").update(s).digest("hex");
const users = () => (existsSync(USERS) ? JSON.parse(readFileSync(USERS, "utf8")) : {});
const log = (...a) => console.log(new Date().toISOString(), "live:", ...a);
const audit = (user, op, detail = "") => {
	try {
		appendFileSync(AUDIT, `${new Date().toISOString()}\t${user}\t${op}\t${String(detail).slice(0, 300)}\n`);
	} catch {}
};

// ---- keys ---------------------------------------------------------
if (process.argv[2] === "add" || process.argv[2] === "remove" || process.argv[2] === "list") {
	const u = users();
	const name = process.argv[3];
	if (process.argv[2] === "list") console.log(Object.keys(u).join("\n") || "(none)");
	else if (!name) console.error("a name?");
	else if (process.argv[2] === "remove") {
		delete u[name];
		writeFileSync(USERS, JSON.stringify(u, null, 2));
		console.log(`key ${name} revoked`);
	} else {
		const token = randomBytes(24).toString("base64url");
		u[name] = sha(token);
		writeFileSync(USERS, JSON.stringify(u, null, 2), { mode: 0o600 });
		console.log(token);
	}
	process.exit(0);
}

// ---- one CDP connection for everyone ----------------------------------------------------------
let cdp, nextId = 0;
const pending = new Map();
const tabs = new Map(); // targetId -> {id,url,title,asleep?}
const touched = new Map(); // targetId -> last activity (navigation, frames, a person's input)
const touch = (id) => touched.set(id, Date.now());
const watchers = new Map(); // targetId -> {session, clients:Set, ack:null|{id,session}, timer}

async function connect() {
	const v = await (await fetch(`${CDP}/json/version`)).json();
	cdp = new WebSocket(v.webSocketDebuggerUrl);
	cdp.onmessage = (m) => onCdp(JSON.parse(m.data));
	cdp.onclose = () => {
		log("CDP closed, reconnecting");
		for (const w of watchers.values()) clearTimeout(w.timer);
		watchers.clear();
		setTimeout(() => connect().catch((e) => log(e.message)), 2000);
	};
	await new Promise((r, j) => ((cdp.onopen = r), (cdp.onerror = j)));
	await call("Target.setDiscoverTargets", { discover: true });
	// Attach to every tab (and new ones) to block ads; the agent and the MCP don't notice.
	await call("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
	log("CDP connected");
}

function call(method, params = {}, sessionId) {
	return new Promise((resolve, reject) => {
		const id = ++nextId;
		pending.set(id, { resolve, reject });
		cdp.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
		setTimeout(() => pending.delete(id) && reject(new Error(`${method}: timeout`)), 15000);
	});
}
const send = (method, params, sessionId) => cdp.send(JSON.stringify({ id: ++nextId, method, params, ...(sessionId ? { sessionId } : {}) }));

function onCdp(m) {
	if (m.id && pending.has(m.id)) {
		const p = pending.get(m.id);
		pending.delete(m.id);
		m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result);
		return;
	}
	if (m.method === "Target.attachedToTarget" && m.params.targetInfo?.type === "page") {
		const sid = m.params.sessionId;
		send("Network.enable", {}, sid);
		send("Network.setBlockedURLs", { urls: BLOCK }, sid);
		touch(m.params.targetInfo.targetId);
		return;
	}
	const t = m.params?.targetInfo;
	if ((m.method === "Target.targetCreated" || m.method === "Target.targetInfoChanged") && t?.type === "page") {
		const was = tabs.get(t.targetId);
		if (!was || was.url !== t.url) touch(t.targetId);
		tabs.set(t.targetId, { id: t.targetId, url: t.url, title: t.title });
		broadcastTabs();
		if (!was) setTimeout(budget, 1000);
	} else if (m.method === "Target.targetDestroyed") {
		// A sleeping tab stays in the list (screenshot + address); one really closed goes.
		if (!tabs.get(m.params.targetId)?.asleep) tabs.delete(m.params.targetId);
		const w = watchers.get(m.params.targetId);
		if (w) clearTimeout(w.timer), watchers.delete(m.params.targetId);
		broadcastTabs();
	} else if (m.method === "Page.screencastFrame") {
		const w = [...watchers.values()].find((x) => x.session === m.sessionId);
		if (!w) return;
		touch(w.tab);
		const frame = JSON.stringify({ frame: m.params.data, w: m.params.metadata.deviceWidth, h: m.params.metadata.deviceHeight, tab: w.tab });
		for (const c of w.clients) if (c.readyState === 1 && c.bufferedAmount < 2e6) c.send(frame);
		// The next frame no sooner than FRAME_GAP_MS (and only while someone watches).
		w.timer = setTimeout(() => w.clients.size && send("Page.screencastFrameAck", { sessionId: m.params.sessionId }, m.sessionId), FRAME_GAP_MS);
	}
}

const clients = new Set();
function broadcastTabs() {
	const list = JSON.stringify({ tabs: [...tabs.values()] });
	for (const c of clients) if (c.readyState === 1) c.send(list);
}

async function watch(c, tab) {
	unwatch(c);
	let w = watchers.get(tab);
	if (!w) {
		const { sessionId } = await call("Target.attachToTarget", { targetId: tab, flatten: true });
		w = { tab, session: sessionId, clients: new Set(), timer: null };
		watchers.set(tab, w);
		await call("Page.enable", {}, sessionId);
		send("Page.startScreencast", { format: "jpeg", quality: 55, maxWidth: 1100, maxHeight: 1100, everyNthFrame: 1 }, sessionId);
	}
	w.clients.add(c);
	c.watching = tab;
}

function unwatch(c) {
	const w = c.watching && watchers.get(c.watching);
	c.watching = null;
	if (!w) return;
	w.clients.delete(c);
	if (!w.clients.size) {
		clearTimeout(w.timer);
		send("Page.stopScreencast", {}, w.session);
		send("Target.detachFromTarget", { sessionId: w.session });
		watchers.delete(w.tab);
	}
}

// ---- the memory budget -----------------------------------------------------------------
const awake = () => [...tabs.values()].filter((t) => !t.asleep && t.url !== "about:blank");
const busy = (id) => watchers.has(id) || Date.now() - (touched.get(id) || 0) < BUSY_MS;

async function sleepTab(id) {
	const t = tabs.get(id);
	if (!t) return;
	try {
		const { sessionId } = await call("Target.attachToTarget", { targetId: id, flatten: true });
		const shot = await call("Page.captureScreenshot", { format: "jpeg", quality: 50 }, sessionId);
		const { mkdirSync } = await import("node:fs");
		mkdirSync(SLEEP_DIR, { recursive: true });
		writeFileSync(`${SLEEP_DIR}/${id}.jpg`, Buffer.from(shot.data, "base64"));
	} catch {}
	tabs.set(id, { ...t, asleep: true });
	send("Target.closeTarget", { targetId: id });
	log(`asleep: ${t.url}`);
	broadcastTabs();
}

/** Extra awake tabs fall asleep: the least recently touched ones nobody watches. */
async function budget() {
	let live = awake();
	while (live.length > LIVE_TABS) {
		const idle = live.filter((t) => !busy(t.id)).sort((a, b) => (touched.get(a.id) || 0) - (touched.get(b.id) || 0));
		if (!idle.length) break; // all in use: keep up to HARD_TABS, past that `open` is refused
		await sleepTab(idle[0].id);
		live = awake();
	}
}
setInterval(() => budget().catch((e) => log(e.message)), 30000);

/** A sleeping tab opens again at its address; returns the new id. */
async function wake(id) {
	const t = tabs.get(id);
	if (!t?.asleep) return id;
	if (awake().length >= HARD_TABS) throw new Error("browser busy: all tabs in use, try again shortly");
	const { targetId } = await call("Target.createTarget", { url: t.url });
	tabs.delete(id);
	touch(targetId);
	broadcastTabs();
	setTimeout(budget, 1000);
	return targetId;
}

// ---- the WebSocket for Bench -----------------------------------------------------------
const server = createServer((req, res) => {
	res.writeHead(req.url === "/live/health" ? 200 : 404);
	res.end(req.url === "/live/health" ? "ok" : "");
});
const wss = new WebSocketServer({ noServer: true, maxPayload: 1 << 20 });
server.on("upgrade", (req, socket, head) => {
	const url = new URL(req.url, "http://x");
	const token = url.searchParams.get("token") || "";
	const user = Object.entries(users()).find(([, h]) => h === sha(token))?.[0];
	if (!url.pathname.startsWith("/live") || !user || clients.size >= MAX_CLIENTS) {
		socket.write("HTTP/1.1 401 Unauthorized\r\n\r\n");
		socket.destroy();
		if (!user) log("refused: wrong key");
		return;
	}
	wss.handleUpgrade(req, socket, head, (c) => {
		c.user = user;
		clients.add(c);
		audit(user, "connect");
		c.send(JSON.stringify({ tabs: [...tabs.values()] }));
		c.on("close", () => (unwatch(c), clients.delete(c), audit(user, "disconnect")));
		c.on("message", async (raw) => {
			let m;
			try {
				m = JSON.parse(raw);
			} catch {
				return;
			}
			const sid = c.watching && watchers.get(c.watching)?.session;
			try {
				switch (m.op) {
					case "tabs":
						c.send(JSON.stringify({ tabs: [...tabs.values()] }));
						break;
					case "watch": {
						const t = tabs.get(m.tab);
						if (!t) break;
						if (t.asleep) {
							// While asleep: its screenshot; only an explicit wake opens it again.
							if (existsSync(`${SLEEP_DIR}/${m.tab}.jpg`))
								c.send(JSON.stringify({ frame: readFileSync(`${SLEEP_DIR}/${m.tab}.jpg`).toString("base64"), w: 0, h: 0, tab: m.tab, asleep: true }));
							break;
						}
						await watch(c, m.tab);
						touch(m.tab);
						audit(user, "watch", t.url);
						break;
					}
					case "wake": {
						const id = await wake(m.tab);
						audit(user, "wake", tabs.get(id)?.url);
						c.send(JSON.stringify({ woke: m.tab, as: id }));
						break;
					}
					case "unwatch":
						unwatch(c);
						break;
					case "open": {
						if (!/^https?:\/\//.test(String(m.url))) throw new Error("only http(s) pages");
						if (awake().length >= HARD_TABS) throw new Error("browser busy: all tabs in use, try again shortly");
						const { targetId } = await call("Target.createTarget", { url: m.url });
						audit(user, "open", m.url);
						c.send(JSON.stringify({ opened: targetId }));
						break;
					}
					case "close":
						if (tabs.has(m.tab)) send("Target.closeTarget", { targetId: m.tab }), audit(user, "close", tabs.get(m.tab)?.url);
						break;
					case "mouse":
						if (c.watching) touch(c.watching);
						if (sid && ["mousePressed", "mouseReleased", "mouseMoved"].includes(m.kind))
							send("Input.dispatchMouseEvent", { type: m.kind, x: +m.x, y: +m.y, button: m.button || "none", clickCount: +m.clicks || 0 }, sid);
						if (m.kind === "mousePressed") audit(user, "click", `${Math.round(m.x)},${Math.round(m.y)}`);
						break;
					case "wheel":
						if (sid) send("Input.dispatchMouseEvent", { type: "mouseWheel", x: +m.x, y: +m.y, deltaX: +m.dx, deltaY: +m.dy }, sid);
						break;
					case "text":
						if (sid) send("Input.insertText", { text: String(m.text).slice(0, 100000) }, sid), audit(user, "type", m.text);
						break;
					case "key":
						if (sid) {
							for (const type of ["rawKeyDown", "keyUp"])
								send("Input.dispatchKeyEvent", { type, key: String(m.key), code: String(m.code), windowsVirtualKeyCode: +m.vk, modifiers: +m.modifiers || 0 }, sid);
							if (m.key === "Enter") send("Input.dispatchKeyEvent", { type: "char", text: "\r" }, sid);
						}
						break;
				}
			} catch (e) {
				c.send(JSON.stringify({ error: e.message }));
			}
		});
	});
});

await connect();
const r = await call("Target.getTargets");
for (const t of r.targetInfos) if (t.type === "page") tabs.set(t.targetId, { id: t.targetId, url: t.url, title: t.title });
server.listen(PORT, "0.0.0.0", () => log(`listening on :${PORT}`));
