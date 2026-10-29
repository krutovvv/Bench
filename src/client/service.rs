//! The agent as its machine's service, the same on every machine: launchd on macOS
//! (`~/Library/LaunchAgents/local.bench.agent.plist`), systemd on Linux
//! (`~/.config/systemd/user/bench-agent.service`, lingering so it runs without a login). The
//! service starts it with the machine, starts it again when it dies, and its restart ends the
//! agent's whole process tree. Bench never starts the agent itself: `launch.rs` installs the
//! sources and runs these scripts there (in a local shell or over SSH), then connects. Closing
//! Bench leaves every agent running.

/// What the scripts below need first: the service's file (`$FILE`), its text (`unit`) and the
/// service manager's verbs (`loaded`, `load`, `unload`, `again`). Run after `launch::PRELUDE`
/// (`$H`, `$NODE`).
const SERVICE: &str = r#"
LABEL=local.bench.agent
UNIT=bench-agent
AGENT_PATH="$(dirname "$NODE"):$HOME/.cargo/bin:$PATH"
case "$(uname)" in
Darwin)
	DOMAIN="gui/$(id -u)"
	# Over SSH to a Mac nobody is logged in to, there is no GUI domain.
	launchctl print "$DOMAIN" >/dev/null 2>&1 || DOMAIN="user/$(id -u)"
	FILE="$HOME/Library/LaunchAgents/$LABEL.plist"
	unit() { cat <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key><string>$LABEL</string>
	<key>ProgramArguments</key><array><string>$NODE</string><string>src/server.ts</string></array>
	<key>WorkingDirectory</key><string>$H/app</string>
	<key>EnvironmentVariables</key><dict><key>PATH</key><string>$AGENT_PATH</string><key>BENCH_HOME</key><string>$H</string></dict>
	<key>StandardOutPath</key><string>$H/server.log</string>
	<key>StandardErrorPath</key><string>$H/server.log</string>
	<key>RunAtLoad</key><true/>
	<key>KeepAlive</key><true/>
</dict>
</plist>
EOF
	}
	loaded() { launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1; }
	# A bootout just made may still be finishing: bootstrap is tried again for a few seconds.
	load() {
		for i in 1 2 3 4 5 6 7 8 9 10; do launchctl bootstrap "$DOMAIN" "$FILE" 2>/dev/null && return 0; sleep 0.5; done
		launchctl bootstrap "$DOMAIN" "$FILE"
	}
	unload() { launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true; }
	again() { launchctl kickstart -k "$DOMAIN/$LABEL"; }
	;;
Linux)
	export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
	if ! systemctl --user show-environment >/dev/null 2>&1; then
		echo "Bench's agent runs as a systemd user service, and this machine has none" >&2; exit 4
	fi
	[ "$(loginctl show-user "$(id -un)" -p Linger --value 2>/dev/null)" = yes ] || loginctl enable-linger 2>/dev/null || true
	FILE="$HOME/.config/systemd/user/$UNIT.service"
	unit() { cat <<EOF
[Unit]
Description=Bench's agent
StartLimitIntervalSec=0

[Service]
WorkingDirectory=$H/app
ExecStart="$NODE" src/server.ts
Environment="PATH=$AGENT_PATH" "BENCH_HOME=$H"
StandardOutput=append:$H/server.log
StandardError=append:$H/server.log
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
EOF
	}
	loaded() { systemctl --user is-active --quiet "$UNIT"; }
	load() { systemctl --user daemon-reload && systemctl --user enable --quiet "$UNIT" && systemctl --user restart "$UNIT"; }
	unload() { systemctl --user disable --now --quiet "$UNIT" 2>/dev/null || true; }
	again() { systemctl --user restart "$UNIT"; }
	;;
*)
	echo "Bench's agent runs as a launchd or systemd service, and $(uname) has neither" >&2; exit 4
	;;
esac
pid_of() { sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$H/server.json" 2>/dev/null; }
build_of() { sed -n 's/.*"build":"\([^"]*\)".*/\1/p' "$H/server.json" 2>/dev/null; }
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
# The agent `server.json` names, when it is not of this build: one the service did not start
# (by hand, or by a Bench from before the service) holds the home, and the service's would wait.
stop_other() {
	p="$(pid_of)"
	if alive "$p" && [ "$(build_of)" != "@BUILD@" ]; then
		kill "$p"
		for i in $(seq 1 20); do alive "$p" || break; sleep 0.5; done
	fi
}
"#;

/// Makes the service match this build and waits for its agent: `server.json` on stdout. The
/// service is (re)loaded when its file changed, started when it is not running, and restarted
/// when it runs other sources — its runs are durable, so one it was in resumes.
const START: &str = r#"
mkdir -p "$(dirname "$FILE")"
unit > "$FILE.new"
if ! cmp -s "$FILE.new" "$FILE"; then
	unload
	stop_other
	mv "$FILE.new" "$FILE"
	load
else
	rm -f "$FILE.new"
	if ! loaded; then stop_other; load
	elif [ "$(build_of)" != "@BUILD@" ]; then again; stop_other
	fi
fi
for i in $(seq 1 120); do
	if alive "$(pid_of)" && [ "$(build_of)" = "@BUILD@" ]; then cat "$H/server.json"; exit 0; fi
	sleep 0.5
done
echo "the agent did not start: $(tail -n 3 "$H/server.log" 2>/dev/null | tr '\n' ' ')" >&2
exit 1
"#;

/// Restarts the service (the agent pressed Restart): it ends the agent's process tree and starts
/// it again.
const RESTART: &str = r#"
if loaded; then again; else load; fi
"#;

/// The script that brings the agent of `build` up as its machine's service.
pub(super) fn start(build: &str) -> String {
    format!("{SERVICE}{START}").replace("@BUILD@", build)
}

pub(super) fn restart() -> String {
    format!("{SERVICE}{RESTART}").replace("@BUILD@", "")
}
