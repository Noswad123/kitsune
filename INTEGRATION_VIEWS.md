# Generic integration views and selection messages

## Overview

The desktop sidebar contains ordered peer sections: **Spaces** first, then every
configured integration in configuration order. All sections coexist; there is no
`Integrations` umbrella or provider selector. Kitsune routes item selection
intents; each application owns what its plain label/detail rows mean. For Coven,
that means Coven group/participant declarations or normal-workflow observations
and read-only details, not Kitsune's
detected-agent inventory.

This generic bridge does not revive retired agent surfaces. Live detection,
agent APIs, automatic provider restore and old navigator/mobile agent cues are
retired globally; cleanup/history compatibility and privacy limits remain as
documented in [global retirement](AGENT_CONTROL_RETIREMENT.md). No Coven dependency
or agent-specific types are added to the generic view model.

| Responsibility | Owner |
| --- | --- |
| Participants, roles, grouping, meaning of details and selections | External application, e.g. Coven |
| Multiplexer operations, menu geometry, clipping, palette and input routing | Kitsune/user |
| Native runtime lifecycle and permissions | Runtime and explicitly authorized coordinator adapters |

## API / Interfaces

### Manual configuration

In a build supporting this contract, manually configure sources in the config
reported by `kitsune config path`, preserving other settings:

```toml
[integrations]
sources = ["coven", "build"]
```

The default source list is empty: only Spaces is present, with no lower
`No integrations configured` placeholder and no detected-agent fallback. The exact
logical ID `coven` has the friendly title **Coven**. Other IDs use sanitized literal
titles, not title-cased names or a configurable label registry. Collisions retain
source attribution: `spaces`/`Spaces` gain `[source:ID]`, and the friendly Coven
title becomes `Coven [coven]` when literal `Coven` is also configured.

Every section has the same disclosure header and collapses independently. Only an
expanded integration body shows `Unavailable` without a live lease, `Waiting for
data` before its first frame, or `No items` for an accepted empty frame. Healthy
bodies show one-line summaries directly, without an `Available` preamble. Optional
generic status glyphs are host-rendered, not native-provider icons.

The list is ordered, unique and bounded to eight sources. Source IDs are 1–64
ASCII bytes of letters, digits, `_`, `.` or `-`. Invalid startup settings disable
providers with diagnostics without discarding unrelated config. Invalid reload
preserves the previous valid list/state. Reorder preserves unchanged sources'
leases, frames, queues, collapse state and per-source scroll positions by ID.
Removal prunes only removed sources' state; re-adding a removed source starts it
expanded. A removed section focus falls back to Spaces. Foreground layout can
clamp scroll to available content. No publisher can opt itself in or change user
configuration.

The obsolete local prototype's `[external_view]` section is ignored with a
diagnostic directing users to `integrations.sources`; no automatic config write
or migration occurs. Both repositories use this new contract together; the
unreleased singleton prototype has no published compatibility requirement.

### Desktop layout and local preferences

Kitsune/user control sizing, collapse, scrolling and colors. The global menu row
and bottom global sidebar toggle are separate from section headers. Spaces' `new`
header action creates a workspace rather than toggling disclosure. Workspace
grouping, focus, reorder, context menus, numbered shortcuts and width dragging
remain generic multiplexer operations.

After reserving the menu, bottom toggle and one header row per visible section,
the allocator divides the remaining body budget equally among expanded sections.
Remainder rows go in stable section order, so expanded body heights differ by at
most one. A collapsed section costs only its header and takes no body share. If
all sections are collapsed, the remaining budget is blank. The old vertical
two-section divider drag is retired; sidebar width dragging remains.

When the height cannot fit all headers and their minimum bodies, a contiguous
window begins at a client-owned section offset. Wheel input on a header advances
the window/focus through section order, making all sections reachable when there
is at least one section row. At that minimum height a header alone is shown.
Wheel input in an integration body scrolls only that source, not the section
window or another source. Spaces-body wheel input scrolls overflowing workspace
content; when it fits, the existing wheel-based workspace selection is preserved.
The global keyboard sidebar toggle remains available; keyboard item inspection
uses the separate generic local inspector described below.

