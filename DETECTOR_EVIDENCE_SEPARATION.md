# Detector evidence separation

## Status And Scope

Implemented in current uncommitted Kitsune source: an internal typed state seam
separates foreground-owner evidence, fallback presentation and restored historical
identity. This is not detector retirement or external mode. No new Config,
wire/API or snapshot-format fields are introduced by this seam. Earlier generic
`snapshot.process_observation` metadata and the five Linux new-install retirements
(Claude, Codex, Copilot, Pi and OpenCode) are preserved separate work. Non-Linux
adapters and broader management extraction remain pending.

Default legacy selection/status remains except deliberate hardening: historical
restore and screen-only observations cannot mint foreground ownership, hook
takeover or effective native authority. This narrows observable API readiness and
authority semantics, not merely a behavior-neutral refactor, despite frozen schemas.

## Evidence Model

`src/terminal/evidence.rs` defines `DetectorEpoch` with a nonserialized value and
creation `Instant`, typed `ForegroundObservation` and separate `FallbackObservation`.
`src/terminal/state/evidence.rs` applies them. Private fallback/historical identity
getters are distinct from foreground evidence. Genuine positive probes retain
actual `observed_at`, optional PGID context and epoch, not native identity or a
native generation. Kind selection is still legacy policy, including environment
hints and the Windows agent-aware selector, not neutral native-process proof.

The existing six-miss/foreground-shell absence policy and hook probe schedule
remain. No new TTL, continuous liveness or native identity authentication is added.
Screen samples, skipped probe ticks and legacy internal `StateChanged` fallback
events cannot create or refresh positive proof.

The App gates events against the runtime currently bound to the pane's Terminal,
synchronizing before owner-sensitive events to invalidate prior-runtime proof.
Admission starts at token birth, not event delivery. Normal and handoff constructors
and probe events use the actual runtime token; queued old-runtime events cannot
adopt a replacement epoch.

## Producer And Lifecycle

`src/pane/detector_producer.rs` and shared pane helpers publish every actual positive
probe, even unchanged findings: deduplication could lose admission after a consumer
generation barrier. Screen-only updates never substitute for probes. `ClearAgent`
publishes explicit `Cleared` without duplicate Idle completion.

`Reset` invalidates proof, not native exit/session retirement. It retains probe
presence context so later shell/six-miss absence can still retire the owner.
Pending release suppression excludes the released kind from reset presence context
to avoid duplicate completion. A separate authoritative-wake control avoids the
Reset/positive-activation loop.

Reset and epoch synchronization suspend native authority to Unknown while preserving
session S, reference and sequence barriers. Admissible same-S reports are buffered
but ineffective. Recognized replacement-start intent is also promoted only on
fresh appropriate proof, without resend. Old-S stale and release barriers remain.

An admitted policy clear or conflicting foreground finding retires a full-lifecycle
native owner even if an unverified hook report has a newer timestamp. Actual newer
foreground evidence still blocks an older clear. The custom-authority timestamp
exception remains. Private pending-replacement `observed_at` preserves a recognized
start received after the old clear sample for the proper generation, without resend.

Unchanged native Idle projection reconfirmation carries a private flag to prevent
Done/unseen state and Finished SFX. Reconfirmation requires unchanged source,
reference and state **and no accepted native state changes during suspension**.
Idle -> Reset -> Working -> Idle -> confirmation is real completion once, even
though its final state matches the initial Idle; repeated Idle reports stay quiet.
Stale, foreign and rejected reports do not dirty the suspension window. Legitimate
Unknown-to-Idle transitions elsewhere remain unchanged.

Release measures pre-release proof to preserve eligible fallback display, then
invalidates eligibility. Explicit acceptance is a boolean independent of visible
`PaneStateUpdate`: accepted Idle-to-Idle release starts runtime suppression without
fake API events; rejection plus epoch change does not. Real `PaneDied` still clears
proof and publishes legitimate Idle completion.

