# portable-pty local patches

This file tracks intentional local changes applied on top of the vendored
`portable-pty` source. Remove a patch only when the upstream crate contains an
equivalent fix or exposes an option that lets Kitsune keep the same behavior.

## 0001 control ConPTY loading

status: active

patch: `vendor/patches/portable-pty/0001-control-conpty-loading.patch`

historical herdr issues:

- https://github.com/ogulcancelik/herdr/issues/761
- https://github.com/ogulcancelik/herdr/issues/1533

upstream discussion: https://github.com/microsoft/terminal/issues/17452

upstream pr: none

vendored base: `portable-pty 0.9.0`

local files:

- `vendor/portable-pty/Cargo.toml`
- `vendor/portable-pty/Cargo.toml.orig`
- `vendor/portable-pty/src/win/psuedocon.rs`

reason: `portable-pty` intentionally probes a bare `conpty.dll` through the DLL
search path. Kitsune must never load another application's DLL from `PATH`.
Kitsune does not ship a pinned app-local Microsoft ConPTY runtime. The Windows
backend uses the ConPTY exports from the already loaded `kernel32.dll`, so
Windows support depends on the system ConPTY implementation provided by Windows
10 October 2018 or newer.

remove when: upstream `portable-pty` stops probing bare `conpty.dll` through the
DLL search path, or Kitsune replaces the Windows PTY backend.

verification:

```sh
python3 -m unittest scripts.test_vendor_portable_pty
```

On Windows, run the enhanced-input CI probe against the system ConPTY path on
supported Windows versions.

## 0002 expose Windows raw command tails

status: active

patch: `vendor/patches/portable-pty/0002-windows-raw-command-tail.patch`

historical herdr issue: https://github.com/ogulcancelik/herdr/issues/1041

upstream discussion: none

upstream pr: none

vendored base: `portable-pty 0.9.0`

local files:

- `vendor/portable-pty/src/cmdbuilder.rs`

reason: Kitsune needs to launch `cmd.exe /d /c` with the user-authored command
tail parsed as shell text. `portable-pty` represents commands as argv and
ArgvQuote escapes embedded quotes, which changes how `cmd.exe` parses the raw
command string.

remove when: upstream `portable-pty` exposes Windows raw command-line tail
support or Kitsune replaces this launch path.

verification:

```sh
python3 -m unittest scripts.test_vendor_portable_pty
```

On Windows, also run `cargo test raw_arg_appends_unescaped_windows_command_tail`.
