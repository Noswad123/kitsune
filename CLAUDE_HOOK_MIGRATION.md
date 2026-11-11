# Claude hook ownership migration to Coven

## Overview

**New Claude hook installation on Linux is retired in Kitsune and owned by Coven.** This
is the first functional cut of the confirmed transfer of **all** agent management
to Coven: discovery/status interpretation, hooks, start/resume and agent restore.
It is not complete Claude extraction. Kitsune retains generic multiplexing and
optional agent-neutral presentation/messaging; native providers retain native facts,
credentials, policy and execution.

The authoritative surface/provider matrix and next-transfer queue live in Coven's
`AGENT_MANAGEMENT_MIGRATION.md` (optional
[sibling link](../coven/AGENT_MANAGEMENT_MIGRATION.md)). Separate repositories and
installed apps have no core Cargo dependency on each other. Non-Linux Kit installation,
recommendations and updates remain until matching Coven adapters are ready. The
ultimate target is all agent features on all platforms, not permanent compatibility.

## API / Interfaces

| Kitsune surface | Current behavior |
| --- | --- |
| Linux `kitsune integration install claude` / `install_target(Claude)` | Immediately refuses with Coven plan/install guidance explicitly saying “on Linux”, before provider PATH/version/config discovery or integration-action logging. |
| Linux API `integration.install` with target `claude` | `integration_install_failed`, with Linux-qualified Coven guidance; no successful install result. |
| Linux startup recommendations and outdated/bulk update candidates | Claude excluded; explicit update instructions direct users to Coven. |
| Non-Linux installation/recommendations/updates | macOS/Windows production legacy installer, assets/result type and recommendation/update/bulk behavior preserved until matching Coven adapters are ready. No working path is dropped. |
| `IntegrationTarget::ALL` | All six entries retained on every platform; no target removal or generic provider registry is implied. |
| Legacy installed-integration status | Retained read-only on all platforms, including Linux. |
| `integration uninstall claude` | Retained on all platforms; cleanup of Kit hooks, not Coven uninstall or rollback. |
| Old Claude hook ingress | Retained on all platforms for legacy sessions; no automatic release of live pane authority. |
| Claude detection, process/screen fallback, start/resume and agent restore | Still Kitsune compatibility; pending transfer, not removed by this cut. |

The old installer body is retained as `legacy_install_claude`, with its embedded
assets and `ClaudeInstallPaths` result type gated by
`cfg(any(test, not(target_os = "linux")))`: historical fixtures on Linux, **production
compatibility on non-Linux**. Linux-only guidance/error branches use
`cfg(target_os = "linux")`. Legacy UI settings remain because other providers and
platforms still use them. Other legacy installer targets (Pi, OpenCode, Codex, Copilot and built-in
Djinn handling) remain pending applicable transfers. Fixed `IntegrationTarget`
and `integration.install/uninstall` are **not** the generic view provider registry.

Coven's new interfaces, with explicit inspected absolute paths:

```text
coven agent serve WORKSPACE --socket ABS_COVEN_SOCKET --duration 60 [--kitsune-socket ABS_KITSUNE_SOCKET --source coven] [--json]
coven agent report --provider claude --socket ABS_COVEN_SOCKET [--event EVENT] [--json]
coven agent inventory --socket ABS_COVEN_SOCKET --json
coven agent hooks plan|install|uninstall|rollback --provider claude --config-root ABS_CONFIG_ROOT [--executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET] [--migrate-kitsune] [--apply] [--json]
```

Report reads bounded native JSON on stdin. The receiver is Unix-only; hook mutation
is Linux-only, with other Unix platforms supporting preview but refusing apply
before effects. Preview/refusal is not an equivalent non-Linux installer. Windows
receiver transport is unsupported; macOS receiver operation is not validated.
No Windows apply or all-platform certification is claimed. Core capture requires neither Kitsune nor
a running native provider and starts no provider or daemon.

## Data Flow

1. The user selects an existing config root, a trusted absolute Coven executable
   and a private Coven socket. There are no implicit HOME roots, provider probes,
   CLI-provider lookup or automatic user-config migration in Coven's new path.
2. Preview plan/install. For known legacy references, explicitly consent with
   `--migrate-kitsune`. Coven requires exact owned command strings and the pinned
   current `hooks/kitsune-agent-state.sh` bytes (SHA-256
   `00b90511a1eb781776952232309a8db3f6ed20a54ece679339cf655b9f6a6ff9`).
   Ambiguous, modified or unknown historical references refuse migration.
3. After stopping native providers/settings editors, explicitly apply on Linux.
   Coven preserves custom settings/hooks and records exact original bytes/mode or
   absence in its private v2 receipt. Generated settings and receipt are 0600.
   The old script remains untouched; running sessions are not converted and live
   Kitsune in-memory authority is not released.
