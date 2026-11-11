# Pi extension ownership migration to Coven

New Pi extension installation on Linux is retired in Kitsune. Coven owns the
explicit replacement installation and verified legacy migration. Kitsune never
invokes Coven automatically.

## Kitsune boundaries

- Linux `integration install pi` refuses before path discovery, version checks,
  config reads/writes or install logging. API `integration.install` returns
  `integration_install_failed` with the same guidance.
- Linux recommendations, bulk-install candidates and outdated-update candidates
  exclude Pi. Explicit update instructions point to Coven.
- Non-Linux retains the legacy installer and its returned extension path,
  recommendations and update behavior. `legacy_install_pi` and the full embedded
  v7 asset compile under `cfg(any(test, not(target_os = "linux")))`. This is
  preservation, not non-Linux validation; the old lifecycle asset is not hollowed out.
- All six `IntegrationTarget::ALL` entries, read-only legacy status, explicit
  uninstall and old hook ingress remain. Status still describes the legacy file,
  not Coven activation.

`kitsune integration uninstall pi` removes only `kitsune-agent-state.ts`, never
`coven-agent-reference.ts`. It is legacy cleanup, not Coven uninstall or rollback.
Do not uninstall first when Coven needs the legacy preimage for migration.

## Coven handoff

Reference syntax only, using inspected absolute paths:

```text
coven agent extensions plan --provider pi --extension-root ABS_EXTENSION_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune
coven agent extensions install --provider pi --extension-root ABS_EXTENSION_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune --apply
```

The explicit absolute extension root must already exist. Coven does not create
it, discover application roots, execute native apps or run discovery probes.
Planning is read-only; mutation requires explicit `--apply` and is Linux-only.
Migration requires `--migrate-kitsune` and the exact Kitsune v7 asset SHA-256:

```text
cce258cccccd2fdf66549cc89f1eee72b3293f558a7da96b8b87fe9071b3f247
```

A marker or matching filename alone does not authorize removal. Custom or
modified legacy assets remain user-owned. The replacement name is
`coven-agent-reference.ts`, separate from legacy `kitsune-agent-state.ts`.
Coven's bounded two-leaf journal installs the new leaf first, then deletes the
verified legacy leaf. Rollback reverses that order and restores original bytes
and exact modes, including rollback after Coven uninstall. Retain the receipt
and recovery artifacts; this is not a global filesystem compare-and-swap.

## Functional reduction

The replacement captures references only at `session_start` using
`getSessionId`. Migration **drops legacy working/blocked/idle reporting**; it is
not lifecycle parity. Its `hasUI === true` eligibility includes TUI and RPC and
does not prove the selected root is loaded. Installed files and passing offline
tests do not prove native activation, operational capture, approval behavior or
full discovery/start/resume/restore support.

This Kitsune change does not edit live application configuration, start Pi,
install Coven or certify native operation. Coven implementation and broader
architecture documentation are separate delegated work.
