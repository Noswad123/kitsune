# Stable snapshot

Date: 2026-07-19

This snapshot is stable enough for daily personal use with Herdr.

## Included baseline

- Herdr save/restore/apply for workspace, tab, pane topology and metadata
- componentized workspace/tab/pane templates with refs and fingerprints
- TUI live/template browser with save, restore/apply, compare, edit, and multi-select
- explicit template actions with workspace/tab/pane fan-out
- Herdr pane action delivery via live pane resolution and `send-text` + Enter
- `kit nav` and fast `kit herdr-nav` smart navigation
- store validation and action diagnostics
- first-pass tmux metadata save and smart navigation

## Known planning topics

- tmux layout restore semantics
- possible Zellij backend
- whether Kitsune should remain a companion/orchestrator or become a multiplexer

See [planning.md](planning.md) before implementing future work.
