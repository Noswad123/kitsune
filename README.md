# Kitsune (`kit`)

![Kitsune](img/kitsune.png)

Kitsune is a multiplexer companion for named, composable working-session kits.
It captures, versions, composes, restores, compares, edits, and runs explicit
actions for workspace/tab/pane layouts.

```text
workspace -> tab -> pane
stack     -> many workspaces
```

Herdr is the primary backend. Tmux has first-pass metadata save support and smart
navigation support; tmux restore/layout restore is still in planning.

## Install

```bash
cargo install --path .
```

This installs the daily command as `kit`.

## Quick start

```bash
kit doctor
kit init
kit save --current
kit list
kit tree workspace darkness
kit tui
```

Common restore/apply/action commands:

```bash
kit restore workspace darkness --dry-run
kit apply tab logs --dry-run
kit stack create morning darkness rustlings
kit run start rustlings --dry-run
```

## Concepts

- Templates live under `~/.config/kitsune` by default.
- Workspaces reference tab templates; tabs reference pane templates.
- Captured foreground commands are inert observed state, not restore behavior.
- Commands that should run belong in explicit template `actions:` config.
- `backend_ref` stores compact live backend IDs for sync/action targeting.

## More documentation

- [Usage](docs/usage.md): commands, TUI keys, storage layout
- [Templates](docs/templates.md): schema concepts, refs, fingerprints, observed state
- [Actions](docs/actions.md): explicit runnable actions and fan-out behavior
- [Navigation](docs/navigation.md): `kit nav` / `kit herdr-nav` setup
- [Planning](docs/planning.md): design topics that are not implementation-ready

## Current status

- Herdr: doctor, save, restore/apply topology, duplicate-label safety, actions,
  smart navigation, TUI metadata flows.
- Tmux: doctor, smart navigation, and metadata save for sessions/windows/panes.
- Planning: tmux layout restore semantics, possible Zellij backend, and whether
  Kitsune should remain a companion or become a multiplexer.
