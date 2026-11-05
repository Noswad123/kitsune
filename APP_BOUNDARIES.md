# App boundaries: Kitsune, Coven, and Djinn

## Overview

Decision date: **2026-10-22**. Status: **accepted target; initial read-only
observation implemented in Coven; production opt-in terminal-border annotations
implemented in Kitsune; explicit participant groups/roles and a foreground publisher
implemented in Coven; native group status/scheduling, automatic following, extraction,
and workspace annotations remain open**.
Keep three independently useful apps rather than tightly coupling them.

| App | Target responsibility |
| --- | --- |
| Kitsune | General multiplexer: PTY/process/terminal/session/pane operations and generic label/badge rendering. |
| Coven | Cross-runtime agent discovery, observations/provenance, job/task-group membership, participant roles/assignments, orchestration, and job progress. |
| Djinn | Djinn-native agent/session execution, configuration, tools, policy, transcripts, and results. |

Kitsune's target is a good general multiplexer. Agent-specific discovery, status
interpretation, start/resume, and job concepts should move to optional Coven
integration. **Target: no Coven means fully functional multiplexing and no
agent-aware features.**

Current Kitsune still includes built-in agent detection, integration status,
agent navigation, startup/resume behavior, and Djinn lifecycle reporting support.
These are compatibility surfaces during transition, not already removed features.
The initial Coven observation bridge does not extract or remove these features.

## API / Interfaces

Coven adapters consume supported multiplexer/runtime capabilities; Kitsune and
Djinn have no core dependency on Coven or each other. Optional Kitsune decoration
and Djinn reporting are external integration edges, not startup prerequisites.
Coven must also work headless or with other multiplexers.

Use a small, versioned, capability-described external boundary. The initial Coven
adapter reads `session.snapshot`; Kitsune now exposes additive `pane.annotate`
with optional snapshot `annotations` capability/epoch discovery (binary protocol
18 unchanged). Broader future interfaces remain undecided. Presentation uses
generic terminal identities and text, not Djinn/Coven types or job interpretation.

The separate [generic command submission API](COMMAND_SUBMISSION.md) lets explicit
clients queue argument-encoded input to a pinned existing terminal's observed
available shell. Its optional snapshot capability is `command_submission.version`.
No agent registry, native arguments, lifecycle/readiness, task or UI presentation
authority is added. Coven may consume it for interactive start/resume without
calling Kitsune's agent-specific control methods; legacy behavior is unchanged.

Keep native identity, Coven participant role/assignment, job/group membership,
and presentation references distinct. An optional pane locator cannot be the sole
agent identity. Never assume one pane equals one native session.

Djinn is Coven's first runtime adapter, not a mandatory universal native agent
model. Future adapters advertise actual capabilities without fabricated normalized
guarantees. Read-only display is not permission to launch, send, resume, or stop;
effects require separately supported, explicitly authorized adapters.

### Separate generic annotations: first production slice

[ANNOTATIONS.md](ANNOTATIONS.md) documents manual source opt-in and the generic
API/CLI contract. Producers write bounded, revisioned, TTL-limited plain text,
pinned to a current pane's `TerminalId` and annotation epoch. Source order governs
priority; clear/expiry retains watermarks without lower-source fallback. Source
labels are not authentication: any existing local API caller can claim an enabled
source. Coven now implements an explicit foreground group publisher in its own
repository; optional [contract/quick start](../coven/GROUPS_CONTRACT.md), not a dependency.

Kitsune/user own layout, rendering, clipping, and quotas. Text uses only spare
existing top-border cells, preserving manual titles and terminal space. Single-pane
layouts have no border; disabled borders hide labels. No panels, scripts, widgets,
workspace annotations, agent identity, group membership, seen/notification changes,
native success facts, or terminal-control permission are added. The
[prototype notes](ANNOTATION_MODEL_PROTOTYPE.md) are historical; workspace targets
remain test-only. Coven's group declarations are separate from Kitsune's generic
presentation; native group status/scheduling, automatic following, broader annotations,
and extraction remain open.