Global compact mode is four columns wide (three content cells), independent of
per-section collapse; hidden mode is zero columns. At three content cells, expanded
headers include `▾Sp`, `▾Co` and `▾bu` for Spaces, exact `coven` and `build`.
`Sp` and `Co` are reserved case-insensitively; conflicting two-character source
prefixes use `#` plus the source's configuration ordinal. At two content cells,
headers use a disclosure marker plus `S`, `C` or a source ordinal; at one cell,
only that identity remains. Headers do not all degrade to identical ellipses.
The compact menu `m` remains clickable. Compact workspace numbers follow the full
visual workspace order, including noncontiguous worktree grouping and scrolling,
matching numbered shortcuts rather than raw storage indices or viewport positions.

Session snapshots optionally persist `collapsed_sidebar_sections`, defaulting to
empty. Typed Spaces and integration IDs remain distinct, even for source `spaces`.
Both ordinary decoding and raw legacy migration use lossy parsing bounded to the
first nine entries: malformed preferences do not discard terminal/session history.
Legacy `sidebar_section_split` is ignored during decode and is not written as an
active preference; no ratio-to-collapse conversion occurs. Per-source scroll,
section focus/window offset and transient render/input captures are not persisted.

External data cannot supply scripts, pane targets, executable callbacks, widgets,
arbitrary markup or native status icons. Labels/details are escaped and clipped.
Source attribution is displayed but **is not authentication**.

### Local integration inspector

`prefix+a`, via the existing customizable `agent_selector` keybinding, opens
**Integration details**, not the retired agent navigator. Source and item lists,
search, and wrapped scrollable details inspect the same shared server source facts.
Focus, query, selection and scroll are client-owned presentation, not source facts
or persisted authority. Tab/Shift+Tab cycles Sources, Items and Details focus.
Up/Down or `k`/`j` navigates or scrolls; PageUp/PageDown moves by a page step.
`/` focuses search. Esc first blurs search, then closes. Source/item clicks only
inspect and queue no intent. Enter in Items explicitly sends the existing
`{epoch, revision, item_id}` publisher selection intent; Enter in Details activates
nothing. There is no native focus, acknowledgement, input, start/resume/cancel,
private-ID projection, authority bits, publisher styles or injected program.

The selected human name appears once in Items, not again in the detail body.
When `information` is present, Details shows **Attention**, **Current activity**
and **Working context**, followed by **Diagnostics**, collapsed by default.
Missing/empty arrays show honest "No ... supplied" placeholders, not Idle or an
all-clear claim; `information: {}` is valid. Required `detail` becomes diagnostic
text. Without `information`, including plain v3 items and old v1/v2 publishers,
`detail` stays visible in generic Details. Kitsune does not parse Coven prose.

With Details focused and search inactive, `d` toggles Diagnostics; a footer hint
advertises it. Clicking its visible header also toggles it, with scroll/clipping
and successful-frame hit-map fences. Search includes all information and diagnostic
text. A hidden-only match shows "Match in Diagnostics — expand to view." without
auto-expansion or activation. Expansion resets on source/epoch/item change,
disappearance or close/reopen, but survives ordinary revisions, including sampled
age updates. Background rendering cannot alter foreground state or captures.

`[ui] integration_animations` defaults to `true`; `false` disables optional local
working spinners. Animation phase is not a source fact, revision or activation
identity and never refreshes evidence. Source-fact age remains independent.
Visible sidebar spinners continue beside an inspector showing a neutral source.
Headless animation scheduling stops without an attached full App renderer and
resumes on reattachment, retaining saved presentation state. Pane-only clients
are not an audience for integration UI animation.

### Coven defaults and optional explicit startup runtime

Enabling the recognized `coven` source now opts into its default attached runtime
and external agent ownership in **server/client mode only** (`--no-session`
refuses runtime/external ownership):

```toml
[integrations]
sources = ["coven"]
```

