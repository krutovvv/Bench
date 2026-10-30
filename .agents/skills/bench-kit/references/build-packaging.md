# macOS packaging, permissions, logs

Working script: `scripts/bundle-macos.sh` — builds `/Applications/Bench.app` (id
`local.bench.app`), what `.agents/skills/bench-kit/scripts/ship.sh` runs. There is no other build.

## What the bundle needs

1. `cargo build --release`, copy the binary to `Contents/MacOS/`.
2. App icon: render `assets/logo.svg` with `magick` at 1024, resize to every iconset size,
   `iconutil -c icns` → `Contents/Resources/AppIcon.icns`.
3. `Info.plist`: `CFBundleIdentifier` (stable: `local.bench.app`), `CFBundleExecutable`,
   `CFBundleIconFile`, `NSHighResolutionCapable`, `LSMinimumSystemVersion`, and a usage string
   for **every** protected resource the app touches:
   - `NSMicrophoneUsageDescription` (dictation),
   - `NSLocalNetworkUsageDescription` (anything on 192.168.x.x / *.local, including SSH spawned
     by the app).
4. **Sign** with the Apple Development identity from the keychain (`BENCH_SIGN=<identity>`
   picks another): `codesign --force --sign "$SIGN" --identifier "$ID"`; without one, ad-hoc
   (`--sign -`) with the same stable identifier — grants are then asked again after every build.
5. **Its own Mach-O UUID per app** (from the linker's and the bundle id), so Dev and release
   built from one binary are not twins to local network privacy.

## Why the identifier matters (the LAN bug)

macOS privacy grants (Local Network, Microphone) are tied to the app's code identity. A default
linker ad-hoc signature has an identifier like `<crate>-<hash>` that changes on every build, so
each rebuild is a "new app": the earlier "Allow" doesn't apply and Local Network access is
silently denied. Symptom: SSH to LAN hosts fails with exit code 255 from the `.app`, while the
same binary started from Terminal (which has its own grant) works; public IPs still work.
Fix: the stable `--identifier` plus `NSLocalNetworkUsageDescription`. After that the prompt
appears once and the grant survives rebuilds.

Diagnose like this: make the app append each host check result to a log file in its state dir,
ship it, have the person open it, read the log.

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

- The project is `~/Projects/bench` (git repo; `.gitignore` covers `target/`, `dist/`,
  `.DS_Store`, `._*`, editor junk — Finder files once blocked a merge).
  Never ignore `Icon?`: git on macOS matches case-insensitively, so it hides every new file in
  `assets/icons/`; the folder-icon file is `Icon\r` — write the pattern with literal CRs.
- The loop after each change is `SKILL.md`'s. Grants follow the signature: with the Apple Development
  certificate they belong to team + bundle id and survive rebuilds; ad-hoc signing ties them to
  one build's hash, and every build becomes a new entry in Privacy & Security.
- A running copy keeps running if its bundle is replaced — replacing the bundle alone changes
  nothing the user can see.
- A Finder-launched app's PATH has no Homebrew: look for tools like ffmpeg in
  `/opt/homebrew/bin`, `/usr/local/bin` yourself.
