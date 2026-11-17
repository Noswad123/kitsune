---
name: kitsune-throwaway-repro
description: Create and control a disposable named Kitsune session from inside an existing Kitsune session. Use for isolated Kitsune runtime, pane, terminal, process, API, persistence, or agent reproductions that should be driven through the CLI/API without touching the default session.
---

# Kitsune throwaway reproduction

Use a disposable named Kitsune session when a reproduction needs a real Kitsune server, panes, PTYs, agents, or socket API without risking the user's main session.

The temporary TUI only keeps the disposable session attached and supplies terminal geometry. Drive the reproduction from the parent session through Kitsune's CLI/API. Do not manually operate the nested TUI unless the bug specifically requires client input.

## Non-negotiable safety

- Never run the reproduction in the default session.
- Never stop, restart, delete, or kill the main Kitsune server.
- Never use `pkill`, broad process matching, or guessed PIDs for cleanup.
- Create a unique session name. Never reuse or delete an unrelated named session.
- Create a new outer pane and close only that pane during cleanup.
- Read workspace, tab, pane and terminal IDs from command output. Never construct them or treat historical agent references as live targets.
- Use a verified disk-backed directory under `${XDG_CACHE_HOME:-$HOME/.cache}/agent-test-runs`, with a unique owned directory per run. Check filesystem backing and available space before substantial work; do not put bulk artifacts in `/tmp`, `/tmp/opencode` or `/dev/shm`.
- Register cleanup before starting any children, using fixture teardown, `try/finally` or exit/signal traps that preserve the failure status. Stop and wait/reap owned children before removing only that run's exact directory, including after failure or catchable interruption. Never clean pre-existing user data, other runs, shared caches or session history.
- Do not approve destructive or unnecessary agent actions.
- Do not spend paid agent tokens without the user's approval. Use the requested low-cost model and the smallest useful prompts.

## Learn the installed interface first

The installed binary is the authority. CLI syntax may have changed since this skill was written.

Confirm the caller is inside Kitsune and inspect the relevant help before doing anything:

```bash
test "${KITSUNE_ENV:-}" = 1
kitsune --version
kitsune --help
kitsune session
kitsune pane
```

Inspect nested command help before using unfamiliar or potentially mutating commands. Do not run bare `kitsune` for discovery because it launches or attaches the TUI.

Record which Kitsune binary and version the reproduction tests. If testing a checkout build, follow the repository's instructions for running that build instead of silently substituting the installed binary.

## Create the outer pane

Create a sibling shell pane in the current tab without moving focus. Use an available Kitsune layout tool when the harness provides one. Otherwise use the installed pane split command after checking its help.

Use the unique owned run directory as the new pane's cwd. Save the returned outer pane ID. This is the only parent-session pane that cleanup may close.

## Start the disposable session

Choose a short unique name such as `repro-<topic>-<timestamp>`.

Prefer private HOME/XDG directories and TMPDIR inside the owned run, created after verifying the parent directory and disk backing. In the examples below, `<run-home>`, `<run-config>`, `<run-state>`, `<run-cache>` and `<run-tmp>` are absolute directories inside that exact run. Use the same private environment for the named server and every client command. Redirect tool-specific outputs there too; TMPDIR alone does not cover all tools or hardcoded paths. Reuse existing package/toolchain caches deliberately rather than downloading new copies. Use shared user configuration only for a user-approved shared-config reproduction.

Run the named session inside the new outer pane. Clear inherited session selection, socket overrides, and caller IDs so the nested runtime cannot accidentally address the parent session:

```bash
env \
  -u KITSUNE_SOCKET_PATH \
  -u KITSUNE_CLIENT_SOCKET_PATH \
  -u KITSUNE_SESSION \
  -u KITSUNE_WORKSPACE_ID \
  -u KITSUNE_TAB_ID \
  -u KITSUNE_PANE_ID \
  HOME=<run-home> \
  XDG_CONFIG_HOME=<run-config> \
  XDG_STATE_HOME=<run-state> \
  XDG_CACHE_HOME=<run-cache> \
  TMPDIR=<run-tmp> \
  kitsune --session <session-name>
```

Add reproduction-specific environment variables to this launch command when needed. Environment variables that configure the server must be present before the named server starts.

Do not continue until the named session's API is ready. Confirm readiness by addressing that session from the parent and listing its panes.

## Address only the disposable session

Every control command issued from the parent must clear inherited socket overrides and explicitly select the temporary session:

```bash
env \
  -u KITSUNE_SOCKET_PATH \
  -u KITSUNE_CLIENT_SOCKET_PATH \
  -u KITSUNE_WORKSPACE_ID \
  -u KITSUNE_TAB_ID \
  -u KITSUNE_PANE_ID \
  HOME=<run-home> \
  XDG_CONFIG_HOME=<run-config> \
  XDG_STATE_HOME=<run-state> \
  XDG_CACHE_HOME=<run-cache> \
  TMPDIR=<run-tmp> \
  KITSUNE_SESSION=<session-name> \
  kitsune pane list
```

Repeat this prefix for every command. Do not rely on shell state persisting between tool calls.

Read the disposable root pane ID from `pane list`. Confirm its cwd and foreground process before starting anything in it.

