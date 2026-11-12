# Kitsune fork roadmap

## Confirmed all-agent migration — first functional cut

All agent-specific discovery/status interpretation, hooks, start/resume and agent
restore move to Coven. Kitsune retains generic multiplexing and optional generic
presentation/messaging, not a permanent agent management model. Coven's
`AGENT_MANAGEMENT_MIGRATION.md` is authoritative for ownership and next transfers
([optional sibling navigation](../coven/AGENT_MANAGEMENT_MIGRATION.md)).

Implemented: **new Claude hook installation retired here on Linux only**, with immediate Coven
guidance before provider/config discovery or install logging, no startup/update/
bulk recommendation on Linux, and Linux API `integration_install_failed` guidance
explicitly saying “on Linux”. Non-Linux production `legacy_install_claude`, assets/
result type and recommendation/update/bulk paths remain until matching Coven
adapters are ready. Coven now owns Linux new hook configuration through
explicit settings plan/install/uninstall/rollback, independent private Unix receipt
capture/redacted inventory, and optional generic-view rows. See
[CLAUDE_HOOK_MIGRATION.md](CLAUDE_HOOK_MIGRATION.md).

Current: Coven's [shared private hook-file backend](../coven/HOOK_FILE_BACKEND_CONTRACT.md)
source-enables explicit macOS file apply for all five managers, provisional eligible
local APFS only, not native qualification. All old Kit non-Linux installers/assets/
status/uninstall/recommend/update/bulk paths stay. Receiver privacy, discovery and
full management are separate pending capabilities.

Pending: native macOS qualification and remaining non-Linux capabilities before retiring working Kit paths;
separate Djinn handling and remaining provider/source management; discovery/status and process/screen
fallback; provider start/resume; agent-aware restore/reference ownership, dedupe
and deferred launch; matching legacy APIs/navigation/mobile retirement. **Claude
fallback and restore remain Kitsune**, as do unmigrated-session legacy status,
uninstall and hook ingress on all platforms, and all six target entries/legacy UI
settings. Coven's receiver is Windows-unsupported, macOS unvalidated; unsupported
hook apply refuses before effects, not equivalent replacement support. No user config/running sessions are automatically
migrated. Preserve generic terminal/session restore, native-runtime fact authority
and independent app startup. Scoped seam validation and its limits are recorded below;
no new full-suite, live-provider or cross-platform acceptance is claimed here.
The ultimate target remains all agent features/platforms in Coven, not permanent
legacy ownership or a core Cargo dependency/native-effects model in generic Kit views.

## Target boundary and priority — 2026-10-22

### Subsequent Codex hook transfer

New Codex install/recommend/update/bulk paths are now also retired **on Linux
only**, before provider/config discovery and without auto-running Coven. Non-Linux
legacy installer/assets/result type remain until matching Coven adapters. Read-only
legacy status, uninstall and old hook ingress remain compatible. See
[CODEX_HOOK_MIGRATION.md](CODEX_HOOK_MIGRATION.md) for Coven's two-file journal,
shared global hook enablement consent, native approval not granted and guarded
rollback/recovery. Config writes are not native operational certification.

Coven captures only SessionStart references for Codex, always unknown even fresh;
Claude behavior/v2 receipts/errors/statuses are unchanged after shared filesystem
extraction. Optional dynamic-provider views keep `integration.view.*` unchanged
and give Kit no provider model. Normal Coven uninstall leaves global hooks enabled,
never restoring Kit commands; explicit guarded rollback restores both original
images/modes/absence. Codex moves out of the wholly pending
hook-owner group; full discovery/status/start/resume/restore transfer is still
pending across **all providers**. Non-Linux adapter work now covers Claude,
Codex, Copilot, Pi and OpenCode before retiring working Kit paths. Earlier owner-reported 16 Codex
workflows + 7 fault tests are historical scoped evidence, not fresh Copilot/full-suite/
platform/native-provider acceptance; see the parent migration record's historical
176 Coven/330 Kit results and separately recorded historical Copilot results.

### Subsequent Copilot hook transfer

