# Preserving process-policy seams

For the active ownership bridge, see [generic integration views](INTEGRATION_VIEWS.md):
Coven supplies management content and consumes selection intents; Kitsune renders
and routes generic data in its default desktop Integrations block. Multiple apps
coexist with independent leases/frames/revisions/queues; the local selector is not
authentication or native execution authority. No agent fallback is used in that
block. The characterization below protects legacy extraction, not a new
agent-management implementation inside Kitsune. Detection/APIs, mobile surfaces,
native hooks/installers and restore remain separate migration work; see the
[scope matrix](APP_BOUNDARIES.md#migration-scope). Prioritize Coven-owned management
model/runtime adapters and matching legacy retirement, not characterization alone.

## Scope

Local refactor candidate against `01d7cef6e98e2ccab216cd669d569c0731abfbce`.
Classification and Windows selection behavior remain the compatibility target.
This does not remove agent integrations, change protocol/schema/configuration,
add a dependency on Coven, expose a process-tree API, or claim full-platform
validation.

Pre-edit risk review classified this as detection/process-reporting refactor-risk.
Protected behavior was identified from existing detection and Windows selector
tests and an independently captured 209-case corpus before moving code. Two
read-only design reviews covered resolver tracing and Windows selection/wiring.

## Resolver seam

`detect::identify_agent_in_job` retains its signature and canonicalizing behavior.
It delegates to the crate-internal `identify_agent_in_job_with_resolver` with a
single `FnMut(&Path) -> io::Result<PathBuf>` callback. Production uses the existing
filesystem canonicalization adapter; injected callers control resolution instead.

The injected graph has no fallback to real filesystem resolution. Native
`Path::components()` eligibility and resolved UTF-8 basename interpretation are
unchanged. Wrapper/name/argv/cmdline precedence, lazy resolution, repeated leader
attempts, ranking and first-equal-score selection are preserved. There is no new
caching, normalization, probing, mode exclusion or identity guarantee.

Nine tests in `src/detect/resolver_tests.rs` cover lexical shortcuts, native
component gating, errors and unusable results, non-UTF-8 basenames on Unix, all
wrapper branches, ordered/repeated calls, stateful retry, leader short-circuiting
and priority/tie preservation. Existing real-symlink adapter coverage remains.

## Neutral selection seam

`src/platform/process_selection.rs` holds `ProcessEntry`, traversal/projection,
and descendant selection. Its classifier is supplied as
`FnMut(&ForegroundJob) -> Option<A>` with `A: PartialEq`; the implementation does
not import agent detection or access OS APIs/filesystems.

The private module compiles under `cfg(any(windows, test))`: Windows production
uses it, and ordinary Linux/macOS application unit tests can exercise the same
policy. Unix production builds do not acquire unused Windows selection policy.

The Windows adapter still supplies `detect::identify_agent_in_job`. Live snapshot
acquisition, command-line parsing, the 250 ms cache, shell availability, leader
lookup and session-process enumeration remain in `platform/windows.rs`.

Preserved details include:

- First root match; missing root returns no job before classification.
- Root excluded from descendant classification; reachable descendants visited in
  breadth-first/source order, with PIDs marked visited on enqueue.
- One candidate wins; multiple candidates need equal classifications and a
  recognized ancestor of every other candidate. Branching descendants beneath
  that ancestor remain eligible; the rule is not a stricter linear-chain check.
- Ambiguity/no candidates fall back to the root, which need not actually be a
  shell. Results remain singleton jobs with selected PID as group-like ID.
- Duplicate-PID first-reachable traversal, last-parent ancestry mapping, candidate
  order and cycle guards remain unchanged. No graph hardening was bundled in.

All ten existing Windows selector tests moved alongside the real policy, keeping
their names/assertions and the real classifier. Six new tests protect generic
callback identities, field/order preservation, empty/missing/duplicate roots,
branching and duplicate-PID behavior. Windows cache, availability and session
enumeration tests remain in the Windows module.

## Validation and limits

The optional standalone replay tool in the separate Coven checkout
(`tools/harnesses/kitsune-process-candidate`) compiles the current unchanged
classifier blocks, complete neutral module, resolver tests and actual Windows
adapter. This is development tooling, not a Kitsune build/runtime dependency.
It verifies an independently frozen baseline corpus, rather than regenerating
expectations from the new code. Reference capture pins remain unchanged.

On Linux, source-subset validation passed:

- **209/209 baseline records**, exact classifier and selected-job projections.
- **26 tests**: replay, nine resolver tests, ten relocated selector tests, six
  new selector tests.
- Subset Clippy and both source/harness formatting checks. The replay scaffold
  permits unused extracted detection helpers and its existing collapsible-if
  style; this is not a full-application lint result.
- Independent post-edit review found no blocking findings.

### Real Linux application validation

The earlier missing-Zig blocker is resolved. The installed default Zig reports
**0.17.1**, which the vendored build rejects; a separately downloaded official
**0.15.2** compiler was checksum-verified and extracted under `/tmp/opencode`.
It was selected through `ZIG`, without replacing the installed compiler.

The shell also selects Homebrew Rust **1.99.1** ahead of the project toolchain.
`rustup run ... just ...` alone does not correct that nested command lookup.
Prepending the **1.96.1** toolchain's `bin` directory selects matching Cargo,
rustc, rustfmt and Clippy for the recipes. Unrelated newer-Clippy diagnostics
were not fixed or suppressed to make this refactor pass.

With those explicit toolchains, the real application builds and passes:

- **145 focused application tests**: 44 resolver/classifier/selector tests, 44
  pane-probing/detection tests, 47 state/snapshot/restore tests, eight API/action
  ownership-release tests and two no-runtime deferred-start guard tests.
- **`just lint`**, including strict all-target application Clippy and formatting.
- Independently, all 209 replay cases and 26 source-subset tests still pass;
  reference pins and fixture bytes are unchanged.

Reproduce the recipe environment from this checkout, substituting an explicit
compatible compiler path. The toolchain lookup honors `rust-toolchain.toml`:

```sh
PATH="$(dirname "$(rustup which rustc)"):$PATH" ZIG=/absolute/path/to/zig-0.15.2/zig just test-one resolver_tests
PATH="$(dirname "$(rustup which rustc)"):$PATH" ZIG=/absolute/path/to/zig-0.15.2/zig just test-one identify_agent_in_job
PATH="$(dirname "$(rustup which rustc)"):$PATH" ZIG=/absolute/path/to/zig-0.15.2/zig just test-one platform::process_selection
PATH="$(dirname "$(rustup which rustc)"):$PATH" ZIG=/absolute/path/to/zig-0.15.2/zig just test-one ownership_tests
PATH="$(dirname "$(rustup which rustc)"):$PATH" ZIG=/absolute/path/to/zig-0.15.2/zig just test-one app::agent_resume::guard_tests
PATH="$(dirname "$(rustup which rustc)"):$PATH" ZIG=/absolute/path/to/zig-0.15.2/zig just lint
```

The resource-heavy full `just check` was **not run**; it requires explicit approval
under the validation-hygiene policy. Only the Linux target is installed. Also run
the Windows application build/Clippy and retained cache, availability,
session enumeration and resolver/selector tests in a real supported Windows
toolchain environment. Linux results cannot certify Windows acquisition/imports,
native linking, macOS, or installed-provider behavior. No build-script bypass,
substitute engine library, global compiler replacement, app restart, live provider,
hook installation or live API was used.

## Handler-boundary characterization

Six tests in `src/app/api/ownership_tests.rs` call actual pane handlers,
event/action dispatch and `TerminalState` methods. They distinguish successful API
responses from accepted identity mutations, and assert dirty state, authority,
persisted reference, detected state, pending plan and application invariants:

| Test | Preserved behavior |
| --- | --- |
| `api_restored_session_release_checks_reference_before_consuming_sequence` | Stale scoped release does not consume the sequence; matching release at that same sequence clears the reference. |
| `api_invalid_report_reference_is_ignored_but_invalid_release_is_unscoped` | Invalid/missing Codex references are ignored by both report handlers without consuming sequence; release with the same invalid/missing selector follows the existing unscoped path. |
| `api_rejected_identity_update_consumes_sequence_before_foreground_confirmation` | A rejected foreign-owner update consumes its source sequence; later foreground confirmation still requires a fresh sequence. |
| `api_live_hook_release_checks_reference_and_sequence_before_clearing_authority` | With authority installed through the report handler, stale reference and stale/equal/unsequenced release attempts preserve authority; matching fresh release clears authority but preserves detected Djinn. |
| `api_live_hook_release_rejects_foreign_source_or_agent_before_consuming_sequence` | Unofficial pairs cannot bypass the installed-authority owner guard. The same-source/wrong-agent row proves legitimate sequence preservation; the foreign-source row alone cannot, because sequence ledgers are source-scoped. |
| `api_unsupported_owner_path_is_discarded_before_takeover_sequence` | An unsupported Claude path is discarded at handler conversion; a valid ID at the same sequence can still take over a differently owned persisted reference with matching foreground evidence. |

Clearing/replacing the persisted reference leaves the existing pending resume plan
unchanged **at immediate handler return**. These tests do not establish what a
later resume scheduler will do. The invalid-selector release behavior is recorded,
not hardened or endorsed as an external-integration contract.

"Live hook" means logical authority in state, not a live provider/process. The
fixture proves authority was installed through the real report handler before
exercising release. A separate private-state test,
`foreground_owner_confirmation_rejects_unsupported_reference_kind`, supplies a
typed unsupported path directly and a supported-ID positive control. It isolates
the planner-support predicate without pretending the real API can produce that
unsupported typed path.

The shared test-only `app::test_support::app_from_state` constructor avoids
duplicating the inert `App` literal across API and deferred-guard fixtures.
Fixtures bypass `App::new`, application config loading and terminal runtimes;
they do not launch providers or PTYs. Existing workspace test helpers can read
cwd/Git metadata, so this is not a zero-filesystem-IO proof. These are direct
handler tests, not wire/async-queue, detector or outbound-event tests. Actual
process liveness and graceful runtime release remain outside the logical-authority
coverage.

## Startup, metadata restore and deferred guards

This test-only follow-up adds eight tests in total: three handler tests, one
private owner-predicate test, two restore tests and two deferred-start guard tests.
It changes no production policy, executor, protocol, configuration or dependency.

- `pane_restore_startup_validates_before_reserving_and_suppresses_duplicate_identity`
  calls the real startup/metadata functions. Disabled/invalid combinations retain
  history and do not reserve; valid sessions reserve and suppress history;
  duplicates suppress both history and rehydrated identity. Valid metadata is
  retained when automatic resume is disabled. Equal ID text for different official
  providers is independent.
- `native_agent_restore_preserves_metadata_across_workspaces_and_tabs_without_runtimes`
  calls real `restore` with eight **unique** resumable sessions across two
  workspaces and four split tabs. It checks every terminal, exact pending plans
  and identities, managed/manual labels, gapped public IDs, layout/root/focus and
  invariants. All terminals survive and no runtimes are registered. This covers
  the deferred metadata branch, not provider startup or terminal history replay.
- `pending_agent_resume_deadline_tracks_geometry_without_extending_wait` tests
  the real candidate/deadline methods: zero-area exclusion, a single installed
  deadline, no extension on repeated sync, exact before/at/after due boundaries,
  geometry-loss reset and no-pending reset. It uses synthetic times, not sleeps.
- `pending_agent_resume_theme_and_empty_argv_guards_preserve_intent` checks bulk
  and terminal-specific starts. Missing theme blocks valid argv; defensive empty
  argv still blocks when empty theme is allowed. Identity, intent, saved launch
  fields and revision remain unchanged; no runtime or save is scheduled. Save
  scheduling is enabled in the fixture so that assertion is not vacuous, but no
  background save executor runs.

Restore/guard fixtures own temporary directories through exclusive creation and
RAII cleanup. Explicit nonexistent child shell paths prevent a bypassed guard
from selecting a user's shell/provider; they do not by themselves prove no PTY
attempt under a regression. Passing state/plan/count assertions and production
branch inspection establish the intended no-runtime paths. Workspace/Git metadata
reads remain possible, including when `TMPDIR` is inside a repository.

Independent review identified a registry-API compile error and a globally shared
temporary-path assumption in the new tests. Both were fixed; the reviewed final
slice passes focused application tests and strict lint.

### Still-open runtime gates

- Global duplicate precedence across workspace/tab/layout traversal. The new
  multi-workspace test deliberately uses unique references; the startup test's
  shared set does not prove real session traversal order.
- Imported-runtime precedence, strict import accounting and reservation rollback.
- Deferred shell-spawn failure and command-enqueue failure, including identity,
  pending-plan, cleanup and persistence-scheduling outcomes.
- Actual graceful runtime release, wire/async dispatch and other-platform behavior.

These need controlled runtime fixtures or a preserving production injection seam.
There is no current injectable import/spawn/send boundary. Do not simulate rollback
by manually editing the reservation set, use arbitrary raw descriptors, or rely
on a shell-exit race to manufacture an enqueue failure.

## Next boundary

After those gates, evaluate an additive generic process-tree evidence interface
with parent relationships, acquisition scope and selection/completeness semantics.
The current Windows `pane.process_info` remains agent-selected and cannot serve
as a complete generic tree. Keep report/native-reference ownership, sequencing,
release, restore/history, deferred launch and hook effects as separate gates.
No built-in removal or optional external delegation is authorized by this refactor.
