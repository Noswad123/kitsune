# Kitsune
![kitsune](img/kitsune.png)

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
Sources are logical labels, not authentication. Coven's separate foreground
publisher supplies group/role text; Kitsune annotations add no workspace annotations,
group model, widgets, new panels or agent-feature extraction.
The [prototype notes](ANNOTATION_MODEL_PROTOTYPE.md) remain historical context.

## Integrations: generic presentation and messaging

The desktop lower side menu is a generic [Integrations block](INTEGRATION_VIEWS.md),
not an agent panel. With no providers it shows `No integrations configured` and
never falls back to detected agents. Manually configure `[integrations]` with
`sources = ["coven", "build"]` (up to eight ordered unique ASCII source IDs), then
select an app locally from its vertical list to see its label/detail rows.
Independent producers use `integration.view.open/replace/poll/close` with per-source
five-second leases, frames and selection queues. Missing/expired apps show
`Unavailable`; other producers and terminals keep working.

Coven's `group present` supplies its own group/participant model without Kitsune
interpreting agents or roles. This changes only default desktop lower-menu
presentation, not all legacy detection, APIs, mobile UI, hooks or restore behavior.
The obsolete `[external_view]` config is diagnosed/ignored, not automatically
rewritten. See the contract for discovery, bounds and at-most-once, non-authorizing
selection messaging.

## Runtime identity

Kitsune has its own runtime identity so it can coexist with an installed Herdr session on the same machine.

- Command: `kitsune`.
- Runtime config, sockets, logs, and environment variables use `kitsune` / `KITSUNE_*` names.
- Panes launched by Kitsune receive Kitsune-owned workspace, tab, and pane environment.
- The default config inspection and reload workflows use Kitsune paths and sockets.

Internal module/type names may still reference Herdr where keeping the fork easy to compare with upstream is more valuable than a broad rename.

## Build and run

The current terminal engine is vendored libghostty-vt and requires the matching
Zig toolchain (currently 0.15.2). `ZIG=/absolute/path/to/zig` selects an explicit
compiler without a global installation. A possible Rust backend and alternatives
to the local compiler requirement have their own
[terminal engine roadmap](TERMINAL_ENGINE_ROADMAP.md); no engine replacement has
been selected or shipped.

```bash
cargo build --bin kitsune
./target/debug/kitsune
```

The local helper justfile installs the debug binary into the scoped development
directory `~/.local/libexec/arcana-dev/kitsune/bin`, separate from Arcana-managed
commands:

```bash
just install
"$HOME/.local/libexec/arcana-dev/kitsune/bin/kitsune"
```

Use `INSTALL_DIR="/your/dev/bin" just install` to override the destination explicitly.
The development directory is not added to `PATH`; use the full path to select it.

## Kitty graphics

Kitty graphics work by default when using a compatible outer terminal. Restart
existing Kitsune servers after updating to enable graphics in their panes. The
legacy `experimental.kitty_graphics` flag is no longer necessary and can be removed
from your config; either value is ignored with the normal unknown-key diagnostic.

## Session recall

Kitsune includes a `session_recall` action bound by default to `prefix+shift+s`. The action opens the external session recall TUI in a popup and invokes it with the native Kitsune backend.

The helper receives `KITSUNE_ENV`, `KITSUNE_BIN_PATH`, `KITSUNE_SOCKET_PATH`, `KITSUNE_CLIENT_SOCKET_PATH`, and active workspace/tab/pane context. Herdr, tmux, and Kitsune are selected as normal helper backends; Kitsune does not mirror one backend's environment variables into another backend.

## Agent navigation

Kitsune includes a session navigator and an agents-only selector:

- `prefix+g` opens the full session navigator.
- `prefix+a` opens the navigator directly in agents-only scope.
- `Tab` toggles the navigator between full tree and agents-only scope.
- In agents-only scope, `r` renames the selected agent pane's pane label.

## Legacy agent hooks

Legacy agent-hook installer targets are `pi`, `opencode`, `claude`, `codex`, `copilot`, and `djinn`. These fixed targets and `integration.install` / `integration.uninstall` are separate from generic `integrations.sources`, not its provider registry. Installer logic remains during transition. Djinn support remains a built-in agent kind for process detection, integration status, interactive startup through `kitsune agent start --kind djinn`, and authoritative lifecycle reports from source `kitsune:djinn`.

Kitsune does not resolve or install Djinn versions itself. Install or update the managed Jamal Arcana tool suite with `arcana update` to keep Kitsune and Djinn on compatible builds.

## Windows note

Native Windows builds use the system ConPTY included with Windows 10 October 2018 or newer. Kitsune does not currently ship a pinned app-local ConPTY bundle.

## Notes

- Agent integrations remain available unless removed separately.
- Durable decisions live in root docs; current feature details also live under `docs/next/`.

Kitsune inherits Herdr's Apache-2.0 license.