At startup, before constructing PTYs, Kitsune finds the first executable `coven`
on absolute PATH entries and invokes literal `doctor status --json`. This is a
read-only compatibility handshake, not a live health check: require
`coven.doctor-status.v1`, integration-runtime v1 and native-event-reporting v1,
not a particular application version. The subprocess has a three-second total
deadline, 16,384-byte stdout limit, closed stdin, discarded stderr and no inherited
`KITSUNE_*` variables. Failed negotiation does not try later PATH candidates.

The returned canonical safe native executable and absolute default root become
frozen literal runtime/reporter argv. This resolves Arcana/public dispatchers to
the selected dev/Brew payload without weakening Coven's reporter identity checks
or hardcoding package paths. Later PATH/Arcana selection changes do not redirect
an active server; restart to select again. A native pathname is not an immutable
build-generation pin. Discovery never creates state or installs provider hooks.
See the [doctor contract](../coven/docs/DOCTOR_STATUS_CONTRACT.md).

Unknown source IDs remain presentation-only. **Compatibility change:** older
source-only Coven configuration was also presentation-only. Manual publishers can
retain that behavior explicitly:

```toml
[integrations.coven]
auto_start = false
```

This disables the implicit runtime and inferred ownership, not an explicit runtime
override. Adding/removing implicit Coven or changing this opt-out is startup-only;
section disclosure still only changes presentation. Negotiation failure retains
requested external ownership and logs `Coven unavailable` with restart guidance;
it never restores legacy detection or automatic resume.

The advanced `[integrations.runtime]` overrides the preset **as a whole**, including
the opt-out; invalid explicit settings never fall back to discovery. Fields are `source`, `command`,
`report_command` and `external_agent_management` (default false); unknown runtime
fields refuse. The source must be explicitly enabled. Commands are literal argv,
not shell strings: absolute executable, 1–64 nonempty arguments, at most 4096 bytes
per argument/16384 aggregate, no NUL. No PATH, `~` or environment expansion.

