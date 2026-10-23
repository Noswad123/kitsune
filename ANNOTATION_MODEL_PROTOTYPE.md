# Generic annotation model prototype

> **Historical scope (2026-10-22 prototype).** The model has since been promoted
> to production; [ANNOTATIONS.md](ANNOTATIONS.md) is the current user/API contract.
> Statements below about test-only compilation, absent runtime integration, and
> work needed before production describe the original slice, not current status.
> `Workspace` targets remain test-only. Store fields are private; production uses
> `AnnotationState` with opaque epoch fencing, live-terminal cleanup, config/API/CLI
> integration, existing-border rendering, and timer-driven expiry. Validation below
> applies only to the historical prototype, not the production implementation.

## Overview

Decision: keep generic source-owned annotation data separate from client rendering,
not an agent model or a TUI-private protocol. `src/annotations.rs` is a **test-only
executable, in-memory prototype**, included only by `#[cfg(test)] mod annotations`
in `src/main.rs`. Runtime annotations and agent-feature extraction are not shipped.
This records the approved internal model/expiry/conflict and in-memory rendering
scope, not a production interface or a speculative external schema.

No runtime path, provider endpoint, config, schema, or production UI space changes
follow from this prototype. It adds no `AgentState`, seen/resume/notify behavior,
dependency, library, daemon, or public exports. Source labels are not authentication
or agent/group identity, and display grants no terminal-control permission.

## API / Interfaces

These are private prototype interfaces, not callable product APIs:

- `ResourceTarget`: `Terminal(TerminalId)` (opaque server-owned identity, not a
  pane/layout index) or `Workspace(String)`. Kind and ID both distinguish targets.
- `AnnotationStore::new(sources)`: fixed, explicitly selected priority order;
  zero through eight unique sources. Empty selection enables nothing. Source and
  workspace IDs must be 1–64 bytes of ASCII letters, digits, `-`, `_`, or `.`;
  terminal IDs use the existing opaque type, not that string validator.
- `replace(source, target, revision, text, ttl, now)`: one latest record per
  selected source/target pair, fully replaced on acceptance. `Some(text)` must be
  nonempty and at most 256 UTF-8 bytes; `None` clears, while `Some("")` is invalid.
  TTL is mandatory, inclusive 1 ms–5 min, including clears; deadline addition is
  checked. Revision is `u64`, strictly increasing per pair (initial zero allowed).
  Receive time is a caller-supplied `Instant`, nondecreasing per pair, not globally.
- `project(target, now) -> Option<&str>`: pure lookup, without mutation, target
  existence checking, or runtime access.
- `render_text(text, maxcells) -> String`: separate plain-text presentation helper
  using the existing `unicode_width::UnicodeWidthStr` dependency.

The store is bounded to 64 distinct targets and at most 512 current pair records
(8 × 64). Validation errors (`InvalidSources`, `UnselectedSource`, `InvalidTarget`,
`InvalidText`, `InvalidTtl`, `DeadlineOverflow`, `StaleRevision`, `TimeRegression`,
`Capacity`) leave state unchanged; rejected updates do not consume a revision.

## Data Flow

```text
explicit ordered selection -> validate replacement -> retain pair watermark/value
  -> project highest-priority recorded source -> hide or expose text
  -> escape plain text -> cell-clip -> caller-owned test buffer/rectangle
```

Projection first chooses the earliest selected source with **any accepted record**
for that target, then checks visibility. Its text is visible only when present
and `accepted_at <= now < deadline`. A clear, expired record, or future-accepted
winner hides output. Lower-priority records do not become fallback content;
preventing stale lower-source resurrection is intentional, independent of arrival
order or lower-source revisions. Before a higher source first records anything,
a lower selected source can display.

## Edge Cases

- Stale/equal revisions cannot replace, clear, or renew TTL, even after expiry.
  A newer revision cannot move receive time backwards for that pair; equal times
  are allowed. New replacements never resurrect an older value.
- Cleared/expired records retain revision and priority history, not an unbounded
  update log. There is no eviction or capacity reclamation: a new target at the
  limit is rejected, while existing targets can still receive updates or records
  from other selected sources. Production needs a lifecycle/cleanup policy.
- Identity is meaningful only within one store's resource lifetime. There is no
  target resolution, persistence, reconnect/epoch fencing, or restart guarantee.
- Rendering escapes C0/C1 controls, DEL, bidi controls/marks, and Unicode line and
  paragraph separators as literal Unicode escapes **before** clipping. Markup
  remains plain text. Cell width includes escape expansion and the clipping
  ellipsis; zero cells yields empty output. Clipping preserves UTF-8 scalar
  boundaries, not necessarily full grapheme clusters.
- Geometry belongs to the caller. The in-memory Ratatui test supplies a fresh
  buffer and rectangle; it does not reserve production space or integrate a TUI
  surface. Tests cover validation, ordering, expiry, replay, capacity, escaping,
  Unicode budgets, and caller-area isolation; no execution result is claimed here.
- Existing `metadata_tokens` expiry is unchanged: `values()` stays stable until
  explicit `expire_at(now)` mutation. Prototype projection-time hiding does not
  redefine that runtime contract.

Before production, design the actual user-opt-in source interface, target
resolution, resource/server lifetimes, revocation, refresh, cleanup, restart
fencing, UI placement/quotas, and service-failure policy. Shared data ownership
must remain separate from client rendering; do not lock an external schema now.

## Example usage

Internal scenario, not CLI/API syntax: select `["build", "tests"]` for a store.
For an existing terminal's opaque ID, accept `tests` revision 1 with text `passed`
and a valid TTL. It projects until `build` first records. Accept a `build` clear
at revision 1: output becomes hidden, including after that clear expires, rather
than falling back to `passed`. A fresh `build` revision 2 with text `ready` may
display within its TTL. Replaying revision 2 cannot renew it. The caller escapes
and clips projected text into its own buffer area; no live terminal is contacted.

## Validation

Focused validation (2026-10-22), with offline Cargo resolution and two build jobs:

- `just test-one annotations::`: **22 passed**.
- Existing characterization: `metadata_tokens::` **7 passed**,
  `ui::sidebar::tokens::` **6 passed**, and `ui::text::` **2 passed**.
- `cargo clippy --all-targets --locked -- -D warnings`: passed. This is the
  second command in `just lint`, run separately after its formatting step stopped.
- `rustfmt --check src/annotations.rs` and `git diff --check`: passed.

`just lint` is **not green**: its initial repository-wide `cargo fmt --check`
reports existing formatting in unchanged `src/ui/keybind_help.rs:471`. That file
matches `HEAD` and was not reformatted as part of this slice. No full suite,
`just check`, cross-target validation, or live application sessions were run.
The executable checks above do not establish production annotation behavior.

## Related files

- [src/annotations.rs](src/annotations.rs) — model, rendering helper, local tests.
- [src/main.rs](src/main.rs) — test-only module gate; no production caller.
- [src/terminal/id.rs](src/terminal/id.rs) — opaque `TerminalId`.
- [src/workspace.rs](src/workspace.rs) — workspace ID generator used by tests.
- [src/metadata_tokens.rs](src/metadata_tokens.rs) — separate existing expiry contract.
- [APP_BOUNDARIES.md](APP_BOUNDARIES.md) — accepted app ownership and future path.
- [KITSUNE_ROADMAP.md](KITSUNE_ROADMAP.md) — remaining integration work.
