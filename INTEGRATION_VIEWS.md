# Generic integration views and selection messages

## Overview

The desktop **persistent lower side-menu section** is `Integrations` by default,
with a vertical list of user-configured applications and the locally selected
application's plain label/detail rows. Kitsune routes selection intents; each
application owns what its rows mean. For Coven, that means Coven group/participant
declarations and read-only details—not Kitsune's detected-agent inventory.

This is a functional first ownership bridge, not completed agent extraction.
Legacy detection, agent APIs, restore, attention, mobile and other agent-aware
surfaces still exist during transition. No Coven dependency or agent-specific
types are added to the generic view model.

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

The default list is empty: the block shows `No integrations configured`, never
detected-agent rows. Each configured source has its own status: `Unavailable`
without a live lease, `Waiting for data` before its first frame, `No items` for
an accepted empty frame, and `Available` when it has rows. The user selects an
application locally from the vertical list; producers do not select the UI.
Upper workspace navigation remains Kitsune-owned and is not replaced.

The selector windows within short panels and keeps the selected source visible.
When height permits, it reserves a complete content row so listing many providers
cannot make every provider's items unreachable. Mouse selection and wheel
navigation are local presentation operations, not messages broadcast to apps.

The list is ordered, unique and bounded to eight sources. Source IDs are 1–64
ASCII bytes of letters, digits, `_`, `.` or `-`. Invalid startup settings disable
providers with diagnostics without discarding unrelated config. Invalid reload
preserves the previous valid list/state. Reorder preserves unchanged sources'
leases, frames, queues, local selection and per-source scroll positions. Removal
discards only removed sources' state; if the selected source is removed, selection
reconciles to the first remaining source. Rendering can clamp scroll to available
content. No publisher can opt itself in or change user configuration.

The obsolete local prototype's `[external_view]` section is ignored with a
diagnostic directing users to `integrations.sources`; no automatic config write
or migration occurs. Both repositories use this new contract together; the
unreleased singleton prototype has no published compatibility requirement.

Kitsune/user still control sizing, collapse/split layout, scrolling and colors.
External data cannot supply scripts, pane targets, executable callbacks, widgets,
arbitrary markup or native status icons. Labels/details are escaped and clipped.
Source attribution is displayed but **is not authentication**.

### Discovery and JSON API v1

The `session.snapshot` result's `snapshot.integration_views` advertises:

```json
{"version":1,"sources":[{"id":"coven","epoch":null},{"id":"build","epoch":null}]}
```

An absent descriptor means unsupported. `sources: []` means no providers configured.
The whole configured list is returned in order, not just the locally selected app.
Each source's `epoch` is null without an active lease; discovery never reveals
lease tokens. Discover the running server capability, not protocol/package/CLI
schema version. These are
additive JSON methods; the binary protocol remains 18.

All requests use the existing local JSON API, with correlated response IDs:

| Method | Parameters | Successful `result.type` and fields |
| --- | --- | --- |
| `integration.view.open` | `source`, `version: 1` | `view_opened`: `lease`, `epoch`, `revision: 0`, `ttl_ms: 5000` |
| `integration.view.replace` | `source`, `lease`, `epoch`, `revision`, `items` | `view_replaced`: `epoch`, `revision` |
| `integration.view.poll` | `source`, `lease`, `epoch` | `view_intents`: `epoch`, `revision`, `intents`, `ttl_ms: 5000` |
| `integration.view.close` | `source`, `lease`, `epoch` | `view_closed` |

There are no aliases for the obsolete unreleased `view.*` methods. Legacy
`integration.install` / `integration.uninstall` and their fixed `IntegrationTarget`
enum are separate compatibility agent-hook installers, not a generic provider
registry. The legacy agent-hook settings surface does not configure this list.

Unknown parameter/item fields are rejected. An item is exactly
`{"id":"opaque-id","label":"Plain label","detail":"Plain detail"}`.

- One owner per configured source; opening an already leased source fails. Each
  source has an independent lease, epoch, revision, frame and intent queue. Producers
  coexist even when not selected; close/expiry affects only that provider.
- Lease/epoch are opaque correlation identities, **not security credentials**.
- A lease lasts five seconds on the server's monotonic clock. Successful replace
  or poll renews it; rejected operations and clicks do not. The initial lease has
  no visible frame until a replacement with positive revision arrives.
- Revision strictly increases. Replacement is atomic and complete, including an
  empty list; invalid frames neither replace content nor renew/consume revision.
