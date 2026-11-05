# Generic terminal annotations

## Overview

Kitsune supports opt-in, ephemeral plain-text annotations on existing terminals
through `pane.annotate`. Build/test/export services and coordinators use the same
generic interface; no Coven or Djinn dependency is included. Coven now implements
an explicit foreground group/role publisher in its own repository; the optional
[Coven contract/quick start](../coven/GROUPS_CONTRACT.md) is navigation, not a dependency.
This first production slice is terminal-border text, not workspace annotations,
grouping, images, progress widgets, panels, or agent-feature extraction.

Manual titles remain untouched. Text is appended only in spare cells of the
existing top pane border; it allocates no rows or terminal space. **Single-pane
layouts have no border**, even with `[ui] pane_borders = true` (the default).
Use an existing split layout to see labels. Disabling borders hides them; a long
title or narrow pane can leave no room even after a successful write.

## API / Interfaces

Enable sources manually in the config located by `kitsune config path`:

```toml
[annotations]
sources = ["build"]
```

The default is `sources = []`, enabling nothing. Select at most eight unique,
ordered IDs, each 1–64 ASCII bytes of letters, digits, `-`, `_`, or `.`. Earlier
sources have priority. A write cannot enable its own source.

`session.snapshot` optionally returns `result.snapshot.annotations`:
`{"epoch":"<opaque epoch>","sources":["build"],"valid_until_unix_ms":true}`. An absent descriptor means
unsupported (older server); a present descriptor with `sources: []` means disabled.
Discover this capability rather than assuming protocol 18 implies support. The
JSON additions do not bump the binary protocol version, which remains 18.
The literal boolean `valid_until_unix_ms: true` advertises source-deadline support;
missing/false means TTL-only support, and null/nonboolean cannot authorize it.
Check the running server's snapshot, not ping, a package version, or the local
CLI's embedded schema. Expiry-dependent publishers must decline live text without
this capability, never silently drop the deadline or downgrade to a static TTL.

`pane.annotate` accepts these parameters and rejects unknown fields. All except
`valid_until_unix_ms` are required:

| Parameter | Contract |
| --- | --- |
| `pane_id` | Exact current canonical public locator, e.g. `w1:p1`; no name/implicit target lookup. |
| `expected_terminal_id` | Opaque `terminal_id` copied from that pane's snapshot entry. |
| `annotation_epoch` | Opaque current descriptor epoch; never derive it. |
| `source` | An already selected source ID. |
| `revision` | `u64`, strictly increasing per source/terminal in this epoch; initial zero is allowed. |
| `text` | **Required** JSON string of 1–256 UTF-8 bytes, or explicit `null` to clear; omission and empty string are invalid. |
| `ttl_ms` | Required integer, inclusive 1–300000 ms, including clears. |
| `valid_until_unix_ms` | Optional integer Unix epoch milliseconds, inclusive 0–9007199254740991 (JSON safe integer). Omission preserves TTL-only behavior; explicit null is invalid. Zero is well-formed but expired. |

Success returns the normal API `ok` result. The CLI exits 0 quietly on success,
2 for invalid arguments, and 1 for API errors. It performs the usual protocol ping
then one annotation write, without target/epoch lookups or automatic retries.
An `ok` acknowledges bounded acceptance, not visibility or continuing validity
when the reply arrives. Older servers reject the new request field rather than
silently honoring it; there is no automatic TTL-only retry.

## Data Flow

```text
user selects ordered sources -> producer reads snapshot identities/epoch
  -> pinned pane.annotate -> server-owned TerminalId record
  -> highest-priority recorded source -> escape/clip into spare border cells
```

Each accepted write fully replaces that source/terminal record. Without a source
deadline, TTL starts at server acceptance, not at a producer timestamp. With a
deadline, TTL remains required and is an additional ceiling on remaining source
validity; invalid TTLs are rejected, not clamped into range. The earliest selected
source with any accepted record wins; a cleared or expired winner hides output **without
falling back** to lower-priority records. Before a higher source first records,
a lower source may display. Clear/expiry retains bounded revision and priority
watermarks, so replay cannot renew text or resurrect a lower source.

Timers trigger expiry/redraw even on quiet terminals, in both monolithic and
headless-server operation; expiry does not depend on incoming terminal output.
Producer disconnect is not detected immediately: the last accepted label can
remain until its TTL expires.

### Source deadlines and compatible clocks

Supply the original source validity horizon, not `send_time + ttl`. Re-publishing
unchanged evidence must preserve that horizon or shorten it; a higher revision is
not fresh evidence. Source interpretation and clock translation belong to the
producer, not Kitsune. A whole text frame expires together; there is no static
layer or lower-source fallback beneath a timed frame.

