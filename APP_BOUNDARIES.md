# App boundaries: Kitsune, Coven, and Djinn

## Overview

Coven's [private hook-file backend matrix](../coven/HOOK_FILE_BACKEND_CONTRACT.md)
now source-enables shared Linux/macOS file apply for all five managers. macOS is
provisional, eligible local APFS only and not natively qualified. All old Kit
non-Linux installers/assets/status/uninstall/recommend/update/bulk paths stay until
native qualification; receiver privacy, discovery and full management are separate
pending capabilities, not qualified by this backend.

Decision date: **2026-10-22**. Status: **accepted target; initial read-only
observation implemented in Coven; production opt-in terminal-border annotations
implemented in Kitsune; explicit participant groups/roles and a foreground publisher
implemented in Coven; Coven Claude/Codex/Copilot hook and Pi/OpenCode reference-extension cuts and Linux-only Kitsune new-install retirement
implemented; native group status/scheduling, automatic following, complete extraction,
and workspace annotations remain open**.
Keep three independently useful apps rather than tightly coupling them.

| App | Target responsibility |
| --- | --- |
| Kitsune | General multiplexer: PTY/process/terminal/session/pane operations and generic label/badge rendering. |
| Coven | Cross-runtime agent discovery/status, hooks, start/resume and agent restore policy; observations/provenance, groups/roles, orchestration and job progress. |
| Djinn | Djinn-native agent/session execution, configuration, tools, policy, transcripts, and results. |

Clarified implementation priority: **Coven owns agent management; Kitsune exposes
generic multiplexer controls, persistent side-menu presentation and app messaging.**
The default desktop [Integrations block](INTEGRATION_VIEWS.md) lists multiple
configured apps and renders the locally selected app's generic label/detail rows.
Coven supplies its own rows and consumes read-only selection messages. Kitsune
interprets no participant/role/agent meaning in that model. With no sources it shows
`No integrations configured`, never native-agent fallback. This advances ownership;
it does not claim legacy agent management has already been removed.

Kitsune's target is a good general multiplexer. Agent-specific discovery, status
interpretation, hooks, start/resume, agent-aware restore and job concepts move to
optional Coven integration. **Target: no Coven means fully functional multiplexing and no
agent-aware features.**

Current Kitsune still includes built-in agent detection, integration status,
agent navigation, startup/resume behavior, and Djinn lifecycle reporting support.
These are compatibility surfaces during transition, not already removed features.
The initial Coven observation bridge did not extract these features. The later
[Claude hook cut](CLAUDE_HOOK_MIGRATION.md) retires only Linux new Claude
installation/recommendations/updates. Non-Linux production installer/assets/result
type and recommendation/update/bulk paths remain until matching Coven adapters.
Process/screen fallback and agent restore remain on all platforms too. The authoritative all-provider matrix and
next-transfer queue are in Coven's `AGENT_MANAGEMENT_MIGRATION.md`
([optional sibling link](../coven/AGENT_MANAGEMENT_MIGRATION.md)).

The subsequent [Codex cut](CODEX_HOOK_MIGRATION.md) also retires Linux new
install/recommend/update/bulk paths before provider discovery, with no automatic
Coven execution. Coven manages two files, hooks.json/config.toml, with explicit
shared global feature enablement consent, not native approval or operational
certification. SessionStart receipts capture references only, always unknown even
fresh. Non-Linux legacy installers remain until matching Coven adapters; read-only
status/uninstall/old ingress remain compatible. Claude behavior/v2 receipt/errors/
statuses are preserved by shared private filesystem extraction; Codex's separate v1
two-file journal is ordered, not globally atomic. Normal Coven uninstall leaves the
shared flag enabled and never revives Kit commands; guarded rollback restores both
original bytes/modes/absence. Full discovery/start/resume/restore transfer is still
pending for every provider; generic views gain no provider-specific knowledge.

The subsequent [Copilot cut](COPILOT_HOOK_MIGRATION.md) completes scoped new Linux
hook-configuration ownership transfer: Coven manages explicit global
`settings.json` direct `hooks.SessionStart[]` command/bash/timeoutSec-10 handlers;
Kit new-install/recommend/update/bulk paths retire before provider discovery and
never auto-run Coven. Non-Linux Kit installer/assets/result type and recommendation/
update/bulk paths remain. All-platform Copilot detection/status/start/resume/restore,
legacy status/uninstall and old ingress remain compatibility. Pi/OpenCode scoped
reference-extension transfers are recorded below, not full management completion.

