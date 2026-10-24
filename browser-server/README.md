# The server browser

A browser on your own server for Bench's **browser threads**. The agent drives a real Chrome
through the [Playwright MCP](https://github.com/microsoft/playwright-mcp); in Bench every step the
agent takes there shows as the page itself, live, inside its answer, where you can click and type
to step in. Sign-ins stay in the browser's profile, so a site you signed in to once stays signed
in for the agent and for everyone who works with it.

One container, three processes and a guard:

| Process | Port | For | Access |
|---|---|---|---|
| Chromium (headed, on Xvfb) | 9222, inside only | the pages, the profile in `/data/profile` | — |
| Playwright MCP | 8931 → `https://<host>/mcp` | the agent | basic auth |
| `live.mjs`, the live view | 8932 → `wss://<host>/live` | the people in Bench | a personal key each |
| `guard.mjs` | — | repairs what hangs, closes idle tabs | — |

The live view never exposes the raw DevTools protocol (it can read cookies and run code): it
passes only the tabs, watching one, opening, closing, the mouse, the wheel, typing and keys, and
it writes every operation to `/data/live-audit.log`.

## What you need

- A server with Docker. **2 GB of RAM is enough**: the container is capped at 1 GB.
- A domain name pointing at it.
- [Traefik](https://traefik.io) on a Docker network (`proxy` by default) with an entrypoint
  `websecure` and a certificate resolver `le` — that is what `docker-compose.yml` labels for. Any
  other TLS proxy works too: send `https://<host>/` to `:8931` behind basic auth, and `/live`
  (a WebSocket) to `:8932` without it.

## Set it up

```bash
git clone https://github.com/krutovvv/bench.git
cd bench/browser-server
cp .env.example .env        # the host, the MCP's password (htpasswd -nb agent <password>), the language
docker compose up -d --build
curl https://browser.example.com/live/health    # ok
```

Give each person their own key for the live view (it prints once; only its hash is kept):

```bash
docker compose exec browser node /opt/live.mjs add anna
docker compose exec browser node /opt/live.mjs list
docker compose exec browser node /opt/live.mjs remove anna
```

## Connect Bench

**The live view**, on each person's Mac, in `~/.bench/ui/browser.json`:

```json
{ "url": "wss://browser.example.com/live", "token": "<their key>" }
```

**The agent**: add the browser to its MCP servers, `~/.bench/agents/<agent>/mcp.json`:

```json
{
  "mcpServers": {
    "browser": {
      "url": "https://browser.example.com/mcp",
      "headers": { "Authorization": "${BROWSER_MCP_AUTH}" },
      "description": "The server browser; Bench shows its tabs live in browser threads.",
      "exposure": "deferred",
      "toolExposure": {
        "browser_tabs": "direct", "browser_navigate": "direct", "browser_snapshot": "direct",
        "browser_click": "direct", "browser_type": "direct", "browser_fill_form": "direct",
        "browser_press_key": "direct", "browser_wait_for": "direct"
      }
    }
  }
}
```

and the password as a service key (Bench: Settings → Providers → Service keys):
`BROWSER_MCP_AUTH` = `Basic <base64 of agent:password>` (`printf 'agent:password' | base64`).

Then the **+** next to Browser in Bench's sidebar starts a browser thread. To sign in to a site
once, ask the agent to open it, and sign in yourself right in the page shown in the thread.

## Kept light

The view is for watching and stepping in, not for browsing, and one small container serves a team:

- **At most two frames a second**, and only while someone watches: the gateway holds each
  frame's acknowledgement for `LIVE_FRAME_GAP_MS` (500 ms); Bench stops watching when nobody has
  touched the thread for a minute and the agent is idle.
- **A memory budget.** A heavy tab costs 200–450 MB. At most `LIVE_TABS` (3) tabs stay awake; the
  least recently touched one nobody watches falls asleep — a screenshot and its address are kept,
  the tab is closed, and it opens again when someone clicks it. Past `LIVE_HARD_TABS` (5) tabs in
  use, opening another answers "busy" instead of the container running out of memory.
- **Ads and counters are blocked** in every tab (`LIVE_BLOCK`): on news sites they are most of
  the weight.
- **The guard** (`guard.mjs`) pings the browser, every tab and the MCP end to end; it dismisses a
  stuck dialog, reloads or recreates a dead tab, restarts the MCP or the browser, and as a last
  resort exits so Docker restarts the container. It also closes tabs idle for `TAB_IDLE_MIN`
  minutes (sooner under memory pressure).
- Chromium runs without site-per-process (one renderer per page), with `RENDERER_LIMIT` (4).

**More people**: run several containers (one profile each) behind the same two endpoints and pin
each thread to one; or give each person a browser context of their own when sign-ins must differ.

## Files

| File | What |
|---|---|
| `Dockerfile` | Alpine, Chromium, Xvfb, `@playwright/mcp` and `ws` pinned |
| `entrypoint.sh` | starts Xvfb, then Chromium, the MCP and the live view, each in a restart loop, and the guard |
| `init-page.js` | every MCP tab looks like an ordinary Chrome (user agent and Client Hints agree) |
| `live.mjs` | the live view: keys, the protocol, the memory budget, the audit log |
| `guard.mjs` | the guard |
| `docker-compose.yml`, `.env.example` | the service, its limits and Traefik routes |