- Per provider: at most 128 uniquely identified items. IDs: 1–128 UTF-8 bytes, nonblank; labels:
  1–160 bytes, nonblank; detail: 0–512 bytes. Controls are rejected. Total raw text
  is bounded to 96 KiB. With eight providers the aggregate bounds are 1,024 items,
  768 KiB raw text and 256 queued intents. IDs need only be unique within a frame;
  identical IDs in different providers do not share state or queues. Rendering
  additionally escapes bidi/separator characters.
- Each visible row has a label and detail line. Content ordering belongs to the
  producer; colors, geometry and clipped/hidden space belong to Kitsune/user.

Errors include `integration_view_disabled`, `integration_view_source_disabled`,
`integration_view_busy`, `integration_view_lease_invalid_or_expired`,
`integration_view_stale_revision`, `integration_view_invalid_frame`, and
`integration_view_unsupported_version`.

## Data Flow

```text
user configures sources → independent applications open leases and publish rows
  → user locally selects application → Kitsune renders its rows → user selects row
  → that source's bounded intent queue → application polls and validates
  → application publishes newer read-only details
```

Internal UI activation passes the source explicitly; it is routing context, not a
new field in the intent. An intent contains only `{epoch, revision, item_id}`.
It is **not authorization**
to execute, launch, resume, stop, send terminal input or bypass application policy.
The first Coven consumer only displays declared participant details.

Activation is fenced against current lease/revision, known item and the most
recent committed render snapshot for the foreground client. Layout changes,
scrolling and foreground-client identity changes invalidate capture until a new
successful render/send. Lower-section legacy row targets, sort controls and agent
cycling cannot activate native agents through the Integrations block.

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
other producers coexist. Select Coven locally to see/select its rows. There is no
broker, plugin framework, embedded script or mandatory Coven dependency.

See [Coven's contract](../coven/GROUP_PRESENTATION_CONTRACT.md). The sibling link is
documentation only, not a build/runtime dependency. No installed/running app was
replaced or reconfigured to validate this feature.

## Validation and remaining transition

Focused tests cover store fencing/atomicity/expiry, config isolation, persistent
rendering, source attribution, native-input isolation, client-switch/queue failure
fencing, and quiet/poll-triggered expiry. A separately invoked developer test uses
the real Coven CLI with actual Kitsune handlers and persistent rendering on a
private fixture socket and `TestBackend`. It exercises source-header selection,
row selection, updated details and close. An independent `build` provider comes
first and uses the same item ID: its frame/queue/lease remain isolated and survive
Coven's close, without native providers or user sessions.

The opt-in test requires an explicit `COVEN_TEST_BIN`; it is ignored in ordinary
test runs and does not locate a sibling automatically. Full-suite, macOS/Windows
and installed-server interoperability remain separate gates. Legacy management
removal still needs Coven-owned runtime adapters and compatibility decisions;
this generic view must not grow native agent policy in their place.

The current multi-integration implementation passes **509 focused Kitsune tests**,
strict all-target application lint, source-schema regeneration/checking and a normal
debug build. This includes eight-provider nonempty short-panel tests and raw-JSON
duplicate-key rejection. The explicit real-Coven/two-provider roundtrip also passes.
It uses actual handlers through a private direct-handler socket proxy and coordinate
helper activation, not a production server, physical mouse input or client-pixel
acknowledgement. Full `just check` has not been approved/run; Windows/macOS are not
validated for this slice.

With project toolchains selected and an explicitly built Coven binary:

```sh
COVEN_TEST_BIN=/absolute/path/to/coven cargo test --offline --locked --bin kitsune app::api::integration_views_roundtrip_tests::integration_views_real_coven_roundtrip -- --exact --ignored
```

## Related files

- [src/integration_views.rs](src/integration_views.rs) — per-source state and bounds.
- [src/api/schema/integration_views.rs](src/api/schema/integration_views.rs) — wire types.
- [src/app/api/integration_views.rs](src/app/api/integration_views.rs) — handlers.
- [src/ui/sidebar/integrations.rs](src/ui/sidebar/integrations.rs) — local selector/render fencing.
- [src/config/model.rs](src/config/model.rs), [src/config/io.rs](src/config/io.rs) — configuration/diagnostics.
- [src/app/api/integration_views_roundtrip_tests.rs](src/app/api/integration_views_roundtrip_tests.rs) — explicit cross-repository fixture.
- [APP_BOUNDARIES.md](APP_BOUNDARIES.md) — legacy migration scope and ownership.
