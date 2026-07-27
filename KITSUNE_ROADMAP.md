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

## Remaining work

### Djinn lifecycle producer integration

Djinn is recognized as a first-class built-in agent kind. Kitsune accepts
`kitsune:djinn` reports as full lifecycle authority for the `djinn` agent label,
accepts Djinn session ids from those reports, and can plan resume launches with
`djinn agent chat --resume <id>`. Kitsune also accepts release reports so a
compatible Djinn build can clear pane authority when chat exits.

Further Djinn-side rollout is intentionally deferred while Djinn has active local
changes in progress. Kitsune should not fetch, compare, or install Djinn
versions; install/update compatibility belongs to the Arcana-managed tool suite,
with user guidance to run `arcana update`. What remains on the Kitsune side is,
if needed, a bundled screen manifest once invariant Djinn TUI evidence is
captured. Unmanaged or older Djinn panes still rely on process detection plus the
generic known-agent idle fallback.

### Product surface pruning

Continue removing or reshaping inherited Herdr behavior only when a specific
feature no longer fits Kitsune's product direction. Avoid broad cleanup for its
own sake.

No concrete pruning candidates are currently identified. Add specific stale
surfaces here when they are found; avoid broad cleanup for its own sake.

### Internal naming debt

Keep internal Herdr module/type names where they help upstream comparison. Rename
internals only when the old name causes active confusion or blocks a product
change.

### Validation hygiene

Resolve the remaining local full-suite blockers so `cargo test --locked` is a
clean validation path on this machine. Known blockers observed recently include
macOS Unix socket path length failures, stale generated API schema artifacts,
unicode-width expectation drift, and poison-error cascades after early failures.

## Working approach

Keep changes small and validate after each product slice so the fork remains
runnable throughout the transition.