After API queueing and ordinary fences, the state-owning handler samples a
monotonic instant **M before Unix wall time W**. For deadline D and valid TTL T:

```text
D <= W -> reject annotation_expired
effective_expiry = M + min(T, D - W)
at insertion A: A >= effective_expiry -> reject annotation_expired
```

The absolute monotonic expiry is retained through insertion, never restarted from
a residual duration. Arithmetic is checked and the minimum is taken before
adding to Instant. Positive sub-millisecond remaining validity is not rounded up
to the minimum request TTL. Expired queued writes are rejected without installing
text or a tombstone. This applies uniformly to `text: null` when D is supplied;
intentional cleanup can omit D and retain normal clear semantics. A rejected newer
write does not revoke older accepted content, consume its revision, change priority
occupancy, or renew its deadline. It leaves that pair's content/expiry/accepted
time/revision/capacity unchanged; unrelated housekeeping may still run.

**Clock limitation:** this contract assumes compatible source/server Unix clocks,
no disruptive wall-clock change during evidence-to-acceptance, and normal
monotonic progression. Discovery advertises API support, not clock quality.
Server-behind-source skew or rollback before acceptance can overestimate validity;
an ahead clock expires early. Known uncertainty can be covered by a conservative
producer margin; unknown skew cannot be solved with these fields. After acceptance,
wall rollback cannot extend the monotonic lease, but forward jumps do not
continuously remap it. Suspend behavior is platform-dependent; paused processes,
event-loop stalls, and client/display lag do not offer hard-real-time pixel expiry.
This is **not arbitrary rollback/skew/suspend safety**. Deployments requiring that
stronger guarantee need a separately approved shared-clock/boot-domain design,
not another timestamp or receipt-relative TTL. A usable Unix clock sample is
required only for deadline-bearing writes; failure does not fall back to TTL.

## Edge Cases

- The store holds at most 64 live terminal targets (at most 512 pair records).
  Cleared/expired live targets still occupy capacity; existing targets remain
  updatable at capacity. Terminal deletion reclaims records. Moves follow
  `TerminalId`, but subsequent writes need its current pane locator. Replacement
  terminals cannot inherit a pinned write.
- Identical ordered source-list reload preserves records and epoch. Any list or
  order change, including disabling, clears all records/watermarks and creates a
   new opaque epoch. Invalid reload preserves the previous valid annotation
   configuration; invalid startup selection disables annotations. Restart creates
   a fresh epoch. Labels are neither persisted nor transferred through handoff.
- `sources` must be an array, e.g. `sources = ["build"]`, not `sources = "build"`.
  Startup isolates structurally invalid annotation settings and disables that
  section with a diagnostic, preserving otherwise valid keybindings and UI
  settings. Invalid TOML or unrelated config type errors still follow the existing
  startup fallback behavior. Older builds may discard the entire config for an
  annotation type error; correct the setting and use `kitsune server reload-config`
  to recover valid settings without restarting running panes.
- Rejected writes do not consume revisions. Equal/older revisions are rejected,
  including after clear/expiry. Increment only for genuinely new updates: do not
  automatically reissue stale content at a higher revision. A missing acknowledgment
  is ambiguous; never automatically retry it with a new revision.
- API errors include `annotation_epoch_mismatch`, `source_disabled`,
  `pane_not_found`, `terminal_mismatch`, `annotation_stale_revision`,
  `invalid_annotation_text`, `invalid_annotation_ttl`, `annotation_capacity`, and
  `annotation_time_regression`; malformed/missing/wrong-type parameters are rejected
  before the handler. Reinspect identities/configuration rather than bypassing fences.
- Deadline errors are `invalid_annotation_deadline` for a parsed integer above the
  safe bound, `annotation_expired` for passed source/mapped deadlines, and
  `annotation_clock_unavailable` for an unusable Unix sample. Null, malformed types,
  negative/fractional values, and u64 overflow remain `invalid_request` at decoding.
  Multiple simultaneous invalid fields/fences have no new error-precedence promise.
- Controls, bidi controls/marks, and Unicode line/paragraph separators are escaped
  before cell-width clipping. Markup is literal text; clipping preserves UTF-8
  scalar boundaries, not necessarily full grapheme clusters.
- Source ownership is logical, **not authentication**: any caller with access to
  the existing local API can claim an enabled source and submit higher revisions.
  Coordinate writers; source selection is not a per-producer authorization boundary.
- Display creates no agent identity/membership, seen state, notification, native
  runtime fact, success/completion evidence, or permission to send input/start/stop.

## Example usage

Use an updated CLI **and server** containing this feature. Building/testing this
checkout does not install it or upgrade an already-running Kitsune server. An
absent snapshot descriptor means that server does not advertise annotation support.

