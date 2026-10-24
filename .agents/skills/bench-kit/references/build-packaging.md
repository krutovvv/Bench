# macOS packaging, permissions, logs

Working script: `templates/app/scripts/bundle-macos.sh` (release build → `dist/<App>.app`).

## What the bundle needs

1. `cargo build --release`, copy the binary to `Contents/MacOS/`.
2. App icon: render `assets/logo.svg` with `qlmanage -t -s 1024`, resize with `sips` to every
   iconset size, `iconutil -c icns` → `Contents/Resources/AppIcon.icns`.
3. `Info.plist`: `CFBundleIdentifier` (stable, e.g. `local.<app>`), `CFBundleExecutable`,
   `CFBundleIconFile`, `NSHighResolutionCapable`, `LSMinimumSystemVersion`, and a usage string
   for **every** protected resource the app touches:
   - `NSMicrophoneUsageDescription` (dictation),
   - `NSLocalNetworkUsageDescription` (anything on 192.168.x.x / *.local, including SSH spawned
     by the app).
4. **Sign with a stable identifier**: `codesign --force --sign - --identifier local.<app> "$APP"`.

## Why the identifier matters (the LAN bug)

macOS privacy grants (Local Network, Microphone) are tied to the app's code identity. A default
linker ad-hoc signature has an identifier like `<crate>-<hash>` that changes on every build, so
each rebuild is a "new app": the earlier "Allow" doesn't apply and Local Network access is
silently denied. Symptom: SSH to LAN hosts fails with exit code 255 from the `.app`, while the
same binary started from Terminal (which has its own grant) works; public IPs still work.
Fix: the stable `--identifier` plus `NSLocalNetworkUsageDescription`. After that the prompt
appears once and the grant survives rebuilds.

Diagnose like this: make the app append each host check result to a log file in its state dir,
launch the bundle with `open -n "dist/<App>.app"` (a second instance — `open` alone just focuses
an already running copy), read the log.

## Finder-launched process environment

- `PATH` is minimal (`/usr/bin:/bin:/usr/sbin:/sbin`): call tools by absolute path or ones that
  live there (`/usr/bin/ssh`, `/usr/bin/git`); for user tools use `$HOME/.local/bin/<tool>` or
  `bash -lc '…'` over SSH.
- No visible stderr: write errors to `~/Library/Application Support/<app>/<thing>.log` with a
  timestamp and the failing call's name.
- Touching `~/Desktop`, `~/Documents`, `~/Downloads` triggers a privacy prompt — don't probe
  those folders speculatively (e.g. running `git rev-parse` in a default working dir); only act
  on folders the user chose.

## Shipping to the user

- Keep the project in `~/Projects/<app>` (git repo; `.gitignore` covers `target/`, `dist/`,
  `.DS_Store`, `._*`, editor junk — Finder files once blocked a merge).
  Never ignore `Icon?`: git on macOS matches case-insensitively, so it hides every new file in
  `assets/icons/`; the folder-icon file is `Icon\r` — write the pattern with literal CRs.
- After each change: `cargo fmt`, build, commit, then `ship.sh` (a new `dist/Bench.<commit>.app`
  opened beside the running one — never quit the running one). Moving the project folder is
  fine; the grants follow the identifier, which every build shares (`local.<crate>`).
- A running copy keeps running if its bundle is replaced — replacing the bundle alone changes
  nothing the user can see.
- A Finder-launched app's PATH has no Homebrew: look for tools like ffmpeg in
  `/opt/homebrew/bin`, `/usr/local/bin` yourself.
