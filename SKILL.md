---
name: kitsune
description: "Control Kitsune, a terminal multiplexer for coding agents. Use only when the user explicitly mentions Kitsune or asks to use Kitsune to inspect or control panes, tabs, workspaces, commands, or another agent. Do not use merely because a task could benefit from a background terminal, delegation, or parallel work. Requires KITSUNE_ENV=1."
---

# Kitsune

Kitsune organizes generic terminals into workspaces, tabs and panes and exposes the session through the `kitsune` CLI. Live agent detection and all live agent API methods are globally retired, including without Coven. Use optional Coven for agent meaning, not a Kit fallback.

Before issuing any control command, verify that this agent is running inside a Kitsune-managed pane:

```bash
test "${KITSUNE_ENV:-}" = 1
```

If the check fails, say that you are not running inside Kitsune and stop. Do not inspect or control the focused Kitsune session from outside Kitsune.

When the check passes, the `kitsune` binary in `PATH` talks to the current session. Use it to inspect neighboring work, create terminal layout, run explicitly authorized commands, read output and wait for output. Do not infer native lifecycle, readiness or completion from terminal activity.

## Learn the current CLI

The installed binary is the authority for command syntax. Start with:

```bash
kitsune --help
```

Then print the relevant command group by running the group without a subcommand:

```bash
kitsune pane
kitsune workspace
kitsune tab
kitsune worktree
kitsune terminal
kitsune notification
kitsune integration
kitsune session
```

Do not run bare `kitsune` for discovery; it launches or attaches the TUI. Do not probe a mutating nested command by omitting arguments. Commands such as `kitsune workspace create` are valid with defaults and will execute.

Most control commands return JSON. Read identifiers and state from those responses instead of predicting them.

## Understand Generic Layout

Choose the primitive that matches the job:

- Workspace, tab, and pane topology organize terminal locations.
- Pane commands control raw terminals, shells, tests, servers, input, and output.
- Agent commands are compatibility spellings, not live controls or pane-read aliases.

A pane exists independently of its foreground program. All 12 live agent API methods return `feature_retired` in both ownership modes before target lookup or input. Convenience and raw API CLI retire locally without socket/protocol probes or native execution. Help remains exit 0; invalid syntax remains exit 2. Explicitly offline `kitsune agent explain --file <capture-path> --agent <kind> --json` remains for static local/bundled compatibility maintenance only.

Snapshot `agents: []` applies globally. Historical names, argv and native session references are data, not aliases, current identity or permission to resume. Saved layout/history never triggers provider execution. Generic shell respawn policy is separate from provider resume.

AgentStatus subscriptions and the agent `events.wait` matcher are retired, including mixed requests atomically before patterns/probes/streams. Generic output/scroll/workspace subscriptions, event streaming and pane output waits remain. Other previously unsupported generic wait matchers stay unsupported. Four legacy pane hooks retire standalone and retain the older external-ownership error externally. Generic metadata accepts tokens only; mixing retired agent presentation with tokens rejects atomically. Explicit notifications remain generic, never idle-inferred completion.

## Use IDs and caller context

Public IDs are opaque stable handles:

- workspace: `w1`
- tab: `w1:t1`
- pane: `w1:p1`

Closed tab and pane IDs are not reused. A pane moved into another workspace receives a new workspace-qualified pane ID. After `pane move`, continue with `.result.move_result.pane.pane_id`. The old value is reported as `.result.move_result.previous_pane_id`; only the moved process's inherited caller context keeps resolving that old ID, so do not use it as a general target.

Kitsune injects the caller's context into each managed pane:

```bash
printf '%s\n' "$KITSUNE_WORKSPACE_ID" "$KITSUNE_TAB_ID" "$KITSUNE_PANE_ID"
```

Prefer `--current` when a pane command should target the calling pane. Omitting a target may use the UI-focused pane, which can belong to the user or another client.

Discover live state with:

```bash
kitsune workspace list
kitsune tab list --workspace "$KITSUNE_WORKSPACE_ID"
kitsune pane current --current
kitsune pane list --workspace "$KITSUNE_WORKSPACE_ID"
kitsune pane process-info "$KITSUNE_PANE_ID"
```

