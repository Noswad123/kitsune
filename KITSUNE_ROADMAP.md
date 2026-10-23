# Kitsune fork roadmap

## Target boundary and priority — 2026-10-22

[APP_BOUNDARIES.md](APP_BOUNDARIES.md) records the accepted target: Kitsune owns
general multiplexer operations and generic labels/badges; optional Coven owns
agent-specific discovery/status interpretation/start/resume and job concepts.
Without Coven, the target is fully functional multiplexing with no agent-aware
features. Current built-in agent integrations remain during transition; this is
not an immediate removal or a claim that extraction has shipped.

Initial single-endpoint observation is implemented in Coven only; built-in
interpretation remains here. The first opt-in generic terminal-border annotation
slice is implemented. Next: characterize generic surfaces before extraction
and group existing sessions in Coven. Do not require a
scheduler, daemon, plugin framework, or native admission design for this slice.
Missing/unreachable/incompatible Coven must not break startup or terminal work;
stale badges are hidden/unknown, never stale active status. Decoration grants no
launch/send/stop permission. More worker/worktree work is deferred behind this path.

The [initial source inspection](AGENT_TRACKING_INSPECTION.md) records the current
flow, status/identity limits, and protected compatibility. In particular, `done`
means idle/unacknowledged, not native success or job completion. The optional
sibling [Coven observation contract](../coven/KITSUNE_OBSERVATION_CONTRACT.md)
documents the initial adapter, not a dependency or completed extraction. Snapshot
dispatch may perform housekeeping/default-workspace creation (potentially spawning
a shell); no zero-server-effects guarantee or live validation is claimed.

[Production annotations](ANNOTATIONS.md) now include ordered source opt-in,
snapshot capability/epoch discovery, pinned `pane.annotate` and CLI writes, bounded
live-terminal records/watermarks, reload fencing, cleanup, timer expiry, and text
in spare existing top-border cells. Single-pane layouts have no border. Disconnect
is not immediately detected; TTL bounds visibility. Source labels are not auth.
The [prototype](ANNOTATION_MODEL_PROTOTYPE.md) is historical context, with workspace
targets still test-only. Workspace presentation, richer progress/group hints,
actual Coven publishing, grouping, and agent extraction remain open; no panels,
widgets, unrequested UI space, or terminal-control authority follows from labels.

This fork started as a Kitsune-branded distribution of Herdr. Completed product
baseline and feature behavior live in `README.md` and `docs/next/README.md`.
This file tracks only work that is still intentionally open.

## Product direction

Kitsune should become a general terminal multiplexer with a Kitsune-owned product
surface, transitioning from inherited agent-aware behavior. Users interact through
the canonical `kitsune` command, which targets Kitsune-owned state rather than
Herdr state.

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

**Compatibility during transition, not the target core boundary.** Preserve and
characterize existing behavior while extracting agent-specific responsibilities;
do not deepen core coupling or treat the rollout below as an immediate removal.

Djinn is recognized as a first-class built-in agent kind. Kitsune accepts
`kitsune:djinn` reports as full lifecycle authority for the `djinn` agent label,
accepts Djinn session ids from those reports, and currently plans resume launches
with `djinn -s <id>`. Kitsune also accepts release reports so a
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

Agent-specific discovery, status, start/resume, and job surfaces are identified
extraction candidates under APP_BOUNDARIES.md. Preserve compatibility while
characterizing and moving them; unrelated pruning remains as-needed.

### Worktree UX nice-to-haves

The TUI worktree create flow derives a checkout path from the branch/name input.
The CLI and API already allow an explicit checkout path. A future TUI polish could
make that derived path visible and editable before creation, but this is not
urgent while the sibling default remains predictable.

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