## Data Flow

Independent paths, not a required round trip:

```text
observation: Kitsune -> Coven adapter -> sourced observations
annotation: user-enabled producer -> pane.annotate -> Kitsune terminal border
```

Kitsune supplies multiplexer evidence; runtimes retain native fact authority.
Coven implements explicit participant groups/display roles and foreground labels;
native group lifecycle/job progress interpretation and scheduling remain future work.
Observed activity is not job acceptance. Pane presence is not success. Screen
matches are heuristic evidence, never lifecycle truth.

## Edge Cases

- Missing, unreachable, or incompatible Coven must not break Kitsune startup,
  terminals, pane/session operations, or Djinn's native operation.
- Hide stale external badges or mark them unknown; never show stale active status
  as current truth. Loss of a pane is not proof a native session stopped.
- Invalid annotation writes are rejected without replacing accepted content.
  Timer-driven expiry hides labels even on quiet terminals; disconnect is not
  immediately detected, so accepted text can last until TTL. Display grants no
  terminal input/start/stop permission, even when the source also observes Kitsune.
- Snapshot building takes `&self`, but existing dispatch expires metadata, drains
  events, syncs titles, and may ensure a default workspace (potentially spawning
  a shell). A client sending only a read method cannot promise zero server-side
  effects. Native references may be persisted and server freshness remains unknown.
- Coven's headless/other-multiplexer operation is the target, not a claim that
  every existing workflow already implements it.
- Observation and grouping do not require a scheduler, daemon, plugin framework,
  or native admission design. Execution/cancel remain separate capabilities.
- Coven's existing safety contracts remain valid; this decision cannot authorize
  automatic execution of old preparations or turn logs into replay authority.

## Example usage

Current optional workflow: use Kitsune for ordinary shells with Coven
absent. Optionally connect Coven to group already-running native agent sessions
into a declared review group. Coven's explicit foreground publisher sends generic
group/role text; Kitsune renders it in available border cells without
knowing the job or agent model. If Coven disconnects, terminal work continues and
external badges expire by TTL, not immediate disconnect detection; no agent is
launched or stopped. Native group status/scheduling and automatic following are not
shipped. See the optional [Coven guide](../coven/GROUPS_CONTRACT.md); the independent
manual build-label example is in [ANNOTATIONS.md](ANNOTATIONS.md).

## Transition

1. Inspect Kitsune's existing agent surfaces and dependencies.
2. Characterize existing behavior and protected compatibility before extraction.
3. Initial single-endpoint observation is implemented in Coven only; characterize
   generic surfaces before extraction. Kitsune's built-ins remain.
4. First optional generic terminal-border annotation slice implemented; broader
   workspace/presentation work remains open; explicit Coven foreground publishing is implemented.
5. Explicit participant groups/roles implemented in Coven before more worker/worktree
   implementation; native group status/scheduling, automatic following, and extraction remain open.

Maintain current Djinn rollout as compatibility during this transition; do not
deepen agent-specific core coupling or assume an immediate removal. Older Coven
execution work is deferred, not invalidated, and still needs its own safety gates.

## Related files

- [README.md](README.md) — current features.
- [KITSUNE_ROADMAP.md](KITSUNE_ROADMAP.md) — target and compatibility backlog.
- [docs/next/README.md](docs/next/README.md) — current integration details.
- [AGENT_TRACKING_INSPECTION.md](AGENT_TRACKING_INSPECTION.md) — source findings.
- Optional sibling navigation: [Coven observation contract](../coven/KITSUNE_OBSERVATION_CONTRACT.md),
  not a Kitsune dependency.
- Optional developer navigation: [Coven decision](../coven/APP_BOUNDARIES.md),
  [Djinn decision](../djinn/APP_BOUNDARIES.md). Sibling checkouts are not required.
