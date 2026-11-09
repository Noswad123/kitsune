# Terminal engine and Rust backend roadmap

## Status and scope

**Status: proposed, separately scoped investigation.** No replacement engine has
been selected, implemented or approved for cutover. Ghostty remains the production
terminal engine.

This roadmap evaluates reducing the Zig build requirement and, independently,
whether a Rust terminal backend is worth adopting. It is **not a prerequisite**
for agent-policy extraction, optional Coven integration or the current process
policy refactor. Those efforts should continue with the existing engine.

Immediate recommendation: supply the matching Zig toolchain and validate today's
application. Do not turn a missing compiler into an unplanned engine rewrite.
This document records future work; it does not install tools or authorize changes
to live sessions, user configuration or the default backend.

## Current architecture

```text
Rust PTY/runtime → Rust terminal facade / Ghostty C-ABI wrapper
                                      ↓
                             Zig libghostty-vt
                                      ↓
                           terminal cells / state
                                      ↓
                             Rust rendering
```

- `build.rs` is already Rust. It invokes Zig to compile the vendored terminal
  engine and statically links the result. Translating build orchestration would
  not eliminate the Zig implementation.
- `vendor/libghostty-vt.vendor.json` pins the engine source to
  `c5a21edfcbc2d5b46540ad91b7980aca31f5f1f3`.
- The current vendored build expects Zig **0.15.2**; do not assume the latest Zig
  release is compatible. Platform-specific toolchain requirements still apply.
- The `ZIG` environment variable already accepts an explicit compiler path, so
  a global Zig installation is not required.
- `src/ghostty/` and `src/pane/terminal.rs` integrate the engine. The existing
  facade still exposes Ghostty-specific types/state; it is not a pluggable backend
  interface yet.
- Local engine patches, including grapheme behavior after reset, are part of the
  compatibility baseline, not incidental changes that a replacement may drop.

## Goals and alternatives

| Goal | Approach | Expected scope |
| --- | --- | --- |
| Build/test the current app | Download and verify the matching Zig distribution; select it through `ZIG`. | Routine prerequisite work. |
| Developers need no local Zig compiler | Optionally consume a matching prebuilt engine library. | Build/distribution and supply-chain work; engine remains Zig. |
| Remove the Zig implementation | Evaluate and adapt an existing Rust terminal engine. | Significant compatibility/backend migration. |
| Port Ghostty itself to Rust | Reimplement and maintain the engine. | Much larger project; not the default proposal. |

Do not conflate these goals. A Rust backend needs a concrete maintenance,
portability or product benefit beyond avoiding one compiler download. A
weeks-to-months migration is plausible, but no delivery estimate is committed
until a bounded capability spike is complete.

## Phases and exit gates

### 0. Unblock and establish the existing baseline

- [ ] Obtain the project-compatible Zig toolchain from a trusted distribution and
  verify its checksum; use an explicit path if a global installation is unwanted.
- [ ] Build and run narrow application tests first, recording actual prerequisites
  and failures. Request approval before the resource-heavy full `just check` gate.
- [ ] Record engine version, patches, target/toolchain configuration and current
  terminal compatibility/performance tests. Real Windows/macOS validation remains
  separate from Linux source-subset checks.

**Exit:** the existing engine can be built and assessed on each intended target.
Finishing this phase does not authorize or require a replacement.

### 1. Define the backend contract and compatibility inventory

- [ ] Inventory every Ghostty call, exposed type and state access in the Rust
  terminal facade and its consumers.
- [ ] Define backend-neutral operations and ownership/lifetime rules for byte
  input, screen state, callbacks, resizing, rendering and resource cleanup.
- [ ] Capture independent terminal byte streams, events and expected observations;
  do not generate expected behavior from the prospective replacement itself.
- [ ] Define performance/resource budgets and explicit requirements for supported
  platforms, terminal applications and outer terminals.

The capability matrix must include at least:

| Surface | Compatibility to assess |
| --- | --- |
| VT state | Primary/alternate screens, cursor/modes, control sequences, resize/reflow and scrollback. |
| Unicode/cells | Graphemes, combining characters, wide cells, styles, colors and reset behavior. |
| Input/output | Keyboard, mouse and focus encoding; terminal replies and PTY writeback. |
| Application views | Plain-text/ANSI snapshots, selection, search, history and dirty-row rendering. |
| Terminal integration | Working-directory updates, clipboard callbacks and hyperlinks. |
| Graphics | Kitty image/placement behavior and resource limits. |
| Lifecycle/safety | Malformed streams, bounded memory, cleanup, callbacks and persisted history compatibility. |

**Exit:** reviewed contract, test corpus and explicit unsupported/gap dispositions.
No silent feature removal to make a candidate appear compatible.

### 2. Evaluate Rust engines in a bounded spike

- [ ] Compare maintained Rust terminal engines against the capability matrix,
  licensing, maintenance activity, integration cost and platform support.
- [ ] Replay representative recorded streams and difficult Unicode/resize/input/
  graphics cases; measure correctness, memory and throughput.
- [ ] Identify whether missing capabilities require small adapters, upstream work
  or effectively building another engine. An ANSI parser alone is not a backend.
- [ ] Produce a recommendation: retain Ghostty, improve build packaging, proceed
  with a Rust backend, or stop pending upstream capability.

**Go/no-go gate:** explicit approval of the chosen benefit, gap treatment, cost and
maintenance plan. Rewriting Ghostty from scratch requires a separate decision.

### 3. Introduce a preserving backend boundary

- [ ] Refactor the facade behind the reviewed contract while keeping Ghostty as
  the default and preserving existing behavior.
- [ ] Remove backend-specific types from consumers only where required by that
  contract; avoid unrelated runtime/UI/state rewrites.
- [ ] Re-run characterization and application tests before adding a second backend.

**Exit:** the Ghostty adapter passes existing behavior through the new boundary.
This phase still requires Zig to build the baseline engine.

### 4. Implement and validate an experimental Rust backend

- [ ] Add an explicitly selected development backend; no automatic fallback or
  production-default change.
- [ ] Run differential replay against the independently captured expectations,
  and investigate differences rather than blindly treating either engine as right.
- [ ] Validate real application workloads, supported platforms, callbacks, graphics,
  history restoration and failure handling.
- [ ] Meet agreed performance/resource budgets and document remaining gaps.

**Exit:** reviewed acceptance matrix and an explicit cutover/rollback proposal.

### 5. Cut over only after acceptance

- [ ] Approve the backend switch, packaging changes and rollback strategy.
- [ ] Validate upgrades and retained history; do not restart or migrate active
  sessions without an explicit lifecycle plan.
- [ ] Remove Ghostty/Zig build requirements only after no supported build/runtime
  path requires them; update licenses, patches, CI and development documentation.

**Exit:** the supported product builds and operates without Zig, with documented
compatibility evidence. No such outcome is claimed today.

## Optional track: prebuilt Ghostty libraries

If build convenience is the actual goal, evaluate this before an engine migration:

- [ ] Define an explicit opt-in prebuilt-library input; no such bypass exists today.
- [ ] Pin source revision, local patches, ABI/header version, build options and
  target architecture/OS/ABI. Verify artifact integrity and provenance.
- [ ] Validate linking, callback contracts, release distribution and source-build
  fallback. Do not silently download or substitute an arbitrary system library.

This can remove Zig from a consumer's build prerequisites, but not from building
the engine artifacts. It does not satisfy an all-Rust implementation goal.

## Priority and non-goals

Keep this work independent of [the main roadmap](KITSUNE_ROADMAP.md) and the
[process-policy extraction candidate](PROCESS_POLICY_EXTRACTION.md). Scheduling
engine migration must not displace their agreed priorities without a new decision.
No new scheduler, Coven dependency, UI redesign or agent behavior is required by
this roadmap. The default near-term path remains **keep Ghostty and supply Zig**.