For advanced Coven executable/root pinning, the retained
[managed setup workflow](../coven/docs/NORMAL_AGENT_WORKFLOW.md#example-usage) is:

```sh
coven integration plan
coven integration enable --apply
```

Review the plan, restart Kit, open new panes and reload providers; Djinn needs an
updated native full-TUI build. Coven setup preserves unrelated config and sources,
selects the current Coven executable and private state, and generates literal
runtime/reporter argv. Kit supplies its actual API address; Coven bootstraps the
per-address workspace/receiver privately. No user `init`, socket pinning or
repeated state-root flags are required. Provider hooks/extensions still require
explicit setup with `--from-kitsune`; there is no automatic install or native
policy/trust override. See [setup/undo and manual handoff](../coven/docs/INTEGRATION_SETUP_CONTRACT.md)
and [versioned provider routes](../coven/docs/PROVIDER_ENVIRONMENT_ROUTES.md).
Existing explicit runtime blocks and setup receipts are never automatically
migrated to source-only configuration. Source-only startup needs no setup receipt.

Raw TOML and fixed `agent run`/`--socket` commands remain advanced interfaces.
Only the manual route requires a preinitialized workspace/private socket parent
and matching receiver pins. Preserve config/preimages and stop affected providers
and server before a deliberate ownership handoff. Setup refuses an unmanaged
runtime, including an earlier manual snippet; it cannot adopt or force-overwrite it.

The server starts the helper after API readiness, holds its piped stdin open,
and on shutdown closes stdin, waits 500 ms, then kills/reaps if needed. There is
no automatic helper restart or legacy fallback. New panes receive
`KITSUNE_INTEGRATION_SOURCE`, JSON literal argv in
`KITSUNE_INTEGRATION_REPORT_COMMAND`, and, in external mode,
`KITSUNE_AGENT_MANAGEMENT=external`. Disabled/nested routes strip inherited values.
Runtime/ownership changes are startup-only: restart the server, create new panes
and reload/restart native providers that already loaded plugins/hooks. Reload
retains the startup route and diagnoses restart-required changes.

External mode suppresses **all legacy agent-status authority, detector tasks and
automatic agent resume**, including imported/deferred restore, even on helper
failure. History/references are not fresh evidence or permission to execute.
Requested external ownership survives an invalid reporting route; undecodable
runtime ownership settings, malformed startup TOML or unreadable startup config
also retain external ownership fail-closed, with reporting disabled rather than
legacy restore; missing config uses defaults. Terminals remain usable. Helper
degradation expires its view lease; deliberate repair/restart is required. Rollback
is an explicit ownership/configuration handoff followed by server restart/new
panes/provider reload, never a live fallback. Generated Coven setup isolates exact
API addresses automatically in either startup mode; fixed socket collisions still refuse, not adopt.
Unreleased crash/partial state requires inspection/manual recovery, not automatic
cleanup or restart healing. The Coven contract documents the 64-instance cap,
private filesystem admission and socket-path length bounds. `integration disable
--apply` restores only the exact managed preimage and refuses later user edits;
it does not stop a live helper or undo provider configuration.
An exact restored preimage may itself enable automatic source-only Coven; disable
warns rather than silently altering that preimage. Complete the explicit ownership
handoff and opt out if necessary.

Expand **Coven** if collapsed to inspect its attributed plain rows. These do not populate
old agent-status widgets, pane focus/sidebar agent models or mobile completions.
Coven's normal facts expire within 30 seconds independently of the five-second
view lease: repaint/metadata do not refresh them, sequence gaps invalidate retained
facts, and cancellations clear only matching pending requests. Retention/tombstones
are bounded, not universal replay protection. Sessions remain unbound/unattested.
Djinn needs an updated native full-TUI build; Codex/Copilot/Pi/OpenCode reference
capture stays unknown even when configured. See the Coven guide for exact semantics,
Claude last-report bounds and remaining native macOS qualification.

### Discovery and JSON item profiles

The `session.snapshot` result's `snapshot.integration_views` advertises:

```json
{"version":1,"supported_versions":[1,2,3],"sources":[{"id":"coven","epoch":null},{"id":"build","epoch":null}]}
```

An absent descriptor means unsupported. `sources: []` means no providers configured.
The whole configured list is returned in order, independent of section visibility.
Each source's `epoch` is null without an active lease; discovery never reveals
lease tokens. Discover the running server capability, not protocol/package/CLI
schema version. These are
additive JSON methods; the binary protocol remains 18.
The descriptor stays version 1. `supported_versions` is additive; absence means
v1 only. The version selected at open is pinned for that lease.

All requests use the existing local JSON API, with correlated response IDs:

| Method | Parameters | Successful `result.type` and fields |
| --- | --- | --- |
| `integration.view.open` | `source`, `version: 1` or explicitly advertised `2` or `3` | `view_opened`: `lease`, `epoch`, `revision: 0`, `ttl_ms: 5000` |
| `integration.view.replace` | `source`, `lease`, `epoch`, `revision`, `items` | `view_replaced`: `epoch`, `revision` |
| `integration.view.poll` | `source`, `lease`, `epoch` | `view_intents`: `epoch`, `revision`, `intents`, `ttl_ms: 5000` |
| `integration.view.close` | `source`, `lease`, `epoch` | `view_closed` |

There are no aliases for the obsolete unreleased `view.*` methods. Legacy
`integration.install` / `integration.uninstall` and their fixed `IntegrationTarget`
enum are separate compatibility agent-hook installers, not a generic provider
registry. The legacy agent-hook settings surface does not configure this list.

Unknown and duplicate parameter/item/presentation fields are rejected. A v1 item
is exactly `{"id":"opaque-id","label":"Plain label","detail":"Plain detail"}`.
V2 retains these required fields and optionally adds
`"presentation":{"label":"","status":"running"}`. Presentation label may be
empty and is at most 48 UTF-8 bytes, with controls rejected. An empty caption renders
only the status icon/animation plus `item.label`, never an opaque-ID fallback.
Nonempty generic third-party captions still render; status words are not globally
stripped. Optional status is one of `pending`,
`running`, `attention`, `error`, `success`, `unknown`; omitted status is neutral,
not a seventh wire enum. Explicit presentation/status null is invalid. V1 rejects
presentation without replacing the frame or renewing the lease. Plain old
publishers work unchanged; v2 also accepts items without presentation.

V3 retains the required three fields and optional v2 presentation, and optionally
adds the strict, non-null object `information`:

```json
{"id":"opaque-id","label":"Human name","detail":"Diagnostic text","information":{"attention":["Needs review"],"activity":["Last reported working."],"context":["Project: example"]}}
```

Only `attention`, `activity` and `context` are allowed, each a string array defaulting
to `[]` when omitted. Each array has at most four entries; each entry is nonblank,
at most 256 UTF-8 bytes and rejects controls/bidi characters. Information text totals
at most 1,536 raw bytes per item and counts toward the existing 96 KiB source quota.
Unknown/duplicate fields, null objects/arrays/entries and wrong types reject.
V1/v2 reject `information` atomically, without replacing the frame, consuming its
revision, renewing the lease or partially changing the intent queue. V3 must be
explicitly opened; advertising it alone does not upgrade a lease. `detail` remains
required and bounded to 512 bytes on all profiles, including structured v3.

Coven normal `item.label` is human-name-only: provider default (e.g. Djinn),
preferring a valid associated explicit manual pane name. V2 sends blank
`presentation.label`; sidebar status is icon/animation only, no words/hashes.
Normal v1 stays name-only, exactly three fields `{id,label,detail}`, no metadata.
Stable opaque row IDs are internal selection identity, not native session IDs or
display labels; duplicate labels are allowed without ID/suffix disambiguation.
Fresh Working maps to `running`, Blocked to `attention`; Idle and Ended omit status
and intentionally share the neutral icon. Their detailed state text remains distinct.
Without structured information, the `prefix+a` inspector shows `Status: running`
or `Status: neutral` plus flat details, not an opaque sidebar label. Structured v3
instead gives the source-owned concise sections above, with metadata in Diagnostics.
Expired/history/unordered native
facts are Unknown. Host `success`/`error` are available for explicit source
assertions, not normal Coven job proof. Never interpret idle as success.

Normal naming uses publisher-only private hints, never public raw targets. It
requires a fresh valid association, exact endpoint binding and live terminal runtime
generation. Explicit generic `pane.manual_label` may already exist or be renamed
now; OSC title, CWD and historical `agent_name` are not manual identity fallbacks.
Lookup is a read bounded to 300 ms without blocking the receiver mutex, association
creation, focus, input or acknowledgement. Confirmed association/runtime/endpoint
loss uses the provider default; temporary incoherence defers publication instead;
name refresh renews no activity or evidence authority.

Normal v3 context uses `Kit project` and `Kit branch`, not native Djinn CWD proof.
Generic JSON `NewWorkspaceInfo.git_branch` is optional, cached current provenance,
also available in the same snapshot/event contexts; absent/unknown is honest, never
derived from a label, path or native metadata. Projection performs no Git,
filesystem or process I/O. Project is the associated worktree's `repo_name`, else
the workspace human label. Name/project/branch form one qualified bundle under the
same 300 ms lookup budget: endpoint/generation, unique pane and workspace/tab,
association epoch, admission and original deadline are checked after I/O and at
transport pre-send. Coherently established loss clears the whole bundle to provider name
and no context, without hiding an otherwise current row. Qualification remains
publisher-private, outside public observations/native payloads, and renews no clock.

All candidates, even empty/partial frames, coherently bracket current receiver
projection, lookup and actual pre-send. Temporary projection-lock contention,
changed/odd admission or unavailable lookup evidence defers rather than publishing
fake absence or a transient fallback that loses name-filtered selection. Last-shown
publication is bounded by the minimum original membership/context deadline and
five-second view TTL; deferrals cannot poll-renew borrowed context indefinitely.
Real monotonic membership expiry still removes rows. Safely Unsent pre-send
refresh-needed candidates can rebuild on the same owner/candidate revision within
that ceiling; attempted/uncertain writes never retry.

Read-only Coven observers (legacy inventory/control observations, `attention.list`,
association inspection and `focus.prepare`) do not invalidate admission or renew
native evidence/association clocks. Strict authority/closed-envelope classification
precedes marking; reports, attachments, acknowledgements and association mutations
remain conservatively fenced. The current gate record qualifies observer-storm
continuity across profiles 1/2/3, not new native-provider operation. Actual cached
branch acquisition is unit-qualified only; composition accepts branch or unavailable.

Coven owns Attention/Current activity wording. Fresh reported pending counts are
not an exhaustive native inventory: "No pending request in recent reports" is
not a global all-clear. Activity is reported Working/Idle/Blocked, not inferred
tool commands or summaries. Missing evidence says "No recent activity evidence"
or "No activity report received yet"; reference-only activity is unavailable.
Evidence age is the selected fact's age, not last receipt time: the owner samples
coarsely once/second, not a continuous TUI clock. It can freeze if publication
stalls until the view lease expires. Task summaries/history, tool commands, cost
and native prompt scope are not supplied.

Djinn's quiet-root fix keeps qualification pure and separately samples actual
local `Session.Service.get`/`SessionStatus.Service.get` after valid selection.
Explicit busy/retry/idle readings are native-owned state, including the closed
service's canonical idle default, not process liveness or sparse SDK absence.
Sampling is at most once/second within 1,900 ms; selection/unmount/deletion,
reporter replacement/ambiguity and new native callbacks cancel or fence stale work.
Attachment heartbeat alone never renews activity. A successful fresh native query
renews only activity via the existing event contract, not pending occurrences,
acknowledgements or outcomes. Failures leave activity unchanged and Unknown after
30 seconds. Quiet mounted normal full-TUI roots can recover, not all live Unknown
causes; unmounted/event-only/remote/unavailable profiles have no automatic heartbeat.
Native status setters commit and publish in per-instance/per-session semaphore
order, while `get`/`list` remain unlocked for callback reads. Immutable query-start
age survives query/queue/IPC within one total 1,900 ms budget, not reset at completion
or replayed as fresh evidence. Other provider profiles remain unchanged.

Coven's normal presenter validates the complete descriptor, sources and advertised
versions before open/write, chooses the highest explicitly advertised 3, else 2,
else 1 (absent list means only 1). Malformed advertisement fails before open; uncertain mutation never triggers
downgrade/retry. Plain publishers, including group present, remain v1. These JSON
profiles change no binary wire structs or protocol 18.

- One owner per configured source; opening an already leased source fails. Each
  source has an independent lease, epoch, revision, frame and intent queue. Producers
  coexist even when collapsed or outside the visible window; close/expiry affects
  only that provider. Disclosure neither queues an intent nor closes a lease.
- Lease/epoch are opaque correlation identities, **not security credentials**.
- A lease lasts five seconds on the server's monotonic clock. Successful replace
  or poll renews it; rejected operations and clicks do not. The initial lease has
  no visible frame until a replacement with positive revision arrives.
- Revision strictly increases. Replacement is atomic and complete, including an
  empty list; invalid frames neither replace content nor renew/consume revision.
- Per provider: at most 128 uniquely identified items. IDs: 1–128 UTF-8 bytes, nonblank; labels:
  1–160 bytes, nonblank; detail: 0–512 bytes. Controls are rejected. Total raw text
  is bounded to 96 KiB, including presentation-label and information bytes. The
  128-row count ceiling does not promise all maximum-size rows fit one frame.
  With eight providers the aggregate bounds are 1,024 items,
  768 KiB raw text and 256 queued intents. IDs need only be unique within a frame;
  identical IDs in different providers do not share state or queues. Rendering
  additionally escapes bidi/separator characters.
- Bodies show one-line summaries; wrapped details belong in the local inspector.
  Content ordering belongs to the producer; colors, geometry and clipped/hidden
  space belong to Kitsune/user.

Coven's bounded producer profile preserves the manual human name and primary
attention/activity under pressure. It first replaces diagnostics with an omission
marker, then compacts primary lines and drops context/evidence-age secondary text,
with an explicit diagnostic explanation. V1/v2 retain the legacy provider fallback
budget behavior. Coverage includes 128 escape-heavy rows with 160-byte labels and
full 512-byte diagnostics; there is no promise that arbitrary accepted 3,072-byte
line combinations are lossless in a complete frame.

Errors include `integration_view_disabled`, `integration_view_source_disabled`,
`integration_view_busy`, `integration_view_lease_invalid_or_expired`,
`integration_view_stale_revision`, `integration_view_invalid_frame`, and
`integration_view_unsupported_version`.

## Data Flow

```text
user configures sources → independent applications open leases and publish rows
  → Kitsune renders peer sections → user selects an item in an expanded body
  → that source's bounded intent queue → application polls and validates
  → application publishes newer read-only details
```

Internal UI activation passes the source explicitly; it is routing context, not a
new field in the intent. An intent contains only `{epoch, revision, item_id}`.
It is **not authorization**
to execute, launch, resume, stop, send terminal input or bypass application policy.
The first Coven consumer only displays declared participant details.

Explicit sidebar item activation routes the exact source/epoch/revision/item ID, without
implicit native focus, terminal input or agent meaning. Activation is fenced
against the current lease/revision, known item and most recent committed render
snapshot for the foreground client. Multi-section geometry, collapse, scroll,
section window/focus, compact state and client changes invalidate stale capture
until a successful render/frame queue. Failed enqueue does not commit new hits;
clipped or absent rows have no invisible targets in compact, hidden or mobile
layouts. Smaller/background or mobile-client draws do not adopt clamps to the
foreground client's scroll/focus/collapse or replace its captures.
Inspector activation also fences source/epoch/revision/item identity, query,
focused region, selection, scroll and layout against successful foreground capture.
Animation phase is excluded; lease/fact-age checks are independent. Key repeat,
identical captures and occlusion must not bypass these fences; the qualified
checks below cover these review fixes.

Mobile integration adoption is out of scope: the generic mobile layout remains
unchanged. The additive JSON profile changes no binary wire structs or protocol;
the binary protocol remains 18. It adds no native agent controls.

This is a successful local/server draw or frame-queue fence, not physical client
pixel acknowledgement. Thin clients can lag after successful queueing. It must
not be treated as an execution permission or atomic confirmation of user intent.

## Edge Cases

Queue capacity is 32 per provider; overflow drops the newest click. Poll drains it
**at most once**. A lost reply can lose selections; there is no durable receipt, ack, retry,
EventHub replay, offline mailbox or exactly-once guarantee. Replacing a frame
discards that provider's queued intents for the old revision. Close, expiry and
source removal discard only its owner/frame/queue; restart discards all. Reordering
does not reset providers. A fresh lease cannot recover old clicks.

Application failure is detected through lease expiry, not immediate disconnect.
Quiet timers expire/repaint without terminal activity. Event-loop/display delays
are not hard-real-time pixel expiry. Slow transport can expire a lease despite
individual calls staying inside their own timeouts; callers must stop on rejection,
not reopen/replay automatically.

The new `integration.view.*` handlers do not issue native operations or create a default
workspace for the request. Ordinary server housekeeping remains separate; snapshot
reads retain their existing documented side effects.

## Example usage

With a compatible running server, an existing Coven group, and the source manually
configured, the optional sibling command is:

```text
coven group present WORKSPACE GROUP --socket ABS_SOCKET --source coven --duration 60 --json
```

Coven validates the complete descriptor and finds the requested source anywhere
in the list, not just first. `--source coven` opens only its own lease; `build` and
other producers coexist. Expand Coven if needed and select its rows. There is no
broker, plugin framework, embedded script or mandatory Coven dependency.

See [Coven's contract](../coven/docs/GROUP_PRESENTATION_CONTRACT.md). The sibling link is
documentation only, not a build/runtime dependency. No installed/running app was
replaced or reconfigured to validate this feature.

## Validation and remaining transition

Source tests cover peer rendering, fair allocation, compact identity/collisions,
short-height reachability, independent collapse/scroll, snapshot tolerance, source
reconfiguration and successful-frame fencing. The separately invoked real-Coven
fixture uses actual Kitsune handlers, a private fixture socket and `TestBackend`;
it exercises participant selection, updated details, close and independent-provider
isolation. It requires explicit `COVEN_TEST_BIN`, is ignored in ordinary runs and
does not locate a sibling automatically.

For current actual-server UI scenarios and source-qualified evidence, consult
[Coven's gate record](../coven/tools/normal-workflow-check/README.md).
The current structured-information increment is recorded separately there; the
following counts qualify earlier source, not the current v3 changes.
Final observed v3/continuity qualification passed with independent review READY,
no P1/P2; the central record separates the earlier full Kit suite from the latest
Coven-only production-fix rerun. When explicitly approved, rebuild/install Kit and
Coven, then restart Kit to pin the new Coven generation. This increment requires
no Djinn rebuild and performs no installed activation.
**Historical name/icon/quiet-root baseline: QUALIFIED.** Final Kit passed 2,578 binary
tests (five ignored), 193 integration tests across 11 targets and one explicit
real-Coven roundtrip, including duplicate/icon/name behavior. The later compiled
Kit hash matched the last unified run. Separately, that latest unified run passed
19 sidebar, 32 normal, four adoption, 16 hosting/status-sampling gates, six Coven
presentation integration tests, 127 filtered agent-management units, 26 Djinn tests
in three files and one actual-source interop fixture. After the type-binding-only
fix, installed full-package tsgo exited 0 with zero errors and broader Djinn
validation passed 84 tests in nine files. Independent review found no P1/P2 after
the semaphore race fix. These are separate checks, not a unified rerun after the
binding fix. Prior 16/32/4/12 and 2,575+193 results remain historical baselines.
All owned children stopped/reaped; cached runs were empty, no retained artifacts.
No full CI/full Coven suite or new provider/mobile/remote/model qualification.
The existing two user Djinn instances were not live-inspected; neither current-row
resolution nor activation is claimed. That historical baseline needed new
Djinn/Coven/Kit builds and explicit
user approval are still needed before installation/restart/new-pane/provider
handoff, not silent installation or config edits. Fixture/helper activation and successful client
frame queueing are not physical native-TTY mouse validation or client-pixel
acknowledgement. Native Windows/macOS operation and installed-server interoperability
remain separate qualifications. This docs-only update runs no build or UI gate and
does not activate an installed UI.

With project toolchains selected and an explicitly built Coven binary:

```sh
COVEN_TEST_BIN=/absolute/path/to/coven cargo test --offline --locked --bin kitsune app::api::integration_views_roundtrip_tests::integration_views_real_coven_roundtrip -- --exact --ignored
```

## Related files

- [src/integration_views.rs](src/integration_views.rs) — per-source state and bounds.
- [src/api/schema/integration_views.rs](src/api/schema/integration_views.rs) — wire types.
- [src/app/api/integration_views.rs](src/app/api/integration_views.rs) — handlers.
- [src/ui/sidebar/sections.rs](src/ui/sidebar/sections.rs): typed identities, compact labels and allocation.
- [src/ui/sidebar/integrations.rs](src/ui/sidebar/integrations.rs): per-source rendering and activation fencing.
- [src/persist/snapshot.rs](src/persist/snapshot.rs): optional collapse preferences and tolerant migration.
- [src/config/model.rs](src/config/model.rs), [src/config/io.rs](src/config/io.rs) — configuration/diagnostics.
- [src/app/api/integration_views_roundtrip_tests.rs](src/app/api/integration_views_roundtrip_tests.rs) — explicit cross-repository fixture.
- [APP_BOUNDARIES.md](APP_BOUNDARIES.md) — legacy migration scope and ownership.