4. Six new hooks invoke **Coven `agent report` only**, never Kitsune or a native
   provider: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`,
   `SessionEnd`. Shell redirection and `|| true` make offline reporting silent,
   best-effort and non-fatal to native operation.
5. Coven drops prompt/transcript/cwd/unknown fields before wire/store/stdout,
   ignores subagents, and stores full IDs privately. Inventory exposes only record
   hashes, provider, last event/state/age and explicit unverified/no-authority fields.
   Optional generic presentation waits for initialized receiver readiness and
   uses only snapshot discovery plus `integration.view.*`. This optional generic
   adapter gives Kit no new agent-native-effects ownership or core Cargo dependency.

Last-reported working/idle/ended/unknown follows local receipt order, not native
ordering. It proves no current job, readiness, identity or completion and grants
no resume permission. After restart, persisted records are unknown with null age
until another receipt. Optional UI failure does not stop capture; row selection is
read-only, with no pane binding, adoption, launch, roles or task authority.

## Edge Cases

- All Coven hook operations without `--apply` preview with no effects; `plan`
  ignores `--apply`. Existing roots must be owned and not writable by others,
  traversed with component no-follow descriptor checks. Settings input is bounded
  to 1 MiB typed JSON. Do not use install commands as live validation shortcuts.
- Repeated matching install is unchanged. Normal Coven uninstall removes Coven
  references and **never restores Kitsune references**. Explicit rollback restores
  exact original bytes/mode/absence only if the managed postimage is unmodified;
  chmod-only edits conflict too. A rollback can therefore explicitly restore old
  Kit commands, unlike normal uninstall.
- There is no strict CAS against noncooperating writers or filesystem-wide
  transaction. Linux atomic exchange/no-replace retains displaced entries. Detected
  races/crashes leave a 0700 `.coven-claude-hooks-recovery/` with fixed
  `expected`, `replacement`, `displaced`, `transaction.json` artifacts as applicable,
  and `claude_hooks_recovery_required`. All operations refuse until manual recovery.
  Stop writers, preserve evidence and inspect; never blindly undo over newer edits.
- A pending receipt without a recovery directory can be inspected with plan and
  resumed only with the same operation plus apply. A migrated pending install at
  the source reverifies the pinned old asset; an already migrated target does not
  require the old asset. Backups/preimages can contain secrets: do not share them.
- Coven requires exact 0700 socket parent and 0600 socket, never unlinks stale
  sockets on bind, and fences endpoint inodes. SIGINT cleanup is guarded before
  bind, including startup errors; graceful `serve` Ctrl-C exits 0 with `type: stopped`.
  SIGTERM/SIGKILL cleanup is not promised: manually remove a stale socket only after
  verifying no live owner. A persistent workspace receiver lease admits one
  receiver and is separate from the ordinary workspace lock between reports.
- A delivery failure after send can mean the receipt was committed. Do not retry
  automatically or treat an uncertain acknowledgement as proof of no effects.

### Validation scope

Parent-reported scoped unit/workflow validation passes. Coven's
`tests/claude_agent_migration_workflow.rs` executes generated commands through the
real Coven CLI receiver/inventory, verifies silent offline hooks and exact fixture
rollback (one test). The optional adapter's reported three-test fake-socket run passed; earlier
real generic bridge work is separate, not fresh installed-app certification here.
The source fixture retains Apache-2.0 license text. Parent reports the two security
fixes done with passing targeted tests: stash `fchmod` normalizes 0700 under an SGID
2700 parent, and generic storage protects the receiver lease basename, including
an active-lease API-replacement test. Final aggregate counts belong to the parent
after MSRV validation. This docs-only
pass ran no commands, live providers or user configs. Full `just check` remains
unapproved; no cross-platform result or automatic install/migration is claimed.

## Example usage

Linux-first reference syntax only. Use an existing workspace and existing owned 0700 socket
parent. Start the foreground receiver separately, then preview the selected root:

```text
coven agent serve WORKSPACE --socket ABS_COVEN_SOCKET --duration 60 --json
coven agent hooks plan --provider claude --config-root ABS_CONFIG_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune --json
coven agent hooks install --provider claude --config-root ABS_CONFIG_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune --json
coven agent inventory --socket ABS_COVEN_SOCKET --json
```

Both hook commands are previews. Review the plan and Coven's recovery contract
before adding `--apply` on Linux. Omit `--migrate-kitsune` when no legacy references
are being replaced. Do not run Kitsune legacy cleanup first if you need Coven to
verify and capture the exact legacy preimage for migration/rollback.

## Related files

- `src/integration/{actions,registry,targets,mod,types}.rs`
- `src/app/api/integrations.rs`, `src/integration/tests.rs`
- [App boundaries](APP_BOUNDARIES.md), [roadmap](KITSUNE_ROADMAP.md),
  [generic views](INTEGRATION_VIEWS.md)
- Optional sibling contracts: [Coven hooks](../coven/CLAUDE_HOOKS_CONTRACT.md),
  [receiver](../coven/AGENT_MANAGEMENT_CONTRACT.md),
  [authoritative migration matrix](../coven/AGENT_MANAGEMENT_MIGRATION.md)
