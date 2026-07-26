# kitsune

Kitsune is a personal fork of Herdr: a terminal-based runtime for running coding agents in panes, tabs, workspaces, and detached sessions.

This fork is being rebranded around the `kitsune` command and local Kitsune runtime state. It intentionally removes Herdr's public website, hosted updater, and plugin marketplace surfaces.

## build and run

```bash
cargo build --bin kitsune
./target/debug/kitsune
```

The local helper makefile can install the debug binary as `~/.local/bin/kitsune`:

```bash
make install
```

## notes

- Runtime config, sockets, logs, and environment variables use `kitsune` / `KITSUNE_*` names.
- The standalone command is `kitsune`; the shorter `kit` name is intentionally left to the user's existing Kitsune multiplexer.
- Agent integrations remain available unless removed separately.

Kitsune inherits Herdr's Apache-2.0 license.
