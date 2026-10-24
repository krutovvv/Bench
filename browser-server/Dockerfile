# The server browser for Bench's browser threads: a headed Chromium (on Xvfb), the Playwright MCP
# for the agent, and the /live gateway for the people who watch and step in.
# Chromium runs on its own and the MCP attaches over CDP, so tabs and sign-ins survive clients
# reconnecting and the MCP restarting.
FROM node:22-alpine
RUN apk add --no-cache chromium xvfb bash curl tini \
      ttf-dejavu font-noto-emoji \
 && npm i -g @playwright/mcp@0.0.83 ws@8.18.3 && npm cache clean --force \
 && mkdir -p /data && chown node:node /data
ENV PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 DISPLAY=:99 LANG=C.UTF-8
COPY --chmod=755 entrypoint.sh /entrypoint.sh
COPY init-page.js guard.mjs live.mjs /opt/
USER node
# 8931: the MCP (put it behind basic auth); 8932: /live (its own per-person keys).
EXPOSE 8931 8932
# A frequent start-interval: while the container is "starting", a proxy like Traefik won't route
# to it, so it should turn healthy quickly.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --start-interval=2s CMD curl -sf -m 4 http://127.0.0.1:9222/json/version >/dev/null || exit 1
ENTRYPOINT ["/sbin/tini", "--", "/entrypoint.sh"]
