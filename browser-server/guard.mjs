// The server browser's guard: the container's main process (after tini).
//
// Why checking /json/version is not enough: on OOM the kernel kills a tab's renderer, the tab stays
// "dead" (CDP still lists it, but it has no JS context), and Playwright waits for it forever on
// every connectOverCDP — the MCP answers "initializeServer: Timeout 30000ms" to every call while
// /json/version is alive and the healthcheck is green.
//
// What the guard does:
//  1. every TICK ms — is the browser's CDP alive; 3 failures in a row -> kill chromium (the loop in
//     entrypoint.sh starts it again);
//  2. every TICK ms — pings each tab with Runtime.evaluate('1'); no answer twice in a row ->
//     dismiss a stuck alert -> reload -> close the tab and open its URL again;
//  3. every MCP_EVERY ticks — an end-to-end MCP check (initialize + browser_tabs list);
//     failures in a row: 2 -> restart the MCP, 4 -> restart chromium, 6 -> exit (Docker restarts
//     the container);
//  4. closes idle tabs: a heavy tab holds 200–450 MB of a 1 GB limit, and Chrome in a container
//     doesn't discard them by itself (it looks at the machine's memory, not the cgroup's limit).
//     Idle means no input, clicks, paste or navigation for over TAB_IDLE_MIN minutes; under memory
//     pressure (anon > MEM_PRESSURE of the limit) for over TAB_IDLE_PRESSURE_MIN, one per tick,
//     oldest first. The last tab is never closed (the window would close and chromium exit) — it
//     is replaced with about:blank.

import { readFileSync } from 'node:fs';

const CDP = 'http://127.0.0.1:9222';
const MCP = 'http://127.0.0.1:8931/mcp';
const TICK = 20000;
const MCP_EVERY = 6;
const PAGE_TIMEOUT = 5000;
const IDLE_MS = Number(process.env.TAB_IDLE_MIN || 120) * 60000;
const IDLE_PRESSURE_MS = Number(process.env.TAB_IDLE_PRESSURE_MIN || 15) * 60000;
const MEM_PRESSURE = Number(process.env.MEM_PRESSURE || 0.75);

// The last activity mark lives in the page itself: set on the first check after the document
// loads and updated on any input. Playwright sends real events, and scripts (a dispatched paste,
// input.click()) produce them too. A symbol key, so no global variable shows.
const ACTIVITY_JS = `(() => {
  const k = Symbol.for('rb.activity');
  if (!window[k]) {
    const mark = () => { window[k] = Date.now(); };
    mark();
    for (const e of ['pointerdown', 'keydown', 'input', 'paste', 'click', 'visibilitychange']) addEventListener(e, mark, true);
  }
  return window[k];
})()`;

const log = (...a) => console.log(new Date().toISOString(), 'guard:', ...a);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
process.on('uncaughtException', (e) => log('uncaught', e?.stack || e));
process.on('unhandledRejection', (e) => log('unhandled', e?.stack || e));

const killPid = (file, what) => {
  try {
    const pid = Number(readFileSync(file, 'utf8').trim());
    if (pid > 1) { process.kill(pid, 'SIGKILL'); log(`restarting ${what} (pid ${pid})`); }
  } catch (e) { log(`could not kill ${what}:`, e.message); }
};

async function fetchJson(url, ms) {
  const r = await fetch(url, { signal: AbortSignal.timeout(ms) });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return r.json();
}

// A minimal CDP client: one WebSocket to the browser, flattened sessions to the tabs.
class Cdp {
  static async open(wsUrl) {
    const c = new Cdp();
    c.ws = new WebSocket(wsUrl);
    c.id = 0; c.pend = new Map();
    c.ws.onmessage = (m) => {
      const d = JSON.parse(m.data);
      const p = d.id && c.pend.get(d.id);
      if (p) { c.pend.delete(d.id); d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result); }
    };
    await new Promise((res, rej) => {
      c.ws.onopen = res; c.ws.onerror = () => rej(new Error('ws error'));
      setTimeout(() => rej(new Error('ws open timeout')), 5000);
    });
    return c;
  }
  send(method, params = {}, sessionId, ms = PAGE_TIMEOUT) {
    return new Promise((resolve, reject) => {
      const id = ++this.id;
      this.pend.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params, sessionId }));
      setTimeout(() => { if (this.pend.delete(id)) reject(new Error(`${method} timeout`)); }, ms);
    });
  }
  close() { try { this.ws.close(); } catch {} }
}

// A tab's last activity, or null when it doesn't answer.
const activity = async (cdp, sid) => {
  try { return (await cdp.send('Runtime.evaluate', { expression: ACTIVITY_JS, returnByValue: true }, sid)).result.value || Date.now(); }
  catch { return null; }
};
const alive = async (cdp, sid) => (await activity(cdp, sid)) !== null;

function memUsage() {
  try {
    const max = Number(readFileSync('/sys/fs/cgroup/memory.max', 'utf8'));
    const anon = Number(/^anon (\d+)/m.exec(readFileSync('/sys/fs/cgroup/memory.stat', 'utf8'))[1]);
    return max ? anon / max : 0;
  } catch { return 0; }
}

// Target.closeTarget closes without beforeunload (editors set one); Page.navigate would hit the dialog.
const closeTab = (cdp, t) => cdp.send('Target.closeTarget', { targetId: t.targetId }).catch(() => {});