## Restore And Input

Both actual `src/persist/restore.rs` branches seed historical identity, never a
fabricated process observation/`Instant`. Historical Idle, name, reference, pending
resume and managed Active are preserved. `interactive_ready` stays false until a
matching foreground probe **or accepted fallback**: readiness is not foreground-only,
and fallback never claims native proof.

Runtime-import existence and resume-command submission are not proof. No current
runtime actor proof persists; snapshot schema and capture bytes remain frozen.
Native prompt/send-keys input still performs its independent fresh process check;
retained evidence is not a substitute for that input guard.

## Validation Record

Verified scoped results reported by the implementation agents/parent, not rerun by
this documentation-only update. The following module filters are disjoint.

| Scope | Reported Record |
| --- | --- |
| Terminal state | 142 checks passed, including latest private suspension dirty flag |
| Pane | 108 checks passed, including 13 producer checks |
| Persistence | 58 checks passed |
| App resume guards | 3 checks passed |
| App actions | 95 checks passed |
| `api::` | 264 checks passed, including App API 182 and 15 detector checks |
| Headless | 130 checks passed |
| UI | 171 checks passed |
| Workspace pure unit | `workspace::tests` 12 plus aggregate 6 passed |
| App input | 329 checks passed |
| Distinct Kit total | **1,318 checks passed** across the disjoint filters above |
| Coven cross-compatibility | **50 checks passed**: discovery 15, presentation 14, legacy observer 21; separate from Kit total |
| Kit strict `just lint` | Passed across all targets, including final confirmation |
| Final confirmation | State 142, API 264, headless 130 and strict all-target lint reruns passed after the final suspension-history fix |

Do not add App API 182, detector subsets (including earlier 13), or producer 13
again. Coven has no new production code in this cut: changes from root `9b36f6c`
are docs-only. Earlier discovery Coven 201/Kit 249 results are historical and are
not added to the seam count.

## Validation Limits

The broader `just test-one workspace::` also selected CLI cases whose harness
hardcodes `/tmp`. Seven failed with ENOSPC because shared temporary storage was
full, before semantic validation; fail-fast left the remainder unrun. Refined pure
workspace unit filters passed 18, already included above. This does not establish
that those CLI cases passed. `workspace::tab::tests` selected zero tests and is not
a pass claim. Full `just check` was not run.

The desktop frame golden initially failed. An owned detached temporary worktree at
unchanged Kit HEAD `fabc9f265e3a8faec86b17a0102e4dd649768902` reproduced exactly the
same actual digest `101b79bcc66557ea1049cd77cc79db9746be6ff236360e32e085e40e9c1159c4`
against the old expected `22dc5c...`, proving a preexisting stale golden. Only the
test was corrected, adding neutral `No integrations configur...`/not-Agents
assertions. Geometry, hyperlinks, cursor and mobile golden coverage were preserved.
The clean temporary worktree was removed without force, staging or commits;
ignored scratch target/build output remains.

The earlier headless title fixture's stale native-Agent expectation was corrected
to the existing neutral Integrations contract; API title/no-event-flood assertions
remain. These fixture corrections introduce no production UI behavior changes.
Production behavior changes are confined to intended evidence hardening. No
live-provider, cross-platform or full readiness certification is claimed.

See the optional sibling [parent migration record](../coven/AGENT_MANAGEMENT_MIGRATION.md#state-seam-validation-table).

## Related Files

- `src/pane.rs`, `src/terminal/{runtime,state}.rs`, `src/events.rs`
- `src/app/{api,actions}.rs`, `src/server/headless.rs`
- `src/persist/{restore,snapshot}.rs`
- [App boundaries](APP_BOUNDARIES.md), [roadmap](KITSUNE_ROADMAP.md)
- Optional sibling [Coven discovery contract](../coven/AGENT_DISCOVERY_CONTRACT.md); reader and accepted capabilities remain unchanged.
