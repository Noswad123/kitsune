# Kitsune agent tracking: initial inspection

Date: **2026-10-22**. Source baseline: `24f50511`, plus documentation-only
changes. This is a read-only source assessment for [APP_BOUNDARIES.md](APP_BOUNDARIES.md),
not an extraction, a shipped provider interface, or validation of live sessions.
No builds, tests, running Kitsune/Djinn commands, or user-session queries were run.

## Main finding

Kitsune already combines terminal observations, agent-specific interpretation,
native reporting hooks, and presentation. Moving agent tracking to Coven is
feasible, but it is **not a single-module move**. Preserve existing behavior while
establishing a read-only observation seam; do not remove integrations first.

Most importantly, the public status **`done` means idle and not yet acknowledged
by the user**, not successful agent execution or accepted work. The API computes
it from `AgentState::Idle` plus `seen == false` in
[`src/app/api_helpers.rs`](src/app/api_helpers.rs) (`pane_agent_status`). Focusing
a tab can acknowledge its panes. Coven must not interpret this badge as job
completion.

## Current data flow

```text
foreground-process evidence + recent bottom-buffer text + OSC observations
    -> built-in agent recognition and manifest rules
native integration reports + optional native session references
    -> source/session/sequence arbitration in TerminalState
effective agent state + pane acknowledgement (seen)
    -> pane/agent API projections, sidebar, navigator, notifications
```

- [`src/pane.rs`](src/pane.rs) schedules process/screen observations. PTY output
  invalidates scans; output bytes alone do not prove work was accepted or advanced.
- [`src/detect/mod.rs`](src/detect/mod.rs) recognizes agent kinds and classifies
  reporting sources. [`src/detect/manifest.rs`](src/detect/manifest.rs) evaluates
  screen/OSC rules, overrides, and bundled manifests. A recognized agent with no
  matching rule defaults to idle; that is not positive idle evidence.
- [`src/pane/agent_detection.rs`](src/pane/agent_detection.rs) implements startup,
  idle-confirmation, and scan decisions. Detection uses the recent bottom buffer,
  not the user's scrolled viewport.
- [`src/terminal/state.rs`](src/terminal/state.rs) holds effective/fallback state,
  hook authority, session references, observation order, and source sequences.
  Matching full-lifecycle reports can suppress screen detection; session-generation
  and process-presence checks reject stale/inapplicable reports. This is richer
  than either "latest wins" or "native always wins."
- [`src/integration/assets/`](src/integration/assets/) contains runtime-specific
  reporting adapters. Some report session identity only; others interpret runtime
  events into status. The receiver's "full lifecycle" source classification is
  not attestation of native execution/result guarantees.

## Freshness and identity limits

- Status-hook authority has no general TTL/heartbeat expiry. Missing a final
  report can leave status sticky while matching process evidence remains.
  Presentation metadata TTL is separate and does not expire lifecycle authority.
- [`src/api/event_hub.rs`](src/api/event_hub.rs) keeps a bounded in-memory event
  history. Public events are not a durable replay log and do not supply a complete
  native-identity/provenance feed.
- [`src/app/api/session.rs`](src/app/api/session.rs) exposes `session.snapshot`
  with version/protocol, panes, agents, and layout. It does not provide a durable
  server-instance identity, observation timestamp, or resumable event cursor.
- Scope pane/terminal references to the selected server/session endpoint. Public
  pane IDs can overlap across servers; terminal IDs can change after restore.
  Neither is a native agent/session identity.
- The current terminal model holds one current agent authority/reference. It is
  not an inventory of every native session or subagent sharing a pane.
- Native-reference acceptance validates recognized source labels; it does not
  authenticate a reporter. A successful reporting RPC need not mean arbitration
  adopted the reported state.

## What stays and what moves

| Keep in Kitsune | Move toward optional Coven adapters |
| --- | --- |
| PTYs, terminal parsing, process observations, bottom-buffer reads | Agent recognition, screen-rule interpretation, runtime-specific status mappings |
| Server/session/pane operations and generic event transport | Cross-runtime discovery/provenance, native-reference reconciliation |
| Generic labels, tokens, clipping, layout, expiry presentation | Participant roles, job/group membership, job progress and acceptance |
| Generic terminal attachment/input primitives | Agent-specific start/resume planning, through separately authorized effectful adapters |

