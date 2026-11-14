# Kitsune
![kitsune](img/kitsune.png)

Kitsune started as a personal fork of Herdr, a terminal-based runtime for running coding agents in panes, tabs, workspaces, and detached sessions.

This fork is branded around the `kitsune` command and local Kitsune runtime state. It intentionally removes Herdr's public website, hosted updater, and plugin marketplace surfaces.

[APP_BOUNDARIES.md](APP_BOUNDARIES.md) records the 2026-10-22 target: a general
multiplexer, with agent-aware behavior moving to optional Coven integration.
Built-in management remains during transition, except new Claude/Codex/Copilot hook and Pi/OpenCode reference-extension installation
on Linux is now retired here and owned by Coven. macOS/Windows installation and
recommendation/update paths remain until matching Coven adapters. Complete
all-platform extraction is not shipped.

Initial opt-in observation is implemented in Coven only, using one explicitly
selected existing `session.snapshot` API socket (protocol 18). See the
[source inspection](AGENT_TRACKING_INSPECTION.md) and optional sibling
[Coven contract](../coven/KITSUNE_OBSERVATION_CONTRACT.md); neither is a dependency.
This is not a zero-server-effects promise: existing request housekeeping can
expire metadata, drain events, sync titles, or ensure a default workspace,
potentially spawning a shell. No live connections were run for this docs update.

[Generic terminal annotations](ANNOTATIONS.md) are now implemented as a separate,
opt-in path: enable a source in config, read snapshot identities/epoch, and use
`kitsune pane annotate` to append bounded, expiring plain text in spare existing
top-border cells. Single-pane layouts have no border; labels need an existing
split layout with borders enabled. Manual titles and terminal space are untouched.
Sources are logical labels, not authentication. Coven's separate foreground
publisher supplies group/role text; Kitsune annotations add no workspace annotations,
group model, widgets, new panels or agent-feature extraction.
The [prototype notes](ANNOTATION_MODEL_PROTOTYPE.md) remain historical context.

Optional generic `snapshot.process_observation` advertises Linux v1 or macOS v2,
both `pane_foreground_cohort`/`unclassified`, with protocol 18 unchanged. macOS v2
names exact `argument_policy: darwin_procargs2_lp64_v1`. Coven's
[discovery/watch](../coven/docs/AGENT_DISCOVERY_CONTRACT.md) admits these tuples for
all-22-literal-family hints, ignoring Linux's optional marker, rejecting old macOS
v1 even with the marker, and rejecting Windows `selected_process`/`legacy_agent_aware`.
macOS is **source-enabled/provisional, not native runtime certification**.
Generic `pane.process_info` uses isolated `macos_observation`/`macos_procargs`;
legacy detector parsing and environment hints remain unchanged. No detector
retirement or provider/receipt contract change follows.