Scoped new Linux Copilot hook-configuration ownership is complete in Coven.
Kit new-install/recommend/update/bulk paths are retired before provider discovery,
without automatically invoking Coven. Coven manages explicit global `settings.json`
direct `hooks.SessionStart[]` command/bash/timeoutSec-10 handlers; see
[COPILOT_HOOK_MIGRATION.md](COPILOT_HOOK_MIGRATION.md). Non-Linux production
installer/assets/result type and recommendation/update/bulk paths remain until
matching Coven adapters. All-platform detection/status/start/resume/restore,
legacy status/uninstall/old ingress and all six targets remain compatibility.
OpenCode's subsequent reference-plugin placement transfer is recorded below.

Coven uses Claude's shared single-file v2 runner with per-provider namespaces;
Codex's v1 two-file journal stays separate. Strict duplicate-free JSON only,
deliberately refusing native JSONC. Exact opt-in legacy migration retains the script;
normal uninstall never revives Kit, guarded rollback restores bytes/mode/absence.
Previews create no lock/files; actual single-file apply may create a persistent
lock before validation. Native disable/approval/sandbox/trust remain unchanged.
Exact `session_id`/`hook_event_name: SessionStart` receipts stay unknown even fresh,
without aliases/inference or Claude subagent suppression. Unix transport remains
macOS-unvalidated and Windows-unsupported; provisional macOS file apply does not
qualify receiver privacy or all-platform management.

The native documentation pin is not implementation/binary certification. Generated
three-provider commands reach the real Coven receiver end to end; scoped historical
Copilot checks are recorded in Coven's migration record, not new seam totals. Historical
176 Coven/330 Kit evidence is not a new Copilot count or native operational result.

### Subsequent Pi reference-extension transfer

New Pi install/recommend/update/bulk paths retire **on Linux only**, before
discovery and without automatically running Coven. Coven owns explicit existing
extension-root plan/install/uninstall/rollback; see [migration guide](PI_EXTENSION_MIGRATION.md)
and optional sibling [contract](../coven/PI_EXTENSION_CONTRACT.md).
**Migration drops working/blocked/idle reporting to reference-only capture**,
always unknown, not full Pi management or native activation proof. Non-Linux
full-v7 asset/installer/result and recommendation/update/bulk paths remain; all
six targets, all-platform status/uninstall/old ingress/lifecycle persist until
explicit migration. Pi discovery/status/lifecycle/start/resume/restore remain pending.