Named sessions isolate runtime state, sockets, panes and persistence, but still share global Kitsune configuration by default. Local manifest overrides matter only for retained static/offline compatibility evaluation, not live detection or reload. Check configuration provenance; prefer the private HOME/XDG environment above unless the user approves a shared-config reproduction. Never change shared configuration or overrides merely to make the test pass.

## Drive the reproduction through the API

Use pane commands for shells and ordinary processes:

- `pane run` to start a command at an available shell prompt.
- `pane wait-output` to wait for deterministic output.
- `pane read` to capture terminal contents.
- `pane send-text` for literal input.
- `pane send-keys` for supported keys.
- `pane get`, `pane process-info`, and `pane layout` for runtime state.

For explicit targeting, use `pane get <pane-id>`, `pane process-info --pane <pane-id>` and `pane layout --pane <pane-id>`. Prefix every command with the disposable-session environment above.

Live Kit agent controls, reads and status waits are globally retired, even without Coven. Use the generic pane commands above for explicitly authorized terminal operations; they do not recognize native agents, establish readiness/current AgentState, correlate turns or infer completion. Title/metadata echoes and process observations are data, not native identity or readiness. Prefer inert deterministic fixtures for generic multiplexer qualification. If native agent behavior is essential, use only the user's explicitly approved native invocation or a supported, explicitly approved Coven transport, after checking its actual contract/help. Do not invent Coven CLI arguments or substitute retired Kit agent controls.

Run the relevant command group's help first because names and options may change.

Prefer bounded `pane wait-output` checks for deterministic output over arbitrary sleeps; an output match is not provider readiness or task completion. When timing itself is under test, record timestamps and use bounded polling. Capture generic terminal/process observations before, during and after the transition being reproduced.

When a needed terminal key is unsupported by the high-level command, send its terminal sequence through the disposable pane only after confirming the target application's expected key. Never send raw control sequences to the parent pane.

## Start agents carefully

Launch a real AI provider only with the user's explicit provider/model approval; otherwise use inert fixtures. Before an approved launch, inspect its installed `--version` and `--help`. Submit an explicitly reviewed native command through generic `pane run` at an available shell prompt, or use an approved supported Coven transport. There is no live Kit agent-start argument-separator workflow. Generic `pane send-text`/`pane send-keys` send terminal input only and require explicit authorization, not inferred readiness.

Use the exact model requested or approved by the user. Inspect the native application's own model display rather than trusting an alias; record it as displayed data, not Kit recognition or native attestation. Prefer low effort, safe mode, and manual permissions for a baseline when the agent supports them. Repeat with the user's real configuration only with approval and when the suspected behavior depends on hooks, plugins, or settings.

Use harmless operations for permission-state testing. Reject the pending action after evidence is captured and verify that no artifact was created.

## Collect useful evidence

Record enough information for another person to repeat the result:

- Kitsune binary and version.
- Named session and launch environment.
- Target application or agent version and arguments.
- Exact commands or prompts.
- Generic pane/terminal/process observations before and after each transition; separately attribute any approved native/Coven evidence without inferring lifecycle from terminal output.
- Relevant `pane read`, `pane get`, `pane process-info --pane <pane-id>`, API output and session logs. Choose `--source visible` for the viewport, `recent` for recent wrapped output or `recent-unwrapped` for joined soft wraps. Keep `pane read <pane-id> --source detection --format text` for generic bottom-buffer captures, never agent inference; detection text and ANSI formats currently both return plain text, not styling evidence.
- Whether private or approved shared config was used. Record any local manifest override only as static/offline compatibility input. If explicitly needed, `kitsune agent explain --file <capture-path> --agent <kind> --json` is legacy static, non-live file explanation, not runtime detection or a reload workflow.

Read the named session directory and socket from `kitsune session list` in the same private environment instead of assuming their paths. Keep bounded evidence/logs in the verified disk-backed owned run. Retain artifacts only when explicitly requested or genuinely needed for diagnosis; report the exact path, reason and cleanup plan. Report cleanup failures or hardcoded-path limitations rather than silently leaving data.

Distinguish observed facts from proposed causes. First reproduce stock behavior, then change one variable at a time.

## Cleanup

Cleanup is part of the reproduction, including after failure or catchable interruption. Register it before starting children; preserve the reproduction's exit/failure status.

1. Reject pending prompts and stop test applications cleanly when practical.
2. Verify that harmless probe files or other test artifacts do not exist, or remove only artifacts created by this reproduction.
3. Stop the temporary named session with the installed session command.
4. Delete that same stopped session.
5. Confirm it no longer appears as running.
6. Wait for the outer pane to return to its shell.
7. Close only the outer pane created by this workflow.
8. Confirm all owned children have stopped and been waited/reaped, then remove only the exact owned run directory, unless bounded diagnostic retention was explicitly recorded. Do not remove it while a child still uses it.

Never delete another named session because it looks stale. Never close the pane running the current agent or any pane not created for the reproduction.

## Report the result

State what reproduced, what did not, and the exact transition that failed. Include child shutdown, exact-run cleanup status and any bounded retained artifacts/cleanup plan. Mention approved shared configuration or static/offline manifest overrides that may have influenced the result; do not imply live detector authority.