Peer labels/PIDs grant no local OS/PID authority. Coven reports
`acquisition_completeness: unknown`: mutable arguments and unreadable/failed
acquisition can leave undetectable omissions. Linux rooted-tree and macOS
process-group sampling are different best-effort cohorts, not complete catalogs.
See [app boundaries](APP_BOUNDARIES.md#generic-process-acquisition) for reviewed
XNU layout, bounds and source files. Actual Apple Rust-target source-checks are
not SDK C/native link/runtime validation; missing SDK `TargetConditionals.h` still
blocks native validation. No builds, tests or live endpoint use occurred in this
documentation update. Historical [evidence-seam validation](DETECTOR_EVIDENCE_SEPARATION.md)
is separate, not a current acquisition total. Read housekeeping may create a
default workspace/shell; the five Linux configuration retirements stay unchanged.

## Integrations: generic presentation and messaging

### Normal native-agent invocation (one-time opt-in)

For the optional [normal workflow](../coven/docs/NORMAL_AGENT_WORKFLOW.md), enable
Coven with compatible Kitsune/Coven builds:

```toml
[integrations]
sources = ["coven"]
```

Kitsune invokes `coven doctor status --json` through startup PATH, negotiates
versioned capabilities and retains the selected native executable/default root.
No runtime command arrays are required. Existing explicit runtime blocks override
these defaults; they are not silently migrated. Unknown source IDs remain
presentation-only. `[integrations.coven] auto_start = false` preserves that older
behavior for manual Coven publishers, disabling the implicit runtime and inferred
ownership. See [startup semantics](INTEGRATION_VIEWS.md).

Then restart Kit, open new panes, select **Coven** in Integrations and invoke
`djinn`, `codex`, `copilot`, etc. normally. Djinn needs an updated native full-TUI
build; other providers need separate explicit hook/extension setup in the guide.
No Coven launcher/group/artifact join, manual workspace initialization or socket
coordination is needed. The optional `coven integration plan` / `enable --apply`
[managed setup](../coven/docs/INTEGRATION_SETUP_CONTRACT.md) retains advanced
explicit executable/root pinning and guarded undo. Debug Kit uses the same `kitsune` config
directory, not `kitsune-dev`. Existing unmanaged runtimes require a manual handoff,
not adoption/overwrite. No provider auto-install or native trust override occurs.

The generic helper is server-only (`--no-session` refuses). External ownership
suppresses all legacy agent-status/detector and automatic-resume paths even on
helper failure or invalid/unreadable startup config, without legacy fallback.
Runtime changes, degraded-helper recovery and rollback require deliberate server
restart; activation also needs new panes/provider reload. No automatic helper
restart; generated bootstrap isolates concurrent Kit API addresses automatically.
Crash/partial state refuses reuse and needs deliberate manual recovery, not
self-healing. Guarded `coven integration disable --apply` restores only the exact
managed preimage; later edits refuse, and ownership handoff still needs restart.
Select **Coven** for attributed plain rows, not old status widgets, pane focus/
sidebar agents or mobile completions. Facts expire within 30 seconds independently
of the five-second lease; sessions are unbound/unattested. Djinn needs an updated
native full-TUI build; reference-only providers remain unknown. Default legacy
compatibility remains; full migration/native macOS qualification is not claimed.

The desktop lower side menu is a generic [Integrations block](INTEGRATION_VIEWS.md),
not an agent panel. With no providers it shows `No integrations configured` and
never falls back to detected agents. Manually configure `[integrations]` with
`sources = ["coven", "build"]` (up to eight ordered unique ASCII source IDs), then
select an app locally from its vertical list to see its label/detail rows.
Independent producers use `integration.view.open/replace/poll/close` with per-source
five-second leases, frames and selection queues. Missing/expired apps show
`Unavailable`; other producers and terminals keep working.

Coven's `group present` supplies its own group/participant model without Kitsune
interpreting agents or roles. Use `[integrations.coven] auto_start = false` and no
explicit runtime for that manual publisher rather than starting two Coven owners.
This presentation-only path changes default desktop lower-menu
presentation, not all legacy detection, APIs, mobile UI, hooks or restore behavior.
The obsolete `[external_view]` config is diagnosed/ignored, not automatically
rewritten. See the contract for discovery, bounds and at-most-once, non-authorizing
selection messaging.

## Runtime identity

Kitsune has its own runtime identity so it can coexist with an installed Herdr session on the same machine.

- Command: `kitsune`.
- Runtime config, sockets, logs, and environment variables use `kitsune` / `KITSUNE_*` names.
- Panes launched by Kitsune receive Kitsune-owned workspace, tab, and pane environment.
- The default config inspection and reload workflows use Kitsune paths and sockets.

Internal module/type names may still reference Herdr where keeping the fork easy to compare with upstream is more valuable than a broad rename.

## Build and run

The current terminal engine is vendored libghostty-vt and requires the matching
Zig toolchain (currently 0.15.2). `ZIG=/absolute/path/to/zig` selects an explicit
compiler without a global installation. A possible Rust backend and alternatives
to the local compiler requirement have their own
[terminal engine roadmap](TERMINAL_ENGINE_ROADMAP.md); no engine replacement has
been selected or shipped.

```bash
cargo build --bin kitsune
./target/debug/kitsune
```

The local helper justfile installs the debug binary into the scoped development
directory `~/.local/libexec/arcana-dev/kitsune/bin`, separate from Arcana-managed
commands:

```bash
just install
"$HOME/.local/libexec/arcana-dev/kitsune/bin/kitsune"
```

Use `INSTALL_DIR="/your/dev/bin" just install` to override the destination explicitly.
The development directory is not added to `PATH`; use the full path to select it.

## Kitty graphics

Kitty graphics work by default when using a compatible outer terminal. Restart
existing Kitsune servers after updating to enable graphics in their panes. The
legacy `experimental.kitty_graphics` flag is no longer necessary and can be removed
from your config; either value is ignored with the normal unknown-key diagnostic.

## Session recall

Kitsune includes a `session_recall` action bound by default to `prefix+shift+s`. The action opens the external session recall TUI in a popup and invokes it with the native Kitsune backend.

The helper receives `KITSUNE_ENV`, `KITSUNE_BIN_PATH`, `KITSUNE_SOCKET_PATH`, `KITSUNE_CLIENT_SOCKET_PATH`, and active workspace/tab/pane context. Herdr, tmux, and Kitsune are selected as normal helper backends; Kitsune does not mirror one backend's environment variables into another backend.

## Agent navigation

Kitsune includes a session navigator and an agents-only selector:

- `prefix+g` opens the full session navigator.
- `prefix+a` opens the navigator directly in agents-only scope.
- `Tab` toggles the navigator between full tree and agents-only scope.
- In agents-only scope, `r` renames the selected agent pane's pane label.

## Legacy agent hooks

New **OpenCode** plugin install/recommend/update/bulk paths retire **on Linux only**,
before discovery and without running Coven. See [migration guide](OPENCODE_EXTENSION_MIGRATION.md)
and optional sibling [contract](../coven/OPENCODE_EXTENSION_CONTRACT.md). Coven owns
explicit existing absolute `plugin` or `plugins` root placement, not JSON settings,
directory/package creation, probes or autodiscovery. Both roots are native-supported;
do not double-install, since no across-root dedupe/authority exists. Full v9 asset,
legacy CLI/status/uninstall/all six targets/old ingress and non-Linux legacy
new-install paths remain. **Migration drops working/blocked/idle to references
only**, unknown even fresh, not full OpenCode management completion.

Authoritative canonical anomalyco/opencode release: **v1.19.5**, commit
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`; changed dev `611cee9` unsupported.
The named async plugin's event/dispose hooks run in the native server's exact
directory instance, not whole-project or selected TUI/pane/attach scope. Remote
servers need executable/socket there. Full root DTO info.id/sessionID equality and
undefined/omitted parentID are required, with no cache/SDK reads/enumeration; quiet
resumed sessions need not emit created/updated events. Conservative DTO validation
caps summary.diffs and permission at 256; capacity precedes DTO work. FIFO 32 pending
plus one active, incoming-full drop, no persistent queue/deduplication/retry.
Strict three-string `coven.opencode-extension.v1` envelopes require matching request/
payload event; provider/event maps interpretation, not authenticated origin.
Native `agent report` remains Claude/Codex/Copilot only; Pi's envelope is separate.

Exact full-v9 bytes/SHA and consent are required. Shared private two-leaf v1
journaling preserves immutable originals and rollback after uninstall, with separate
OpenCode namespaces and [Pi 887451b frozen compatibility](../coven/tests/pi_extension_frozen_compatibility.rs).
**Stop native servers/watchers before apply; restart the native server after manual
apply**, not just TUI attach. No automatic restart or in-memory authority release.
Five providers retain store-v1's same five fields/aggregate 128 cap; upgrade receiver
first, since older binaries reject new providers and stored records limit downgrade.
Linux and provisional eligible-local-APFS macOS file apply are source-enabled;
macOS native qualification/receiver privacy remain pending, Windows unsupported.
The [historical OpenCode scoped table](../coven/AGENT_MANAGEMENT_MIGRATION.md#opencode-validation-table)
records Kit **111 integration/10 OpenCode checks** (overlapping), plus **six legacy-asset
harness checks**; Coven shared 20/workflows 14/Node 12 with Bun harness coverage.
The historical OpenCode Coven scope passed 236 focused Rust checks and lint/MSRV/build/package
checks, with receipt-timing limits recorded. Historical Pi totals are not additive;
no installed-native operational or cross-platform certification is claimed.

New **Pi** extension install/recommend/update/bulk paths are retired **on Linux
only**, before discovery and without auto-running Coven. Coven owns explicit
existing-root reference-extension placement; see [PI_EXTENSION_MIGRATION.md](PI_EXTENSION_MIGRATION.md)
and optional sibling [contract](../coven/PI_EXTENSION_CONTRACT.md). **Migration
drops working/blocked/idle reporting to references only**, always unknown. It is
not full Pi management or native activation proof. Non-Linux full-v7 paths and
all-platform status/uninstall/old ingress/lifecycle remain until explicit migration.
All six targets remain. Pi discovery/lifecycle/start/resume/restore remain pending.

Pi reports a strict Coven three-field envelope through `report-extension`, not
native hook JSON. hasUI includes TUI/RPC, not root/TUI proof or headless capture;
the pinned source declares 1.5.2 on main, not a release tag. Exact-v7 bytes/consent
are required. Coven's separate journal retains originals and rollback after
uninstall; normal uninstall never revives Kit. Stop providers/watchers before
mutation: no approval changes, atomic activation, autoload or live authority
release. Upgrade receiver first because older binaries reject Pi records despite
schema 1. The historical [Pi validation record](../coven/AGENT_MANAGEMENT_MIGRATION.md#pi-validation-table)
covers 107 integration and 9 Pi checks here (overlapping filters), plus Coven
fixture checks, not native operational or cross-platform acceptance.

New **Copilot** hook installation/recommendation/update/bulk paths are retired
**on Linux only**, with immediate Coven guidance before discovery or mutation.
Non-Linux paths, status inspection, explicit legacy uninstall and old ingress
remain. Coven manages one explicit `settings.json` using direct SessionStart
handlers and strict JSON (native JSONC is refused). Capture is reference-only,
always unknown, not native delivery, approval or activity proof. See
[COPILOT_HOOK_MIGRATION.md](COPILOT_HOOK_MIGRATION.md) for explicit preview/apply,
verified opt-in migration and guarded byte/mode-exact rollback.

New **Codex** hook installation/recommendation/update/bulk paths are also retired
**on Linux only**, before provider/config discovery and without auto-running Coven.
Non-Linux legacy installer/assets/result type remain until matching Coven adapters.
Read-only status, uninstall and old ingress stay compatible. See
[CODEX_HOOK_MIGRATION.md](CODEX_HOOK_MIGRATION.md) for explicit two-file
hooks.json/config.toml previews, global hook enablement consent (not native trust),
SessionStart-only reference capture and guarded two-file rollback/recovery.
Codex never reports working/idle: even fresh records are unknown. Normal Coven
uninstall leaves the shared global hook feature enabled and never revives Kit
commands; native approval remains user-controlled. A config write is not native
operational certification. The shared filesystem extraction preserves Claude
behavior/v2 receipts/errors/statuses; Codex has a distinct v1 journal.

New Claude hook installation **on Linux is Coven-owned**: Linux Kitsune refuses
`integration install claude` immediately with Coven guidance, before provider or
configuration discovery. API callers receive `integration_install_failed` with the
same Linux-qualified guidance; Claude is excluded from startup recommendations
and update/bulk candidates **on Linux only**. Non-Linux production
`legacy_install_claude`, assets/result type and recommendation/update/bulk paths
remain until matching Coven native qualification is complete. Coven's
[shared private hook-file backend](../coven/HOOK_FILE_BACKEND_CONTRACT.md) now
source-enables explicit macOS apply for all five managers, provisional eligible
local APFS only, not natively qualified. All old non-Linux installers/assets/status/
uninstall/recommend/update/bulk paths stay. Coven receiver privacy now has shared
source-enabled admission, provisional local APFS on macOS, **not native
qualification**; see [private storage and endpoint limits](../coven/docs/AGENT_PRIVATE_STORAGE_CONTRACT.md).
macOS capture remains natively unvalidated; generic discovery is provisional,
full management pending and
Windows unsupported. See
[CLAUDE_HOOK_MIGRATION.md](CLAUDE_HOOK_MIGRATION.md) for explicit
preview/apply migration, private receiver/inventory, rollback and recovery limits.

All six `IntegrationTarget::ALL` entries, Claude/Codex/Copilot/Pi/OpenCode status inspection, uninstall and old
hook ingress remain on all platforms. Legacy UI settings remain for other
providers/platforms. Claude process/screen fallback, start/resume and agent restore
are **still Kitsune**, as are Codex/Copilot/Pi/OpenCode discovery/status/start/resume/restore,
not transferred by retiring installers. Separate built-in `djinn` handling and
other applicable transfers remain pending. Fixed targets and `integration.install/uninstall` are
separate from generic `integrations.sources`, not its provider registry.

The confirmed target includes **all** discovery/status interpretation, hooks,
start/resume and agent restore in Coven on all platforms, not just Claude/Linux;
retained compatibility is transitional, not permanent ownership. The authoritative
[Coven ownership matrix](../coven/AGENT_MANAGEMENT_MIGRATION.md) is optional sibling
navigation, not an install/startup dependency. No user configurations or running
sessions are automatically migrated. Coven's optional inventory view uses generic
snapshot/`integration.view.*` only; failed presentation does not stop its capture,
and selecting a last-reported row grants no launch/adoption/resume authority.

Djinn support remains a built-in agent kind for process detection, integration
status, interactive startup through `kitsune agent start --kind djinn`, and
authoritative lifecycle reports from source `kitsune:djinn`.

Kitsune does not resolve or install Djinn versions itself. Install or update the managed Jamal Arcana tool suite with `arcana update` to keep Kitsune and Djinn on compatible builds.

## Windows note

Native Windows builds use the system ConPTY included with Windows 10 October 2018 or newer. Kitsune does not currently ship a pinned app-local ConPTY bundle.

## Notes

- Agent integrations remain available unless removed separately.
- Durable decisions live in root docs; current feature details also live under `docs/next/`.

Kitsune inherits Herdr's Apache-2.0 license.
