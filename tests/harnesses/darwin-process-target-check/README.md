# Darwin Process Source Check

This standalone harness imports the actual `src/platform/foreground.rs` neutral
types and the actual generic decoder/collector/native adapter. It has no Kit
Cargo dependency, SDK build script, Carbon dependency, or copied record types.
`libc` is pinned to 0.2.183. Linux tests use injected sources only; real Apple
Rust targets compile the native adapter and tests without pretending Linux is
macOS. Cross-checks are type/lint evidence, not native runtime qualification.

From the Kit root (Coven and Kit are sibling checkouts):

```sh
export CARGO_TARGET_DIR="$PWD/target/darwin-process-check"
export TMPDIR="$(realpath ../coven/target/tmp)"
export PATH="$(dirname "$(rustup which --toolchain 1.96.1 cargo)"):$PATH"
cargo test --offline --locked --manifest-path tests/harnesses/darwin-process-target-check/Cargo.toml
cargo check --offline --locked --tests --target aarch64-apple-darwin --manifest-path tests/harnesses/darwin-process-target-check/Cargo.toml
cargo check --offline --locked --tests --target x86_64-apple-darwin --manifest-path tests/harnesses/darwin-process-target-check/Cargo.toml
cargo clippy --offline --locked --tests --target aarch64-apple-darwin --manifest-path tests/harnesses/darwin-process-target-check/Cargo.toml -- -D warnings
cargo clippy --offline --locked --tests --target x86_64-apple-darwin --manifest-path tests/harnesses/darwin-process-target-check/Cargo.toml -- -D warnings
```

The PATH selection also selects the matching Clippy driver rather than a
Homebrew driver with a different sysroot. Build outputs should use a uniquely owned child directory under
the specified target directory, with an exit/signal cleanup trap; do not delete
pre-existing shared outputs. Verify disk space first.

## Layout and Trust

Policy marker: `darwin_procargs2_lp64_v1`, reserved for the new generic path.
Capability/routing publication is owned outside these sources. Legacy Darwin
detector/hint/normalized argv0 and Linux v1 semantics are not changed here.

Reviewed source: Apple XNU tag `xnu-11215.81.4`, annotated tag object
`ccee93b09f6eb7f8c29517a3cc5dfd39ea743c11`, resolved commit
`d4514f0bc1d3f944c22d92e68b646ac3fb40d452`:

- [kern_exec.c](https://github.com/apple-oss-distributions/xnu/blob/d4514f0bc1d3f944c22d92e68b646ac3fb40d452/bsd/kern/kern_exec.c): `exec_save_path` saves the 16-byte `executable_path=` prefix and NUL-terminated path; `exec_extract_strings` zero-pads the original region to target `new_ptr_size`, then copies arguments. Direct exec does not replace an empty argv0; interpreter handling is distinct.
- [kern_sysctl.c](https://github.com/apple-oss-distributions/xnu/blob/d4514f0bc1d3f944c22d92e68b646ac3fb40d452/bsd/kern/kern_sysctl.c): `sysctl_procargsx` strips the prefix without repacking and prepends a four-byte argc. A successful full-capacity read can be an undersized suffix. It explicitly notes stale exec data is possible.
- [proc_info.h](https://github.com/apple-oss-distributions/xnu/blob/d4514f0bc1d3f944c22d92e68b646ac3fb40d452/bsd/sys/proc_info.h): public `PROC_FLAG_LP64 = 0x10`, `PROC_PGRP_ONLY = 2`, 16-byte BSD comm. These two constants are missing from libc 0.2.183, so the adapter documents their source.

If L includes the path NUL, LP64 reply argv starts at
`4 + round_up(L, 8)`, never `round_up(4 + L, 8)`. Only calculated, present,
zero padding is consumed. Exactly argc terminated strings are decoded,
including empty argv0 and later empty operands; trailing bytes are not parsed.
This is interpretation under a reviewed kernel layout, not authentication or
an absolute no-environment-data assertion against mutable process memory.

## Bounds and Qualification

- Only positive PID/PGID values fitting i32 cross the native boundary.
- Raw sysctl allocation: 1 MiB cap checked before reservation, six attempts,
  query/read length checks, retry with growth after a capacity-filling success.
- Arguments: 1..128, each decoded string at most 4096 bytes, aggregate 64 KiB.
- Exact exported text budget: argv bytes + raw argv0 bytes + space-joined
  cmdline bytes + name bytes + 3072-byte cwd reserve <= 64 KiB. Checked before
  allocating the duplicate fields or joining. Failure omits all three argument
  fields; the independent strict-UTF8 BSD comm remains usable.
- Group query: 16/32/64/128/256/257 slots, six attempts, checked byte counts,
  positive sorted deduplicated PIDs. The extra slot allows a strictly-short
  result to positively admit 256 entries. Overflow/exhaustion returns None,
  not an authoritative empty cohort. No host-wide userspace enumeration.
- Membership is read again for each candidate. A post-argument BSD read rejects
  visible PID/start-time/PGID/LP64/name changes. No stable process generation or
  atomic exec guarantee is published. Child e_tpgid is terminal foreground
  membership evidence only, not session/descendant proof.

Fixtures cover every path-length residue (including no padding), empty first
and later operands, exact argc stopping, malformed/truncated data, nonzero or
missing padding, raw/decoded/aggregate/combined limits, lossy expansion,
raw argv0 preservation, independent names, PID ranges, dedup/sort, group growth
and overflow, read retry/exhaustion, and visible metadata changes.

Native runtime qualification remains required on supported macOS versions and
architectures, including direct exec with empty argv0, protected processes,
concurrent exec/title mutation, permissions, and changing foreground groups.
No Darwin syscalls or native applications are executed by Linux pure tests.
