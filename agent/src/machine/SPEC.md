---
id: agent-machine
type: submodule-design
status: draft
title: agent/src/machine — where the tools work
parent: agent
references: [projects-and-agents]
---

# The machine

Gives each conversation the `ExecutionEnv` (Pi Durable's files and shell) its tools work
through, so Pi's read, write, edit and bash and our grep, glob and ls work unchanged wherever the
project is. `runtime/tools-host.ts` `envFor` picks it per conversation; why a project's work can
be on another machine than its agent is `projects-and-agents` (`src/app/SPEC.md` §5).

## The three environments

- **This machine**: Pi's own `NodeExecutionEnv`.
- **A server over SSH** (`ssh.ts`, `SshExecutionEnv`): one OpenSSH connection per server with
  `ControlMaster` (its socket in `~/.bench/ssh/`), so every command and file operation rides it
  without a new handshake. The connection is the alias's block in `~/.ssh/config`, never
  duplicated. Commands run through `NodeExecutionEnv` as `ssh … host '<script>'`, keeping its
  timeout, abort and output window; files are POSIX commands over the same connection. The
  server needs only `sh` (and GNU or BSD `stat`) — nothing is installed there. Pi's
  `ExecutionEnv` conformance suite is its test (`test/ssh-env.test.ts`).
- **Kept in its folder** (`confined.ts`), wrapping either of the above: file tools are refused
  any path outside the folder, reading included; commands run in the system's sandbox with
  writing allowed only in the folder and the temp folders (`sandbox-exec` on macOS, bubblewrap
  on Linux when installed).

`sh.ts` runs one POSIX script in a folder of the project's machine — over the same connection, or
here — for what reads it in one go (a turn's git trees, a review's diff, the specs, the Explorer's
searches); its failures are plain errors, the routes saying what they mean in HTTP.

`settings.ts` routes the project's own settings files (`SYSTEM.md`, `mcp.json`, skills, prompts)
to this agent's machine wherever the work is, so the agent can change its environment from any
thread; every other path and every command goes to the conversation's environment.

## Known limitations

- Confinement checks paths as written (`..` resolved); a symbolic link inside the folder that
  points out is not caught, and a command may still read outside.
- On Linux without bubblewrap, commands run unsandboxed in the folder and only the prompt states
  the rule.
