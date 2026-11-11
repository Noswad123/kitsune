# OpenCode Plugin Ownership Migration

New OpenCode plugin installation on Linux is retired in Kitsune. Coven owns
explicit replacement installation and verified legacy migration. Kitsune never
invokes Coven or restarts a native provider automatically.

## Kitsune Boundaries

- Linux `integration install opencode` refuses before path discovery, version
  checks, config access or install logging. API `integration.install` returns
  `integration_install_failed` with the same guidance.
- Linux recommendations, bulk-install and outdated-update candidates exclude
  OpenCode. With all Linux external installers retired, those lists may be empty.
  Explicit update instructions point to Coven.
- Non-Linux retains `legacy_install_opencode`, its `OpenCodeInstallPaths` return
  type, plugin path, recommendations and update behavior. The full embedded v9
  asset and installer compile under `cfg(any(test, not(target_os = "linux")))`.
  This preserves behavior; it does not validate non-Linux operation.
- All six `IntegrationTarget::ALL` entries, read-only legacy status, explicit
  uninstall, old ingress and discovery/status/start/resume/restore remain.
  Legacy file status is not Coven activation evidence.

`kitsune integration uninstall opencode` removes only the legacy
`kitsune-agent-state.js` in Kitsune's existing `plugins` location, never
`coven-agent-reference.js`. It is not Coven uninstall or rollback. Do not
uninstall first if migration needs the legacy preimage.

## Explicit Handoff

Reference syntax only; substitute inspected absolute paths:

```text
coven agent extensions plan --provider opencode --extension-root ABS_EXTENSION_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune
coven agent extensions install --provider opencode --extension-root ABS_EXTENSION_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune --apply
```

The explicit absolute extension root must already exist. There is no HOME-based
automatic selection, directory creation, JSON configuration or package change.
Planning is read-only; mutation requires explicit manual `--apply` on Linux.
Migration requires `--migrate-kitsune` and the exact Kitsune v9 SHA-256:

```text
7782f028d172b7b56b95aa364f443b8929446a40ce19a505e55aa8d8e405312b
```

Markers and filenames alone do not authorize removal. Modified legacy files are
user-owned. Replacement is confined to the single selected existing root; it
does not deduplicate plugins across roots. The pinned native source accepts
both global `plugin` and `plugins` directories, including Kitsune's existing
`~/.config/opencode/plugins` location. Inspect other roots yourself.

Coven uses the same private fixed-pair two-leaf journal engine as Pi. It
installs `coven-agent-reference.js` before removing the verified legacy leaf.
The original preimage is immutable across reinstall, including after uninstall.
Rollback restores original bytes and exact modes, including rollback after
Coven uninstall. Retain receipts and recovery artifacts; this is not a global
filesystem compare-and-swap.

## Native Contract

Supported source pin: `anomalyco/opencode` **v1.19.5**, commit
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`. This supports function plugins only.
The changed development API at `611cee9` is **UNSUPPORTED**. Source inspection
and offline tests are not certification of an installed OpenCode binary.

The plugin runs server-side for an exact directory instance. That instance can
see multiple session roots, not just the selected chat or Kitsune pane. Capture
uses `session.created` and `session.updated` events with full `info.id`; it does
not enumerate sessions through the SDK and is not universal resume capture.

Migration **drops legacy working/blocked/idle lifecycle reporting**. The new
plugin captures references only; fresh lifecycle state remains unknown. This is
a functional reduction, not lifecycle parity or complete restore certification.

**Stop native OpenCode servers and relevant watchers before mutation.** After
reviewing and explicitly applying the chosen plan, restart the native server.
Attaching or reopening only a TUI is insufficient for an already-running server.
There is no automatic restart. Installed files and passing tests do not prove
native activation, operational capture or full discovery/start/resume/restore.

This Kitsune change does not edit live configs, launch native providers, install
Coven, or contact the user's application server.
