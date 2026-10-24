#!/bin/bash
set -u
W=${SCREEN_W:-1440}; H=${SCREEN_H:-900}
LANG_TAG=${BROWSER_LANG:-en-US}
# Introduce ourselves as an ordinary Chrome on Windows (the real major version, so it matches the
# engine): many sites treat a headless-looking Linux browser as a bot.
MAJOR=$(chromium --version | grep -oE "[0-9]+" | head -1)
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${MAJOR}.0.0.0 Safari/537.36"
mkdir -p /data/profile /data/output
rm -f /tmp/.X99-lock /data/profile/Singleton*

Xvfb :99 -screen 0 ${W}x${H}x24 -nolisten tcp &
for i in $(seq 50); do [ -e /tmp/.X11-unix/X99 ] && break; sleep 0.1; done

# The browser never stays down: if it exits, it starts again with the same profile (sign-ins).
# Without site-per-process a page and its frames share one renderer: much less memory.
(while true; do
  chromium --no-sandbox --disable-dev-shm-usage --disable-gpu \
    --no-first-run --no-default-browser-check --disable-features=Translate,MediaRouter,IsolateOrigins,site-per-process \
    --renderer-process-limit=${RENDERER_LIMIT:-4} --lang=${LANG_TAG} \
    --user-data-dir=/data/profile --user-agent="$UA" \
    --remote-debugging-address=127.0.0.1 --remote-debugging-port=9222 \
    --window-position=0,0 --window-size=${W},${H} --start-maximized ${START_URLS:-about:blank} &
  echo $! > /tmp/chromium.pid
  wait $!
  echo "chromium exited ($?), restarting in 2s"; sleep 2
done) &

for i in $(seq 100); do curl -sf http://127.0.0.1:9222/json/version >/dev/null && break; sleep 0.3; done

# The agent's channel: the Playwright MCP over HTTP, attached to that browser. The guard may kill
# it when it hangs; the loop brings it back.
(while true; do
  node /usr/local/lib/node_modules/@playwright/mcp/cli.js \
    --port 8931 --host 0.0.0.0 --allowed-hosts '*' \
    --cdp-endpoint http://127.0.0.1:9222 \
    --init-page /opt/init-page.js \
    --output-dir /data/output --output-max-size 50000000 &
  echo $! > /tmp/mcp.pid
  wait $!
  echo "mcp exited ($?), restarting in 2s"; sleep 2
done) &

# The people's channel: the live view of the tabs for Bench (/live, personal keys, watching and
# input only) — see live.mjs.
(while true; do
  node /opt/live.mjs &
  echo $! > /tmp/live.pid
  wait $!
  echo "live exited ($?), restarting in 2s"; sleep 2
done) &

# The guard checks the browser, every tab and the MCP end to end, and repairs what hangs (see
# guard.mjs). It is the main process: if it exits, Docker restarts the container.
exec node /opt/guard.mjs
