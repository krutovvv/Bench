---
name: bench-repo
description: "Use before a push or for the repository's face: README, pictures, About, web demo. No binaries."
---

# The repository on GitHub

Bench is a personal project published as source to start from. Deep alpha, not a product. There are
no releases, no prebuilt binaries and no build instructions for people: do not add them back. The
person's own app is `scripts/bundle-macos.sh`, which publishes nothing.

## Where is what

| What | Where | How it changes |
|---|---|---|
| Description, website, topics (the About box) | GitHub settings | `gh repo edit` (the text is below) |
| Social preview (the picture in links) | `assets/social-preview.png` | by hand: GitHub → Settings → General → Social preview; no command does it, so ask the person |
| The text people read first | `README.md` | an edit |
| The README's pictures | `assets/shots/` | `scripts/readme-shots.sh`, a list of demo states |
| Web demo | `scripts/build-web.sh` into `web/dist` (ignored by git) | a push: `.github/workflows/web-demo.yml` publishes it to the `gh-pages` branch, served at https://krutovvv.github.io/Bench/ |
| The repository's address in the app | `src/app/menu.rs` (Help menu), `agent/src/runtime/runtime.ts` (attribution) | an edit, only if the repository moves |

The About box:

- Description: `Native desktop app for coding with an AI agent on a local LLM, tuned to my own hardware. Rust + GPUI, agent on Pi Durable. Deep alpha, no releases: source to fork and change with your own agent.`
- Website: `https://krutovvv.github.io/Bench/`
- Topics, at most 20: `local-llm`, `local-ai`, `coding-agent`, `ai-coding-agent`, `ai-agent`, `ai-agents`, `agent-ui`, `llm-client`, `gpui`, `rust`, `rust-gui`, `native-app`, `desktop-app`, `macos`, `openai-compatible`, `qwen`, `pi-durable`, `boilerplate`, `agent-skills`, `personal-software`

GitHub's search reads the name, the description, the topics and the README. Wide topics
(`local-llm`, `coding-agent`) bring the right people; narrow ones (`agent-ui`, `llm-client`,
`rust-gui`, `gpui`) are where Bench can be seen at all.

## What the README says

A reader comes to decide whether this is for them, and leaves if it is not. Help them decide fast.

- The first two lines: what it is, in the words people search for (desktop app, AI agent, local LLM,
  Rust, GPUI), then that it is deep alpha with nothing to download.
- Next: "Is this for you?", with who it may suit and who it will not.
- Plain, short sentences in the first person. No lecture and no selling. A detail that only someone
  changing the code needs goes in a `SPEC.md`.
- Names: only the stack (Pi Durable, GPUI, Zed) and the model the person runs. No AI products or
  companies, no other apps. Nothing promised that the code does not do: check it in the module's
  `SPEC.md`.

## Before a push

1. PRIVATE. Run this and read what it prints:
   `git diff origin/main..main | grep -E '^\+' | grep -nIE '/Users/[a-z]|sk-[A-Za-z0-9]{16,}|ghp_[A-Za-z0-9]{10,}'`
   Also search that diff and `git log origin/main..main` for the person's company and client names.
   They are in the agent's memory and must never be written in this repository, this page included.
   Demo fixtures (`src/demo/`) are made up and fine. Output: nothing found, or what you removed.
2. PICTURES. If what the README shows has changed: `scripts/readme-shots.sh`, then read each picture.
3. CHECKS. The ones in `AGENTS.md`, and `cargo check --release --target wasm32-unknown-unknown` for
   the web demo. Another agent may be working in this folder: its unfinished edits can fail a check,
   so look at which files the errors are in.
4. ASK. "Пушу N коммитов?" A push is never made without a yes. A yes to one push is not a yes to the next.
5. PUSH. `git push origin main`.
6. RESULT. `gh run list --workflow web-demo.yml --limit 1` is green, and the demo opens at its address.

## A real example

The pass of 2026-10-31. The diff check found only `192.168.1.100` and `homelab`, which are made-up
fixtures in `src/demo/`, so nothing was removed. The first check of the web demo failed on another
agent's unfinished edits and passed once they were committed. The pictures were retaken by the script,
the README was rewritten, and the desktop builds and the release job left the workflow.
