# Kitsune fork roadmap

This fork started as a Kitsune-branded distribution of Herdr. Completed product
baseline and feature behavior live in `README.md` and `docs/next/README.md`.
This file tracks only work that is still intentionally open.

## Product direction

Kitsune should provide Herdr's core terminal-agent runtime while presenting a
Kitsune-owned product surface. Users interact through the canonical `kitsune`
command, which targets Kitsune-owned state rather than Herdr state.

The remaining goal is not basic rebranding; it is choosing the defaults,
integrations, and workflows that make Kitsune its own product.

## Architecture principles

- Treat this as a product fork, not a one-time source dump.
- Keep upstream Herdr changes easy to inspect while the fork is still young.
- Prefer runtime/API boundaries over coupling new behavior to TUI-only state.
- Separate product identity from runtime/session state so Kitsune and Herdr can
  coexist on the same machine.
- Avoid broad internal renames until the user-facing identity and runtime
  separation are already stable.

## Open work

### Djinn lifecycle rollout

Djinn is recognized as a first-class built-in agent kind. Kitsune accepts
`kitsune:djinn` reports as full lifecycle authority for the `djinn` agent label,
accepts Djinn session ids from those reports, and can plan resume launches with
`djinn agent chat --resume <id>`. Kitsune also accepts release reports so a
compatible Djinn build can clear pane authority when chat exits.

Djinn-side reporting rollout is intentionally deferred while Djinn has active
local changes in progress. Kitsune should not fetch, compare, or install Djinn
versions; install/update compatibility belongs to the Arcana-managed tool suite,
with user guidance to run `arcana update`. Older or unmanaged Djinn panes still
fall back to process detection plus the generic known-agent idle behavior.

Kitsune-side follow-up is limited to optional screen detection: add a bundled
Djinn manifest only if stable invariant TUI evidence emerges and live pane reads
show that process detection plus lifecycle reports are insufficient.

### As-needed product pruning

No concrete pruning candidates are currently identified. Remove or reshape
inherited Herdr behavior only when a specific surface no longer fits Kitsune's
product direction.

### As-needed internal naming cleanup

Keep internal Herdr module/type names where they help upstream comparison.
Rename internals only when the old name causes active confusion or blocks a
product change.

### Validation hygiene audit

Audit current full-suite behavior before listing blockers. Several previously
observed local failures have been fixed or may be stale, so future validation
work should start by running the narrowest useful check, recording any live
failure, and fixing only confirmed blockers. Do not run the resource-heavy
`just check` path without explicit approval.

## Working approach

Keep changes small and validate after each product slice so the fork remains
runnable throughout the transition.
