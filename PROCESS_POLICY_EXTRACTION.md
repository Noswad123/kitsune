# Preserving process-policy seams

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

- **136 focused application tests**: 44 resolver/classifier/selector tests, 44
  pane-probing/detection tests, 43 state/snapshot/restore-planning tests, and five
  API/action ownership-release tests (including three new handler-boundary tests).
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

Three new tests in `src/app/api/ownership_tests.rs` call actual pane handlers,
event/action dispatch and `TerminalState` methods. They distinguish successful API
responses from accepted identity mutations, and assert dirty state, authority,
persisted reference, detected state, pending plan and application invariants:

| Test | Preserved behavior |
| --- | --- |
| `api_restored_session_release_checks_reference_before_consuming_sequence` | Stale scoped release does not consume the sequence; matching release at that same sequence clears the reference. |
| `api_invalid_report_reference_is_ignored_but_invalid_release_is_unscoped` | Invalid/missing Codex references are ignored by both report handlers without consuming sequence; release with the same invalid/missing selector follows the existing unscoped path. |
| `api_rejected_identity_update_consumes_sequence_before_foreground_confirmation` | A rejected foreign-owner update consumes its source sequence; later foreground confirmation still requires a fresh sequence. |

Clearing/replacing the persisted reference leaves the existing pending resume plan
unchanged **at immediate handler return**. These tests do not establish what a
later resume scheduler will do. The invalid-selector release behavior is recorded,
not hardened or endorsed as an external-integration contract.

Fixtures bypass `App::new`, application config loading and terminal runtimes;
they do not launch providers or PTYs. Existing workspace test helpers can read
cwd/Git metadata, so this is not a zero-filesystem-IO proof. These are direct
handler tests, not wire/async-queue, detector or outbound-event tests. Live
hook-authority release, unsupported-reference owner confirmation, imported runtime
restore and deferred launch/failure remain separate coverage gaps.

## Next boundary

After those gates, evaluate an additive generic process-tree evidence interface
with parent relationships, acquisition scope and selection/completeness semantics.
The current Windows `pane.process_info` remains agent-selected and cannot serve
as a complete generic tree. Keep report/native-reference ownership, sequencing,
release, restore/history, deferred launch and hook effects as separate gates.
No built-in removal or optional external delegation is authorized by this refactor.
