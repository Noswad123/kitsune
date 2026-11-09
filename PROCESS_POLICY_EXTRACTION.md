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

**Full application validation is blocked, not passed.** Before and after edits,
`CARGO_NET_OFFLINE=true just test-one identify_agent_in_job` / focused resolver
and selector filters fail in `build.rs` because Zig is absent. Likewise,
`CARGO_NET_OFFLINE=true just lint` passes formatting but cannot reach application
Clippy past the vendored libghostty build. Only the Linux Rust target is installed;
the Windows module as a whole has not been compiled or executed here. No build
script bypass, substitute library, toolchain installation, app restart, live
provider or live API was used.

Remaining gates, with prerequisites supplied:

```sh
just test-one identify_agent_in_job
just test-one resolver_tests
just test-one platform::process_selection
just check
```

Also run the Windows application build/Clippy and retained cache, availability,
session enumeration and resolver/selector tests in a real supported Windows
toolchain environment. Linux source-subset success cannot certify OS acquisition,
platform imports, native linking or installed-provider behavior.

## Next boundary

After those gates, evaluate an additive generic process-tree evidence interface
with parent relationships, acquisition scope and selection/completeness semantics.
The current Windows `pane.process_info` remains agent-selected and cannot serve
as a complete generic tree. Keep report/native-reference ownership, sequencing,
release, restore/history, deferred launch and hook effects as separate gates.
No built-in removal or optional external delegation is authorized by this refactor.