Existing generic-ish observations include `pane.process_info` in
[`src/app/api/panes.rs`](src/app/api/panes.rs) and terminal snapshot reads in
[`src/app/api_helpers.rs`](src/app/api_helpers.rs). These need characterization,
not an assumption of platform-independent completeness: Windows process selection
already contains agent-aware policy. Screen/process arguments can contain sensitive
data; do not copy full buffers or command lines into job records by default.

Presentation is also not isolated yet:

- [`src/workspace/aggregate.rs`](src/workspace/aggregate.rs) and
  [`src/ui/sidebar.rs`](src/ui/sidebar.rs) carry agent enums into pane details and
  agent-panel entries. A generic decorated shell does not automatically qualify
  for the existing Agents panel.
- [`src/ui/mobile.rs`](src/ui/mobile.rs), navigator/input code, notifications,
  border labels, and settings depend on agent status/attention semantics. Existing
  aggregate ordering is not identical in every surface.
- [`src/metadata_tokens.rs`](src/metadata_tokens.rs) has generic token expiry,
  but its flat storage is not an established source-owned provider contract.
  `agent_view_override` only filters/sorts existing agents; it is not an external
  decoration provider.
- The thin client mostly receives rendered frames. Do not introduce a Coven-aware
  private TUI socket as a shortcut around the shared server/API boundary.

## Protected compatibility and next slice

Agent startup/control and automatic resume are effectful existing behavior, not
part of a read-only display migration. Resume planning lives in
[`src/agent_resume.rs`](src/agent_resume.rs), restoration in
[`src/persist/restore.rs`](src/persist/restore.rs), and deferred startup in
[`src/app/agent_resume.rs`](src/app/agent_resume.rs). Preserve deduplication,
history suppression, pending-start, terminal-identity, and native-reference checks.
The current Djinn planner emits `djinn -s <id>`; this inspection did not verify a
running Djinn build or reporting rollout.

**Initial observation adapter implemented in Coven only:** one explicitly selected
existing endpoint and offline fixtures. See the optional sibling
[observation contract](../coven/KITSUNE_OBSERVATION_CONTRACT.md), not a Kitsune
dependency. Extraction, annotations, and grouping are not shipped. Preserve legacy raw
status/attention meaning, available native references, endpoint context, and
observation provenance. Missing/stale/unsupported evidence stays unknown; no
launch, send, resume, authority release, or stop follows from an observation.
This is a compatibility bridge, not completed extraction. Characterize generic
process/read surfaces before migrating interpretation away from built-in detection.

The builder takes `&self`, but existing request dispatch expires metadata, drains
events, syncs titles, and can ensure a default workspace (potentially spawning a
shell). The adapter's read method cannot prevent housekeeping: do not claim zero
server-side effects. Snapshot receipt does not attest freshness; native references
may be persisted. No live connections were run for this follow-up.

Then design a separate generic optional annotation path and group existing native
sessions in Coven. User-enabled coordinator/build/test/export sources may annotate
existing panes/workspaces with bounded text badges/labels/group hints/progress.
Kitsune/user own layout, space, rendering, clipping, and quotas; source-owned values
need conflict policy/expiry. No panels, embedded apps/scripts, or unrequested UI
space; labels confer no agent identity, group membership, or terminal control.
No plugin framework, daemon, new external schema, or return endpoint is implemented.
Absent/malformed/incompatible/disconnected input is hidden/unknown without loss
of terminal usability. An absent provider must leave ordinary multiplexing unchanged;
expired decoration cannot remain active, change agent authority/acknowledgement,
or attach to a replacement terminal. Transport and exact interface remain undecided.

Existing characterization tests to inspect before implementation include
`src/detect/manifest/tests.rs`, `src/pane/agent_detection.rs`,
`src/terminal/state.rs`, `src/terminal/metadata.rs`, `src/api/subscriptions.rs`,
`src/workspace/aggregate.rs`, `src/ui/mobile.rs`, `src/app/agent_resume.rs`,
`src/persist/restore.rs`, and `tests/cli/agents.rs`. Test health was not established
by this inspection. Run the narrow relevant `just` recipes before changing code;
the roadmap requires explicit approval before the resource-heavy `just check`.
