# Kitsune

Kitsune started as a personal fork of Herdr, a terminal-based runtime for running coding agents in panes, tabs, workspaces, and detached sessions.

This fork is branded around the `kitsune` command and local Kitsune runtime state. It intentionally removes Herdr's public website, hosted updater, and plugin marketplace surfaces.

[APP_BOUNDARIES.md](APP_BOUNDARIES.md) records the 2026-10-22 target: a general
multiplexer, with agent-aware behavior moving to optional Coven integration.
Built-in agent features described below still exist; extraction is not shipped.

Initial opt-in observation is implemented in Coven only, using one explicitly
selected existing `session.snapshot` API socket (protocol 18). See the
[source inspection](AGENT_TRACKING_INSPECTION.md) and optional sibling
[Coven contract](../coven/KITSUNE_OBSERVATION_CONTRACT.md); neither is a dependency.
This is not a zero-server-effects promise: existing request housekeeping can
expire metadata, drain events, sync titles, or ensure a default workspace,
potentially spawning a shell. No live connections were run for this docs update.

[Generic terminal annotations](ANNOTATIONS.md) are now implemented as a separate,
opt-in path: enable a source in config, read snapshot identities/epoch, and use
`kitsune pane annotate` to append bounded, expiring plain text in spare existing
top-border cells. Single-pane layouts have no border; labels need an existing
split layout with borders enabled. Manual titles and terminal space are untouched.
Sources are logical labels, not authentication. No Coven publisher, workspace
annotations, grouping, widgets, new panels, or agent-feature extraction is included.
The [prototype notes](ANNOTATION_MODEL_PROTOTYPE.md) remain historical context.

## Runtime identity

Kitsune has its own runtime identity so it can coexist with an installed Herdr session on the same machine.

- Command: `kitsune`.
- Runtime config, sockets, logs, and environment variables use `kitsune` / `KITSUNE_*` names.
- Panes launched by Kitsune receive Kitsune-owned workspace, tab, and pane environment.
- The default config inspection and reload workflows use Kitsune paths and sockets.

Internal module/type names may still reference Herdr where keeping the fork easy to compare with upstream is more valuable than a broad rename.

## Build and run

```bash
cargo build --bin kitsune
./target/debug/kitsune
```

The local helper justfile can install the debug binary as `~/.local/bin/kitsune`:

```bash
just install
```

## Session recall

Kitsune includes a `session_recall` action bound by default to `prefix+shift+s`. The action opens the external session recall TUI in a popup and invokes it with the native Kitsune backend.

The helper receives `KITSUNE_ENV`, `KITSUNE_BIN_PATH`, `KITSUNE_SOCKET_PATH`, `KITSUNE_CLIENT_SOCKET_PATH`, and active workspace/tab/pane context. Herdr, tmux, and Kitsune are selected as normal helper backends; Kitsune does not mirror one backend's environment variables into another backend.

## Agent navigation

Kitsune includes a session navigator and an agents-only selector:

- `prefix+g` opens the full session navigator.
- `prefix+a` opens the navigator directly in agents-only scope.
- `Tab` toggles the navigator between full tree and agents-only scope.
- In agents-only scope, `r` renames the selected agent pane's pane label.

## Agent integrations

Supported integration targets are `pi`, `opencode`, `claude`, `codex`, `copilot`, and `djinn`. Djinn support is built in as a first-class agent kind for process detection, integration status, interactive startup through `kitsune agent start --kind djinn`, and authoritative lifecycle reports from source `kitsune:djinn`.

Kitsune does not resolve or install Djinn versions itself. Install or update the managed Jamal Arcana tool suite with `arcana update` to keep Kitsune and Djinn on compatible builds.

## Windows note

Native Windows builds use the system ConPTY included with Windows 10 October 2018 or newer. Kitsune does not currently ship a pinned app-local ConPTY bundle.

## Notes

- Agent integrations remain available unless removed separately.
- Durable decisions live in root docs; current feature details also live under `docs/next/`.

Kitsune inherits Herdr's Apache-2.0 license.
