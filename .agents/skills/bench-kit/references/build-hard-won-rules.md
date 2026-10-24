# Hard-won rules (each cost a round of user feedback)

- Spacing complaints are about 1–3 pt. Measure both sides (`measure.py`), then fix the exact
  difference. Text glyphs have side bearing: an input's text inset that "looks" 1pt off usually is.
- gpui-kit `Input` ignores `.h(...)`; its height comes from its size (Medium = `h_8` = 2rem =
  28px at 14px rem). Make neighbours match the input (2 rems), not the other way round.
- A scroll container ignores the last child's bottom margin → add a real spacer element.
- Hover groups need **unique** names per row (`format!("row-{i}")`), or every row reacts.
- The OS blocks a Finder-launched app from the LAN unless the bundle has
  `NSLocalNetworkUsageDescription` **and a stable code-signing identifier**; otherwise SSH to
  192.168.x.x fails with exit 255 while the same binary works from Terminal.
- A Finder-launched app has no stderr: log errors to a file in
  `~/Library/Application Support/<app>/`.
- When patching files with scripts, never write back a stale buffer read before another edit of
  the same file (that silently reverted a whole feature once). Re-read, or use exact edits.
- gpui-kit pins a pre-release GPUI fork (`gpui-pre`, `gpui-base`): Zed-head and tutorial snippets
  use a diverged API. Take code from `references/ui-content.md` and from Bench first. When
  one exact name is missing there, grep the registry for that one name
  (`grep -rn "pub fn cursor_position" ~/.cargo/registry/src/*/gpui-base-0.8.0/src`) — don't read
  the sources to learn the framework.
- Don't fake data the backend doesn't send: compute it yourself transparently (e.g. tokens/s from
  streamed text + calibration) and say it is an estimate (`~`).
- A button that "doesn't work" usually got an error nobody showed: read the app's error log,
  surface the backend's real reason in the toast (CLI tools often print refusals on stdout).
- A child process that may leave a daemon behind (`ssh -f`/ControlPersist, CLIs that start a
  background worker) keeps an inherited pipe open forever: `.output()` then never returns.
  Send its stdout/stderr to a file (or /dev/null) and wait on the direct child only.
- Talk to servers through one shared ssh connection (`ControlMaster=auto`,
  `ControlPath=~/.ssh/cm/%C`, `ControlPersist=600`): the terminal becomes the master, file
  panels and agent tools reuse it — one login, instant opens. Never change the user's ssh config.
- Ask before anything that can kill the user's running work (restarting their backend, pressing
  Stop in their live session) — test those on throwaway state.