Coven accepts strict duplicate-free JSON only, deliberately refusing native JSONC.
It shares Claude's single-file v2 runner with separate Copilot namespaces, not
Codex's v1 two-file journal. Previews create no lock/files; actual single-file apply
may create a persistent lock before validation. Exact opt-in legacy migration
retains the script; normal uninstall never revives Kit, while guarded rollback
restores bytes/mode/absence. Native disable/approval/sandbox/trust stay unchanged.
Exact snake_case `session_id`/`hook_event_name: SessionStart` capture stays unknown
even fresh, without aliases/inference or Claude subagent suppression. Unix receipt
transport remains macOS-unvalidated and Windows-unsupported. The pinned native
documentation is not implementation/binary certification. Generated three-provider
commands reach real Coven end to end; historical Copilot checks are recorded in
Coven's migration record, not native operational acceptance or new seam totals.

The subsequent [Pi extension cut](PI_EXTENSION_MIGRATION.md) retires Linux new
install/recommend/update/bulk paths before discovery, without auto-running Coven.
Coven owns explicit existing-root reference-extension placement, not native
activation or full Pi management. **Migration drops working/blocked/idle to
reference-only capture**, always unknown. Non-Linux full-v7 installer/asset/result
and recommendation/update/bulk paths remain, as do all six targets and all-platform
legacy status/uninstall/old ingress/lifecycle until explicit migration. Pi
discovery/status/lifecycle/start/resume/restore transfer remains pending.