Creation responses expose the IDs to use next. `workspace create` returns `.result.workspace`, `.result.tab`, and `.result.root_pane`. `tab create` returns `.result.tab` and `.result.root_pane`. `pane split` returns the new pane as `.result.pane`.

## Create Terminal Layout

Default to a sibling pane in the current tab and the current working directory. Do not create a workspace, tab, worktree, or different cwd unless the user explicitly requests that topology or location.

Honor a direction requested by the user. Otherwise inspect the caller pane:

```bash
kitsune pane layout --pane "$KITSUNE_PANE_ID"
```

Split a wide pane to the right and a narrow or tall pane down. Avoid repeated same-direction splits that create unusably narrow columns or short rows. Keep the user's focus in the calling pane and explicitly preserve the caller's working directory:

```bash
kitsune pane split --current --direction right --cwd "$PWD" --no-focus
```

Replace `right` with `down` when appropriate. Read the new pane ID from `.result.pane.pane_id`.

An available shell pane must be at its interactive prompt, with the shell itself in the foreground and no foreground command/editor/provider. Any authorized native command uses generic command submission or Coven's explicit supported adapter, never `kitsune agent start/prompt/wait/attach`. Generic reads/input/output matches cannot validate provider readiness, correlate a turn or authorize further input. Keep generic process observations separate from authenticated native facts.

## Run an ordinary command in another pane

Create a sibling pane with the same geometry rule, preserve the caller's working directory, and keep user focus unchanged:

```bash
kitsune pane split --current --direction right --cwd "$PWD" --no-focus
```

Read the new pane ID from `.result.pane.pane_id`, then run and inspect the command:

```bash
kitsune pane run <returned-pane-id> "just test"
kitsune pane wait-output <returned-pane-id> --match "test result" --timeout 120000
kitsune pane read <returned-pane-id> --source recent-unwrapped --lines 120
```

`pane run` atomically sends command text and Enter. `pane wait-output` searches the selected snapshot immediately, so output that already exists can match. Use `--match <text>` for a literal substring or `--regex <pattern>` for a Rust regular expression. Omitting `--timeout` allows an indefinite wait.

Use the read source that matches the task:

- `visible`: the currently rendered viewport.
- `recent`: recent rendered output, including soft wraps.
- `recent-unwrapped`: recent output with soft wraps joined; prefer it for logs and transcripts.
- `detection`: a generic plain-text bottom-buffer snapshot, independent of the viewport and without a native classifier.

For sources that retain styling, `--format ansi` can preserve it. The `detection` source currently returns plain text for both text and ANSI formats; do not promise ANSI styling or recovered alternate-screen pixels.

`--lines` asks Kitsune for more rows from the pane's available screen and host scrollback. If increasing it does not reveal more of a completed response, the pane is probably running the agent on the terminal's alternate screen. Rows that leave the alternate screen do not enter Kitsune's host scrollback, so a larger line count cannot recover them.

After that failed read, ask the agent to write its complete response as Markdown in a temporary directory and reply only with the file path, then read the file directly. Use this only as a fallback; do not request file output in the initial prompt.

## Safety and coordination rules

- Use `--no-focus` for background work unless the user asked to switch context.
- Use `--current` or an explicit pane ID. Do not rely on another client's focused pane or historical agent names.
- Parse IDs from JSON responses. Do not derive them from sidebar order or examples.
- Do not close workspaces, tabs, panes, or sessions you did not create unless the user explicitly asked.
- Never run `kitsune server stop` from an active session unless the user explicitly intends to stop the server and its pane processes.
- Never kill the main Kitsune process. Use named test sessions for experiments that need an isolated server.
- CLI server errors are JSON on stderr with exit status 1. CLI syntax errors exit with status 2.

Fresh legacy `integration install` is retired for all six targets on all platforms before discovery/probes/writes. Status/uninstall and frozen migration assets remain for explicit cleanup, not automatic uninstall. Generic Integrations sources/leases/views remain separate. See [retirement and validation](AGENT_CONTROL_RETIREMENT.md); remaining static evaluator/DTO/cleanup residue is not live authority or cross-OS qualification.
