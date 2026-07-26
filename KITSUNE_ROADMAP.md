# Kitsune fork roadmap

This fork started as a Kitsune-branded distribution of Herdr. The completed
runtime identity and session recall baseline is documented in `README.md` and
`docs/next/README.md`. This roadmap now tracks forward-looking divergence work.

## Product goal

Kitsune should provide Herdr's core terminal-agent runtime while presenting a
Kitsune-owned product surface. Users can interact through the canonical
`kitsune` command or the short `kit` command. Both entry points run the same
runtime and target Kitsune-owned state rather than Herdr state.

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

Candidate work:

- Finish replacing Herdr release/update surfaces with Kitsune-owned channels.
- Replace or remove remaining Herdr-branded docs, assets, integration text, and
  package metadata.
- Add Kitsune-specific workflows and defaults.
- Remove features that do not fit Kitsune's product direction.
- Rename internal modules/types only when it reduces confusion more than it
  increases merge pain.
- Keep `kit` and `kitsune` behavior aligned so editor integrations, shell
  aliases, and remote workflows can use either entry point predictably.

## Near-term working branch

Use `kitsune-integration` as the integration branch until the first Kitsune
release line is stable. Keep changes small and validate after each phase so the
fork remains runnable throughout the transition.