The source pin declares 1.5.2 on main, not a release tag. hasUI includes TUI/RPC,
not root/TUI proof or headless capture. The strict Coven envelope uses separate
report-extension, never invented native Pi JSON. Exact-v7 consent and a private
two-leaf v1 journal preserve immutable originals and rollback after uninstall;
normal uninstall never revives Kit. No approval changes, atomic activation,
autoload or in-memory authority release. Upgrade receiver first: older readers
reject Pi store records even with schema 1 unchanged. The historical
[Pi validation record](../coven/AGENT_MANAGEMENT_MIGRATION.md#pi-validation-table)
covers 107 integration and 9 Pi checks here (overlapping filters), strict lint and
Coven fixture checks; earlier Copilot scopes remain historical, not additive totals.

### Subsequent OpenCode reference-plugin transfer

New Linux OpenCode install/recommend/update/bulk paths retire before discovery,
without running Coven. See [migration guide](OPENCODE_EXTENSION_MIGRATION.md) and
optional sibling [contract](../coven/OPENCODE_EXTENSION_CONTRACT.md). Coven owns
explicit existing absolute plugin/plugins root placement, not settings/directories/
packages/probes/autodiscovery. Both roots are native-supported; no across-root
dedupe/authority, so do not double-install. **Migration drops working/blocked/idle
to references only**, always unknown, not full OpenCode management completion.
Non-Linux full-v9 new-install paths and all-platform legacy CLI/status/uninstall/
all six targets/old ingress remain; discovery/status/start/resume/restore pending.

Authoritative canonical anomalyco/opencode release v1.19.5 commit
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`, not changed unsupported dev `611cee9`.
The named async plugin event/dispose hooks run in the native server's exact directory
instance, not whole project or selected TUI/pane/attach scope. Remote servers need
executable/socket there. Full root DTO info.id/sessionID equality and undefined/
omitted parentID only, no cache/SDK reads/enumeration; quiet resumed sessions need
not emit either event. Conservative DTO validation caps summary.diffs and permission
at 256. Capacity precedes DTO work; FIFO 32 pending plus one active, incoming-full
drop, no persistent queue/deduplication/retry. Separate strict three-string envelope
requires matching request/payload event; interpretation is not authenticated origin.
Native report remains Claude/Codex/Copilot only, and Pi's envelope stays separate.

Shared private fixed-two-leaf v1 engine has separate OpenCode namespaces and
preserves [Pi 887451b frozen API/receipt/generated bytes](../coven/tests/pi_extension_frozen_compatibility.rs).
Exact full-v9 bytes/SHA plus consent, immutable originals and rollback after
uninstall are required. Stop native servers/watchers before apply; restart native
server after manual apply, not just TUI attach. No automatic restart/in-memory
release. Five providers share unchanged five store-v1 fields/aggregate 128 cap;
receiver first, older binaries reject new providers and downgrade is limited.
Linux and provisional eligible-local-APFS macOS file apply are source-enabled;
macOS native qualification/receiver privacy remain pending, Windows unsupported.
The [historical OpenCode scoped table](../coven/AGENT_MANAGEMENT_MIGRATION.md#opencode-validation-table)
records Kit 111 integration/10 OpenCode overlapping checks, six legacy-asset checks,
Coven shared 20/workflows 14/Node 12 with Bun harness coverage. The historical OpenCode Coven
scope passed 236 focused Rust checks and lint/MSRV/build/package checks, with
receipt-timing limits recorded; earlier Pi totals are not additive. No installed-native operational
or cross-platform certification is claimed.

Generic process capability is implemented as optional `snapshot.process_observation`
v1: Linux `pane_foreground_cohort`/`unclassified`, additive source API metadata with
protocol 18 unchanged. Coven's production [discovery/watch](../coven/AGENT_DISCOVERY_CONTRACT.md)
recognizes all 22 literal families without Kit agent APIs/hints, native identity
or activity/status authority; Windows `selected_process`/`legacy_agent_aware` and
macOS are rejected first cut. Metadata is not authenticated platform/live
attestation, process tree/host catalog or native generation. **No matching detector
retirement yet.** The internal [typed evidence seam](DETECTOR_EVIDENCE_SEPARATION.md)
is implemented, separating foreground-owner evidence, fallback and restored history.
Historical/screen-only data cannot mint native ownership; restored readiness needs
a matching probe or accepted fallback. This deliberately hardens observable semantics,
not blanket compatibility. No external mode, new Config, wire/API/snapshot field
changes or toggle/kill/hide controls ship. Functional Coven recognition temporarily
duplicates legacy policy, not completed sole ownership/full status. Five prior Linux
configuration retirements/non-Linux pending adapters are unchanged. [Scoped
validation](../coven/AGENT_MANAGEMENT_MIGRATION.md#discovery-validation-table) records
201 focused Coven checks, 249 Kit API checks and lint/MSRV/release/package checks;
earlier OpenCode results remain historical,
not additive. Read housekeeping may ensure a default workspace/shell; no actual
live installed Kit commands were used in this documentation session. Discovery counts
are historical, not added to **1,318 distinct Kit seam checks** or separate **50
Coven cross-compatibility checks**. The evidence guide records completed scoped
API/UI/input/workspace validation and strict all-target lint, final confirmation
reruns, ENOSPC-aborted workspace CLI selection, a zero-match filter and reproduced
stale golden/test-only corrections. No full-suite or new production UI behavior
is claimed beyond intended evidence hardening.

[APP_BOUNDARIES.md](APP_BOUNDARIES.md) records the accepted target: Kitsune owns
general multiplexer operations and generic labels/badges; optional Coven owns
agent-specific discovery/status interpretation/hooks/start/resume/agent restore and job concepts.
Without Coven, the target is fully functional multiplexing with no agent-aware
features. Current built-in agent integrations remain during transition; this is
not full removal; Linux new Claude/Codex/Copilot hook and Pi/OpenCode extension installation is the targeted retirement above.

Initial single-endpoint observation is implemented in Coven only; built-in
interpretation remains here. The first opt-in generic terminal-border annotation
slice and Coven's explicit existing-session groups/publishers are implemented.
Next: move management model/adapters into Coven through generic surfaces, while
characterizing affected legacy subsystems before removal. Do not require a
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
targets still test-only. Workspace annotations and richer progress hints remain
open; Coven publishing and grouping are implemented separately. Agent extraction
remains open; no widgets, unrequested UI space or terminal-control authority follows
from labels. Persistent application rows use the separate Integrations block below.

This fork started as a Kitsune-branded distribution of Herdr. Completed product
baseline and feature behavior live in `README.md` and `docs/next/README.md`.
This file tracks only work that is still intentionally open.

The separate [terminal engine roadmap](TERMINAL_ENGINE_ROADMAP.md) evaluates Zig
build convenience and a possible Rust backend. It is proposed work, not an approved
engine rewrite or a prerequisite for agent-policy extraction; Ghostty remains the
default, with the matching Zig toolchain as the immediate build solution.

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

### Coven-owned management through generic presentation and messaging

The [integration-view bridge](INTEGRATION_VIEWS.md) implements the default generic
desktop lower `Integrations` block, with a vertical app selector, selected-app
label/detail rows and read-only selection roundtrip. Empty config shows
`No integrations configured`, not native agents. Up to eight independent sources
use `integration.view.*` and whole-list `snapshot.integration_views` discovery;
reorder preserves retained state, and removal/expiry/close is per provider. Extend
Coven's management model/runtime adapters through generic interfaces, then retire
the matching built-in detection/status/navigation/start/resume surfaces in steps.
Do not add Coven semantics to Kitsune, or let characterization alone replace this
ownership milestone. Existing legacy restore/failure tests remain subsystem
removal gates; full-suite and supported-platform validation remain separate.

This is not complete agent removal: agent APIs/detection, mobile UI, remaining hooks,
restore/resume and other providers' legacy hook installers remain on applicable
platforms, as does non-Linux Claude/Codex/Copilot installation; Linux new Claude/Codex/Copilot
installation is retired separately (see the
[migration matrix](APP_BOUNDARIES.md#migration-scope)). Keep `IntegrationTarget` and
`integration.install/uninstall` separate from generic provider configuration.
Retire management through Coven-owned model/adapters, not a permanent test-only
gate or another Kitsune agent registry. The obsolete unreleased singleton config
and methods have no aliases; manual `integrations.sources` replaces that prototype.

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

### Upgrade-safe executable paths for pane integrations

Deferred bug fix: Neovim Ctrl-H/J/K/L navigation can fail at a split boundary with
`E5108` / `ENOENT` when `KITSUNE_BIN_PATH` points to a removed Homebrew Cellar
binary, observed as `/home/linuxbrew/.linuxbrew/Cellar/kitsune/0.1.0-dev/bin/kitsune (deleted)`.
The dotfiles `pane-navigator.lua` currently trusts that override verbatim. A
still-running Kitsune process after an upgrade is the suspected source of the
stale value; confirm how Kitsune exports the path before changing runtime code.

Make executable discovery upgrade-safe: honor `KITSUNE_BIN_PATH` only when it is
executable, otherwise resolve `kitsune` on `PATH`. If neither is usable, skip the
external pane-focus request with a clear warning rather than an uncaught Lua
spawn error. Handle spawn failures even after validation, preserve Neovim split
navigation and the existing pane/session targeting, and audit the other dotfiles
consumers (`lua/util/harness/opencode.lua` and `kpm.py`) for the same assumption.
Do not simply strip ` (deleted)`; the underlying versioned path may also be gone.

Validate valid overrides, stale/deleted overrides with a working PATH fallback,
no executable available, and an executable disappearing between lookup and
spawn. Include an upgrade-with-a-running-session regression check. Until fixed,
save work and fully restart the Kitsune server/session after upgrades; an
existing Neovim can clear `vim.env.KITSUNE_BIN_PATH` if `kitsune` is on its PATH.

### Validation hygiene audit

Audit current full-suite behavior before listing blockers. Several previously
observed local failures have been fixed or may be stale, so future validation
work should start by running the narrowest useful check, recording any live
failure, and fixing only confirmed blockers. Do not run the resource-heavy
`just check` path without explicit approval.

## Working approach

Keep changes small and validate after each product slice so the fork remains
runnable throughout the transition.