Pi uses Coven's separate strict three-field `coven.pi-extension.v1` envelope via
`report-extension --provider pi --event session_start`, not native hook JSON.
The native source pin is main `6e11a00a01a568ef0ae0df5074acf02fc107c34b`, declaring
1.5.2, not a release tag. hasUI includes TUI/RPC, not root/TUI proof or headless
capture. File placement does not establish native discovery/activation or change
approval policy. Exact-v7 consent and Coven's private two-leaf journal preserve
originals and rollback after uninstall; normal uninstall never revives Kit.
Stop providers/watchers before mutation; no atomic activation, autoload or
in-memory authority release. Upgrade receivers first: older readers reject Pi
records despite unchanged store-v1 fields and the now five-provider aggregate 128 cap.
The historical [Pi validation record](../coven/AGENT_MANAGEMENT_MIGRATION.md#pi-validation-table)
covers 107 integration checks and 9 Pi checks here (overlapping filters), plus
Coven fixture checks; this is not native-provider/platform certification.

The subsequent [OpenCode cut](OPENCODE_EXTENSION_MIGRATION.md) retires Linux new
install/recommend/update/bulk without running Coven. Coven owns explicit existing
absolute plugin/plugins root placement, not JSON settings, directories, packages,
probes or autodiscovery. Both native roots are accepted; no across-root dedupe/
authority exists, so do not double-install. Full v9 asset, legacy CLI/status/
uninstall/all six targets/old ingress and non-Linux new-install paths remain.
**Migration drops working/blocked/idle to references only**, unknown even fresh;
discovery/status/start/resume/restore remain pending, not full management completion.

Canonical anomalyco/opencode release v1.19.5 commit
`810c0eba1a7348036f8bdf61e6632d0c36c6876c` is authoritative; changed dev `611cee9`
unsupported. Named async event/dispose plugins run in the native server's exact
directory instance, not whole project or selected TUI/pane/attach scope. Executable/
socket must exist on that server, possibly remote. Full root info.id/sessionID
equality and undefined/omitted parentID are required; null/children reject without
cache/SDK reads/enumeration. Quiet resumed sessions need not emit either event.
Conservative DTO validation caps summary.diffs and permission at 256; capacity
precedes DTO work. FIFO 32 pending plus one active, incoming-full drop, no persistent
queue/deduplication/retry. Coven's strict three-string envelope requires matching
request/payload created/updated event. Provider/event maps interpretation, not
authenticated origin; native report remains the old three providers only.

Exact full-v9 bytes/SHA plus consent and shared private fixed-two-leaf v1 journaling
preserve immutable originals/rollback after uninstall. OpenCode namespaces protect
[Pi's frozen 887451b API/v1 receipts/generated bytes](../coven/tests/pi_extension_frozen_compatibility.rs).
Stop native servers/watchers before apply and restart native server after manual
apply, not just TUI attach; no automatic restart or in-memory release. Five providers
share unchanged five store-v1 fields/aggregate 128; receiver first because older
binaries reject new providers, with downgrade limits. Linux and provisional
eligible-local-APFS macOS file apply are source-enabled; macOS native qualification
and receiver privacy remain pending, Windows unsupported. The
[historical OpenCode scoped table](../coven/AGENT_MANAGEMENT_MIGRATION.md#opencode-validation-table)
records Kit 111 integration/10 OpenCode overlapping checks plus six legacy-asset
harness checks, Coven shared 20/workflows 14/Node 12 with Bun harness coverage.
The historical OpenCode Coven scope passed 236 focused Rust checks and lint/MSRV/build/package
checks, with receipt-timing limits recorded. Historical Pi totals are not additive and no
installed-native operational certification is claimed. See the
[Coven OpenCode contract](../coven/OPENCODE_EXTENSION_CONTRACT.md) for exact bounds.

## API / Interfaces

Coven adapters consume supported multiplexer/runtime capabilities; Kitsune and
Djinn have no core dependency on Coven or each other. Optional Kitsune decoration
and Djinn reporting are external integration edges, not startup prerequisites.
Coven must also work headless or with other multiplexers.

Use a small, versioned, capability-described external boundary. The initial Coven
adapter reads `session.snapshot`; Kitsune now exposes additive `pane.annotate`
with optional snapshot `annotations` capability/epoch discovery (binary protocol
18 unchanged). The separate [integration views](INTEGRATION_VIEWS.md) expose
`integration.view.open/replace/poll/close` and `snapshot.integration_views` for the
whole configured source list, without changing that binary version. Up to eight
ordered unique sources have independent leases/frames/revisions/queues. Local UI
selection and source IDs are routing, not authentication or execution authority.
Broader interfaces remain undecided. Presentation uses
generic terminal identities and text, not Djinn/Coven types or job interpretation.

Generic process capability: optional `snapshot.process_observation` v1 reports
Linux `pane_foreground_cohort`/`unclassified`; protocol 18 is unchanged (additive
source API metadata). Coven's production [discovery contract](../coven/AGENT_DISCOVERY_CONTRACT.md)
uses it for ephemeral all-22-family recognition/watch, not Kit agent APIs/hints.
Coven rejects Windows `selected_process`/`legacy_agent_aware` and macOS first cut.
Metadata is not authenticated platform/live attestation, a process tree/host catalog
or native generation. No matched Kit detector retirement: `detected_agent` also
previously coupled hook-owner confirmation, start readiness, restore/restored-idle
and mobile/navigation. The internal [typed evidence seam](DETECTOR_EVIDENCE_SEPARATION.md)
now separates foreground evidence, fallback publication and historical identity.
Historical/screen-only data cannot grant native ownership; restored readiness needs
a matching probe or accepted fallback. This is deliberate semantic hardening, not
blanket behavior compatibility. No external mode, Config or wire/snapshot field
changes, toggle/kill/hide controls ship. This is functional Coven recognition with temporary duplicate legacy
policy, not sole ownership/full status. Five prior Linux configuration retirements
and pending non-Linux qualification/capability gates are unchanged, not absence
of the provisional macOS file backend. [Validation](../coven/AGENT_MANAGEMENT_MIGRATION.md#discovery-validation-table)
records 201 focused Coven checks, 249 Kit API checks and lint/MSRV/release/package
validation; historical OpenCode results are non-additive. Read housekeeping can ensure a workspace/shell; no
zero-effects or live installed Kit command validation is claimed for this session.
Those discovery counts are historical, not added to the seam's **1,318 distinct Kit
checks** and separate **50 Coven cross-compatibility checks**. The evidence guide
records completed scoped validation, strict all-target lint, final confirmation
reruns and limits: ENOSPC-aborted workspace CLI selection, zero-match tab filter,
and independently reproduced stale golden/test-only corrections. No full-suite or
production UI change is claimed beyond intended evidence hardening.

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
integration views: independent configured apps -> per-source frames -> local app selector/rows -> per-source intents
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

### Migration scope

| Surface | Current scope / next owner |
| --- | --- |
| Desktop persistent lower menu | Generic Integrations block; empty/unavailable states never fall back to agents. |
| New Claude hook configuration | Linux: Coven-owned explicit settings operations; Kit new install refuses with Linux-qualified guidance before provider/config discovery and excludes recommendations/updates. Non-Linux: production legacy installer/assets/result type and recommendation/update/bulk paths preserved until matching Coven adapters. |
| New Codex hook configuration | Linux: Coven owns explicit two-file operations; Kit install/recommend/update/bulk paths retired before provider discovery. Non-Linux legacy installer/assets/result type preserved until matching adapters; see [guide](CODEX_HOOK_MIGRATION.md). |
| Independent Codex receipt inventory | Coven Unix SessionStart-only references, always unknown. Native trust state stays user/native-controlled. No working/idle lifecycle or operational certification. |
| New Copilot hook configuration | Linux: Coven owns explicit global settings.json direct startup handlers; Kit install/recommend/update/bulk paths retired. Non-Linux legacy installer/assets/result type and recommendation/update/bulk paths preserved until matching adapters. See [guide](COPILOT_HOOK_MIGRATION.md). |
| Independent Copilot receipt inventory | Coven Unix exact SessionStart references, always unknown; no native identity/activity/readiness or resume authority. |
| New Pi reference extension | Linux: Coven existing-root placement and exact opt-in v7 migration; Kit install/recommend/update/bulk retired. Non-Linux full legacy paths preserved. See [guide](PI_EXTENSION_MIGRATION.md). |
| Independent Pi reference inventory | Coven Unix strict envelope, session_start/getSessionId interpretation only, unknown even fresh. Migration drops working/blocked/idle; not full Pi management. |
| New OpenCode reference plugin | Linux: Coven explicit existing plugin/plugins root placement and exact opt-in full-v9 migration; Kit new install/recommend/update/bulk retired. Non-Linux full legacy paths preserved. |
| Independent OpenCode references | Coven Unix strict created/updated envelope; root DTO references only, unknown even fresh. Server exact-directory-instance scope, not full management or native activation proof. |
| Independent Claude receipt inventory | Coven Unix private receiver; Windows unsupported, macOS not validated. Not equivalent non-Linux hook-configuration support. |
| Agent detection, agent APIs, navigator/attention and mobile surfaces | Legacy compatibility remains; move management interpretation/model into Coven and retire matching surfaces incrementally. |
| Native references, agent restore/resume and remaining lifecycle hooks | Still Kitsune, including Claude process/screen fallback and restore; move to Coven adapters with subsystem-specific removal gates. |
| `integration.install` / `integration.uninstall`, fixed `IntegrationTarget`, legacy hook settings | All six target entries, status/uninstall/old ingress remain on all platforms. Claude/Codex/Copilot/Pi/OpenCode new installation remains non-Linux; legacy UI settings remain for other providers/platforms. Separate from generic provider registration. |

The confirmed target is **all** provider/source management on **all platforms**,
not only Claude/Linux or the desktop block. Compatibility is transitional; working
non-Linux paths are not dropped before replacement. No core Cargo dependency or
agent-native-effects ownership is added by generic presentation. Linux new
configuration ownership does not automatically migrate user
settings, release live pane authority or convert running sessions. Coven's report
inventory is last-reported, caller-supplied evidence, not current native identity,
readiness, completion or resume permission. Its optional independent view worker
uses generic snapshot/`integration.view.*` after receiver readiness; Kitsune failure
does not stop capture, and selection is read-only. See the
[migration guide](CLAUDE_HOOK_MIGRATION.md) for the earlier Linux retirement cut and
[backend matrix](../coven/HOOK_FILE_BACKEND_CONTRACT.md) for provisional macOS apply,
privacy, rollback and manual recovery boundaries.

The generic block is not a broker, plugin or script host. Reorder preserves retained
providers; removal/close/expiry clears only the affected provider. Producers renew
their own five-second lease only by successful replace/poll, independently of local
UI selection. Draw/queue fencing is not physical-pixel acknowledgement. See the
contract for per-provider quotas and at-most-once selection delivery.

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
