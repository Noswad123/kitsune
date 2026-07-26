# Kitsune

Kitsune started as a personal fork of Herdr, a terminal-based runtime for running coding agents in panes, tabs, workspaces, and detached sessions.

This fork is branded around the `kitsune` command and local Kitsune runtime state. It intentionally removes Herdr's public website, hosted updater, and plugin marketplace surfaces.

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

The local helper makefile can install the debug binary as `~/.local/bin/kitsune`:

```bash
make install
```

## Session recall

Kitsune includes a `session_recall` action bound by default to `prefix+shift+s`. The action opens the external session recall TUI in a popup and invokes it with the native Kitsune backend.

The helper receives `KITSUNE_ENV`, `KITSUNE_BIN_PATH`, `KITSUNE_SOCKET_PATH`, `KITSUNE_CLIENT_SOCKET_PATH`, and active workspace/tab/pane context. Herdr, tmux, and Kitsune are selected as normal helper backends; Kitsune does not mirror one backend's environment variables into another backend.

## Notes

- Agent integrations remain available unless removed separately.
- Durable fork documentation lives under `docs/next/`.

Kitsune inherits Herdr's Apache-2.0 license.
