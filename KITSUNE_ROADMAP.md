# Kitsune fork roadmap

This fork started as a Kitsune-branded distribution of Herdr. The completed
runtime identity and session recall baseline is documented in `README.md` and
`docs/next/README.md`. This roadmap now tracks forward-looking divergence work.

## Product goal

Kitsune should provide Herdr's core terminal-agent runtime while presenting a
Kitsune-owned product surface. Users interact through the canonical `kitsune`
command, which targets Kitsune-owned state rather than Herdr state.

The remaining product goal is no longer basic rebranding; it is choosing the
defaults and workflows that make Kitsune its own product.

## Architecture principles

- Treat this as a product fork, not a one-time source dump.
- Keep upstream Herdr changes easy to inspect while the fork is still young.
- Prefer runtime/API boundaries over coupling new behavior to TUI-only state.
- Separate product identity from runtime/session state so Kitsune and Herdr can
  coexist on the same machine.
- Avoid broad internal renames until the user-facing identity and runtime
  separation are already stable.

## Phase 3: intentional divergence — active

Goal: make Kitsune its own product while preserving a working runtime.

Completed in this phase:

- Removed Herdr release/update command surfaces and hosted stable/preview
  channel behavior from the active Kitsune CLI/UI path.
- Replaced low-risk Herdr-branded docs, assets, integration text, and package
  metadata while keeping historical Herdr references where context matters.
- Aligned the `kitsune` help/completion surface so generated shell completions
  and command examples use the canonical entry point.
- Deleted dead supporting code for removed updater, remote auto-install, and
  inherited integration targets so Kitsune only carries supported integration
  plumbing.
- Added Djinn as a built-in first-class agent kind for process detection,
  interactive `agent start --kind djinn`, and integration status.

Remaining candidate work:

- Remove features that do not fit Kitsune's product direction.
- Rename internal modules/types only when it reduces confusion more than it
  increases merge pain.
- Continue auditing `kitsune` behavior so editor integrations, shell aliases,
  and remote workflows use the canonical entry point predictably.

## Agent selector
- Right now I leverage prefix+a to shift focus from agent to agent
- I would like to pull a view of just agents then have the ability to navigate them or change the name of their panes
## Support djinn agent harness
- my djinn harness can be found ~/projects/djinn
- It needs to be treated like a first class harness akin to opencode, pi, etc
- Status: Djinn is now recognized as a first-class built-in agent kind. Future
  work can add deeper Djinn-specific lifecycle reporting if the harness exposes
  stable hooks/events for working and blocked states.

## Near-term working branch

Use `kitsune-integration` as the integration branch until the first Kitsune
release line is stable. Keep changes small and validate after each phase so the
fork remains runnable throughout the transition.