async function closeIdle(cdp, live) {
  const mem = memUsage();
  const pressure = mem > MEM_PRESSURE;
  const limit = pressure ? IDLE_PRESSURE_MS : IDLE_MS;
  const now = Date.now();
  const idle = live.filter((p) => now - p.act > limit).sort((a, b) => a.act - b.act);
  if (!idle.length) return;
  const keep = live.length === idle.length ? idle.pop() : null; // one tab always stays
  for (const p of pressure ? idle.slice(0, 1) : idle) {
    await closeTab(cdp, p.t);
    log(`closed an idle tab (${Math.round((now - p.act) / 60000)} min${pressure ? `, memory ${Math.round(mem * 100)}%` : ''}): ${p.t.url}`);
  }
  if (keep && keep.t.url !== 'about:blank' && !pressure) {
    await cdp.send('Target.createTarget', { url: 'about:blank' }).catch(() => {});
    await closeTab(cdp, keep.t);
    log(`replaced the last tab with about:blank (${Math.round((now - keep.act) / 60000)} min): ${keep.t.url}`);
  }
}

async function waitAlive(cdp, sid, ms) {
  for (const end = Date.now() + ms; Date.now() < end; await sleep(1000)) if (await alive(cdp, sid)) return true;
  return false;
}

// A tab didn't answer two ticks in a row: alert -> reload -> recreate.
async function repair(cdp, t, sid) {
  try {
    // Without Page.enable on this session CDP won't dismiss the dialog.
    await cdp.send('Page.enable', {}, sid, 3000).catch(() => {});
    await cdp.send('Page.handleJavaScriptDialog', { accept: false }, sid, 3000);
    if (await waitAlive(cdp, sid, 3000)) return log(`dismissed a stuck dialog: ${t.url}`);
  } catch { /* no dialog */ }
  try {
    await cdp.send('Page.reload', {}, sid, 10000);
    if (await waitAlive(cdp, sid, 20000)) return log(`reloaded a dead tab: ${t.url}`);
  } catch (e) { log(`reload failed (${e.message}): ${t.url}`); }
  // The new one first, then close the old: if it is the last, the window must not close.
  await cdp.send('Target.createTarget', { url: t.url || 'about:blank', background: true }).catch(() => {});
  await closeTab(cdp, t);
  log(`recreated a tab: ${t.url}`);
}

const strikes = new Map(); // targetId -> missed answers in a row
let browserFails = 0;

async function checkBrowser() {
  let v;
  try { v = await fetchJson(`${CDP}/json/version`, 5000); browserFails = 0; }
  catch {
    browserFails++; log(`CDP doesn't answer (${browserFails}/3)`);
    if (browserFails >= 3) { killPid('/tmp/chromium.pid', 'chromium'); browserFails = 0; strikes.clear(); await sleep(30000); }
    return;
  }
  const cdp = await Cdp.open(v.webSocketDebuggerUrl);
  try {
    const { targetInfos } = await cdp.send('Target.getTargets');
    const pages = targetInfos.filter((t) => t.type === 'page' && !/^(chrome|devtools):/.test(t.url));
    for (const id of strikes.keys()) if (!pages.some((t) => t.targetId === id)) strikes.delete(id);
    const live = [], sids = [];
    await Promise.all(pages.map(async (t) => {
      try {
        const { sessionId: sid } = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true });
        sids.push(sid);
        const act = await activity(cdp, sid);
        if (act !== null) { strikes.delete(t.targetId); live.push({ t, sid, act }); return; }
        const n = (strikes.get(t.targetId) || 0) + 1;
        strikes.set(t.targetId, n);
        log(`a tab doesn't answer (${n}/2): ${t.url}`);
        if (n >= 2) { strikes.delete(t.targetId); await repair(cdp, t, sid); }
      } catch (e) { log(`checking a tab: ${e.message}: ${t.url}`); }
    }));
    // Close idle tabs only when every tab answered: no cleaning up during a failure.
    if (live.length === pages.length) await closeIdle(cdp, live);
    await Promise.all(sids.map((s) => cdp.send('Target.detachFromTarget', { sessionId: s }).catch(() => {})));
  } finally { cdp.close(); }
}

async function mcpCall(sid, body, ms) {
  const r = await fetch(MCP, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', ...(sid && { 'mcp-session-id': sid }) },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(ms),
  });
  return { sid: r.headers.get('mcp-session-id'), text: await r.text() };
}

async function checkMcp() {
  let sid;
  try {
    ({ sid } = await mcpCall(null, { jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'guard', version: '1' } } }, 10000));
    if (!sid) throw new Error('no mcp-session-id');
    await mcpCall(sid, { jsonrpc: '2.0', method: 'notifications/initialized' }, 5000);
    const { text } = await mcpCall(sid, { jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'browser_tabs', arguments: { action: 'list' } } }, 45000);
    if (!text.includes('### Result') || text.includes('"isError":true')) throw new Error(text.replace(/\s+/g, ' ').slice(0, 300));
    return true;
  } catch (e) { log('the MCP is not working:', e.message); return false; }
  finally { if (sid) fetch(MCP, { method: 'DELETE', headers: { 'mcp-session-id': sid }, signal: AbortSignal.timeout(5000) }).catch(() => {}); }
}

let mcpFails = 0;
log('start');
await sleep(60000);
for (let tick = 0; ; tick++) {
  try { await checkBrowser(); } catch (e) { log('checkBrowser:', e.message); }
  if (tick % MCP_EVERY === 0) {
    if (await checkMcp()) {
      if (mcpFails) log('the MCP works again');
      mcpFails = 0;
    } else {
      mcpFails++;
      if (mcpFails === 2) killPid('/tmp/mcp.pid', 'MCP');
      if (mcpFails === 4) killPid('/tmp/chromium.pid', 'chromium');
      if (mcpFails >= 6) { log('nothing helped — exiting, Docker restarts the container'); process.exit(1); }
    }
  }
  await sleep(TICK);
}