These are manual user steps, not an auto-edit/restart procedure. After editing the
config above, target the intended existing server with the usual socket/session
selection and run:

```sh
kitsune server reload-config
kitsune api snapshot
```

The snapshot command prints JSON for the currently selected server. Copy
`result.snapshot.annotations.epoch` and the chosen entry's `pane_id` and
`terminal_id` from `result.snapshot.panes`; confirm `sources` contains `build`.
No `jq` is required. Substitute those values for the placeholders below (and
replace `w1:p1` if necessary). Use revision 1 only for a fresh source/terminal pair:

```sh
kitsune pane annotate w1:p1 --expected-terminal '<terminal_id>' --annotation-epoch '<epoch>' --source build --revision 1 --text 'Build: running' --ttl-ms 5000
```

For a genuinely new clear after revision 1, keep the same identities/epoch and use
revision 2. `--clear` replaces `--text`; TTL remains required:

```sh
kitsune pane annotate w1:p1 --expected-terminal '<terminal_id>' --annotation-epoch '<epoch>' --source build --revision 2 --clear --ttl-ms 5000
```

For source-validity-sensitive text, discover literal deadline support first, then
add `--valid-until-unix-ms '<source deadline in Unix milliseconds>'` to the same
command. The CLI validates integer syntax/bounds before contacting the server and
forwards that absolute value unchanged; it performs no hidden discovery, clock
conversion, automatic retry, or fallback. Do not copy a fixed example timestamp
into a live invocation. Intentional cleanup clears may omit the deadline.

Coven's foreground group publisher and independent build producers use this same
generic method; these manual commands do not install or activate a publisher. Use
a dedicated coordinated source per workspace, not the publisher's source for manual
writes. For a combined policy, `sources = ["coven", "build"]` gives Coven priority
even if build recorded first. Once Coven claims a target, its expired/cleared record
still shadows lower sources until source-policy reset; stopping the publisher does
not restore build-label fallback. No live server or config was changed for this guide.

## Validation

Pre-deadline baseline validation (historical; not results for the expiry upgrade):

- `just test-one annotation`: **46 tests**, including the real CLI against private
  fake sockets and JSON dispatch through the actual App handler into an in-memory
  pane-border frame, followed by timer expiry and baseline-frame restoration.
- Existing/combined characterization filters: `ui::panes::` **21**, `config::`
  **132**, `persist::` **54**, `pane_move` **17**, and `session_snapshot` **2** tests.
  These filters overlap; their counts are not a distinct-test total.
- Generated API-schema currency check and all-target Clippy with warnings denied
  pass. Diff whitespace checks pass.

`just lint` remains blocked by pre-existing formatting in unchanged
`src/ui/keybind_help.rs:471`; its Clippy command was run separately. No full suite,
`just check`, cross-target validation, installation, or live-session smoke test was
performed. The fake-socket tests are explicitly enabled on macOS, unlike the
repository's older CLI integration test target.

Deadline-focused checks use `just test-one annotation`, `just test-one pane_annotate`,
`just test-one generated_protocol_schema_artifact_is_current`, and the existing
`ui::panes::`, `pane_move`, and `session_snapshot` filters. Deterministic injected
clocks cover queue/insertion delay, sub-ms validity, clamps, atomic rejections,
clear behavior, clock limitations and legacy independence from wall time. Quiet
runtime tests cover both TTL and absolute caps; private fake sockets cover exact
CLI forwarding and errors without retry. Regenerate the embedded schema after
authorized API changes with
`KITSUNE_UPDATE_API_SCHEMA=1 just test-one generated_protocol_schema_artifact_is_current`.

## Related files

- [src/annotations.rs](src/annotations.rs) — store, priority, expiry, text escaping/clipping.
- [src/app/api/annotations.rs](src/app/api/annotations.rs) — epoch/identity fences and errors.
- [src/api/schema/panes.rs](src/api/schema/panes.rs), [session.rs](src/api/schema/session.rs) — request and descriptor types.
- [src/cli/pane.rs](src/cli/pane.rs), [server.rs](src/cli/server.rs), [api.rs](src/cli/api.rs) — commands.
- [src/config/model.rs](src/config/model.rs), [io.rs](src/config/io.rs), [src/app/mod.rs](src/app/mod.rs) — configuration/startup/reload.
- [src/ui/panes.rs](src/ui/panes.rs), [src/app/runtime.rs](src/app/runtime.rs) — existing-border rendering and deadlines.
- [APP_BOUNDARIES.md](APP_BOUNDARIES.md), [ANNOTATION_MODEL_PROTOTYPE.md](ANNOTATION_MODEL_PROTOTYPE.md) — ownership and historical prototype.
