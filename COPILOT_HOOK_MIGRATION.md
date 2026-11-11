# Copilot hook ownership migration to Coven

New Copilot hook installation on Linux is retired in Kitsune, matching the
Claude/Codex cut. Use Coven's explicit hook manager for new configuration and
verified legacy migration. Kitsune never invokes Coven automatically.

## Kitsune boundaries

- Linux `integration install copilot` refuses before path/config discovery,
  provider version checks, writes or install logging. API `integration.install`
  returns `integration_install_failed` with the same Coven guidance.
- Linux recommendations, bulk-install candidates and outdated-update candidates
  exclude Copilot. Explicit update instructions point to Coven instead.
- Non-Linux production retains the legacy installer, embedded shell/PowerShell
  assets, result type, recommendations and update behavior. This is preservation,
  not new non-Linux validation.
- `legacy_install_copilot`, its embedded asset and `CopilotInstallPaths` compile
  under `cfg(any(test, not(target_os = "linux")))`, like Claude/Codex. Linux tests
  retain historical fixtures and the existing legacy hook characterization.
- All six `IntegrationTarget::ALL` entries, read-only legacy status, explicit
  uninstall, legacy hook ingress and other installers remain intact.

`kitsune integration uninstall copilot` is legacy Kitsune cleanup only. It is
not Coven uninstall or rollback. Do not clean up first if Coven needs the exact
legacy preimage to verify migration. Custom handlers remain user-owned.

## Coven handoff

Reference syntax only, with inspected absolute paths:

```text
coven agent hooks plan --provider copilot --config-root ABS_CONFIG_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune
coven agent hooks install --provider copilot --config-root ABS_CONFIG_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune
```

The new Coven manager owns explicit `settings.json` direct `hooks.SessionStart`
handlers, not Claude/Codex-style nested hook groups. The scope is reference-only
startup capture, not working/idle lifecycle, native approval, provider execution
or full discovery/start/resume/restore migration. Installed configuration alone
does not prove operational capture. Migration is explicit via
`--migrate-kitsune`; inspect the plan and Coven's apply requirements before mutation.

This Kitsune cut does not edit live settings, start native apps, install Coven,
or claim native operational certification. Broader architecture documentation
and Coven manager implementation are separate work.
