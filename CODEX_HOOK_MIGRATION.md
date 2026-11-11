# Codex hook ownership migration to Coven

## Overview

**New Codex hook installation on Linux is retired in Kitsune and owned by Coven**,
matching the earlier [Claude cut](CLAUDE_HOOK_MIGRATION.md). This moves hook
configuration and reference capture, not full discovery/status/start/resume/restore.
The target remains all agent management in Coven, generic multiplexing and optional
presentation/messaging in Kitsune, and native execution/policy/trust in providers.
The authoritative [ownership matrix](../coven/AGENT_MANAGEMENT_MIGRATION.md) and
[Codex contract](../coven/CODEX_HOOKS_CONTRACT.md) are optional sibling navigation,
not startup dependencies. No automatic execution of Coven, dependency/source copy,
user-config migration or live-session conversion occurs.

## API / Interfaces

| Kitsune surface | Current behavior |
| --- | --- |
| Linux `integration install codex` / `install_target(Codex)` | Immediately refuses with Linux-qualified Coven guidance before provider PATH/version/config discovery or install logging; never auto-runs Coven. |
| Linux API `integration.install`, target `codex` | `integration_install_failed` with the same guidance, not an installed result. |
| Linux startup recommendation / update / bulk candidates | Codex excluded; explicit update guidance points to Coven and independent native trust. |
| Non-Linux installer/recommendation/update/bulk paths | Legacy installer, assets and result type preserved until matching Coven platform adapters are ready. This is code preservation, not new platform validation. |
| Read-only legacy status, `integration uninstall codex`, old hook ingress | Retained compatibility on all platforms; Kit uninstall is legacy cleanup only, not Coven uninstall/rollback. |
| All six `IntegrationTarget::ALL` entries and legacy UI settings | Retained; fixed legacy integration targets are not the generic `integrations.sources` registry. |
| Process/screen fallback, discovery/status, start/resume, agent-aware restore | Still compatibility work pending transfer for Codex and all providers; generic terminal/session restore remains Kit-owned. |

`legacy_install_codex`, embedded assets and `CodexInstallPaths` are retained under
`cfg(any(test, not(target_os = "linux")))`: test fixtures on Linux, production
compatibility elsewhere. Linux retirement is not hidden all-platform retirement.
Coven mutation is Linux-only; other Unix platforms can preview, not apply.
Its receiver is Unix-only, Windows unsupported and macOS unvalidated.

Explicit Coven interfaces (paths are inspected absolute placeholders):

```text
coven agent serve WORKSPACE --socket ABS_COVEN_SOCKET --duration 60 [--kitsune-socket ABS_KITSUNE_SOCKET --source coven] [--json]
coven agent report --provider codex --socket ABS_COVEN_SOCKET [--event SessionStart] [--json]
coven agent inventory --socket ABS_COVEN_SOCKET --json
coven agent hooks plan|install|uninstall|rollback --provider codex --config-root ABS_CONFIG_ROOT [--executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET] [--migrate-kitsune] [--apply] [--json]
```

The common event parser lists Claude events too, but Codex domain validation
accepts only **SessionStart**. No source flag, implicit HOME, provider discovery,
version probing or provider/daemon start is added by Coven's manager.

## Data Flow

1. Select an existing owned root, trusted Coven executable and private socket.
   Coven validates **both `hooks.json` and `config.toml`**, receipt and generated
   images before any apply artifacts, including the lock. Each input is bounded
   to 1 MiB. TOML editing with `toml_edit` preserves comments/unrelated semantics;
   inline/dotted `features` layouts refuse as unsupported.
2. Preview adds one nested `hooks.SessionStart` group with nested `hooks`,
   `type: "command"`, `timeout: 2` and **no matcher**. Its shell-quoted command invokes
   only Coven `agent report --provider codex --socket … --event SessionStart`, with
   silent best-effort `>/dev/null 2>&1 || true`, never Kit or the provider.
3. Preview discloses setting **global `[features].hooks = true`**, including
   absent/false → true, and removing **only `features.codex_hooks`**. Explicit
   install `--apply` authorizes this shared enablement, potentially affecting other
   custom hooks, **not native approval**. `[hooks.state.KEY]` `enabled`,
   `trusted_hash`, `trusted_content_hash` remain user/native-controlled. No approval
   bypass or hash manufacture. Installed config alone cannot prove capture works.
4. With writers stopped, Linux apply locks, rereads/revalidates, persists a private
   0600 bounded **v1** two-file journal, then writes **hooks first, TOML second,
   final journal last**. All original/source/target bytes, modes and absence are
   recorded. This is ordered recoverable sequencing, not global atomicity/CAS.
   Shared private `hook_files` storage preserves Claude behavior/v2 receipts/errors/
   statuses; Codex has separate allowlists/recovery names and journal policy.
5. Native SessionStart stdin has seven required fields: session_id,
   hook_event_name (`SessionStart`), source (startup/resume/clear/compact/fork), cwd,
   nullable transcript_path, model, permission_mode. Coven deliberately captures
   **session ID/event only**, dropping all other/secret fields before RPC,
   store and output. No `thread_id`/`thread-id` alias is accepted; internal ThreadId
   naming is not a native wire alias.
6. The existing private Unix receiver stores schema_version **1**, with unchanged
   fields provider/session_id/last_event/timestamp_ms/server_epoch. Hashes are
   provider + NUL + exact SID: Claude IDs unchanged, equal SIDs across providers
   distinct. The 128-record cap is **aggregate**, not per provider. Codex stays
   unknown even fresh; all records restart unknown/null age until another receipt.
   Optional dynamic-provider rows use generic snapshot/`integration.view.*` only;
   Kit gains no provider model, selection grants no authority and UI failure cannot
   stop core capture. This is reference capture, not working/idle lifecycle.

### Exact legacy migration

`--migrate-kitsune` is required for recognized legacy command removal in either
file. Coven verifies root-level `kitsune-agent-state.sh` **version 6**, SHA-256:

```text
2343713ae8e665baa22968e155ced9fdc1269f43b490af230316754a18caaf9a
```

Only exact `bash 'ABS_ROOT/kitsune-agent-state.sh' ACTION` strings in direct native
command handler positions qualify: SessionStart session/idle; UserPromptSubmit or
PreToolUse working; PermissionRequest blocked; Stop idle. Edited/unknown references
or assets refuse. Custom native handlers, including valid structured MCP input,
stay data, never recursive ownership positions. The old asset is left **untouched**.
Pending source leaves needing migration reverify it; already migrated target leaves
do not require it. No live pane authority is released. Do not run Kit cleanup first
if Coven needs the exact legacy preimage for migration/rollback.

## Edge Cases

- Plan/install/uninstall/rollback preview unless apply; plan never writes even
  with apply. No live provider/user configuration was used in this docs task.
- Normal **Coven uninstall** removes only the managed Coven command and **leaves
  shared global hooks enabled**, never restoring Kit commands. Disabling the global
  flag is a separate user/native consent decision, not safe provider-local cleanup.
  Guarded rollback restores **both** original byte/mode/absence images (also after
  managed uninstall) only if all postimages are unchanged; it can explicitly
  restore Kit references and the original feature value/alias. Chmod-only changes
  and unknown edits conflict rather than being overwritten.
- Pending journals without a recovery directory resume only the **same operation**
  at a legal source/target prefix: source/source, target/source, target/target in
  hooks/TOML order. Plan inspects, but another stage or unknown edits refuse.
- Linux shared `RENAME_EXCHANGE`/`RENAME_NOREPLACE` retains displaced entries until
  verified. Detected races may already affect a live leaf and retain private 0700
  `.coven-codex-hooks-recovery/` artifacts (`expected`, `replacement`, `displaced`,
  `transaction.json` as applicable). Even preview refuses while it exists. Stop
  writers, preserve/inspect evidence, never blindly undo over newer edits. No
  automatic recovery command or protection against later old-FD writes. Private
  preimages can contain secrets; do not publish them. Uncertainty is not no-effects
  proof or rollback permission.
- Use owned no-follow roots/regular single-link leaves. Prepare the receiver's
  existing owned exact-0700 socket parent separately; `serve` writes local state,
  runs foreground, starts no provider and requires neither Kit nor native Codex.

### Pinned native evidence and validation limits

Read-only upstream format verification uses **openai/codex rust-v0.167.1**, commit
`0a94398fa60c4fac0e7659e4213a0a36e613dd46`:
[protocol hooks](https://github.com/openai/codex/blob/0a94398fa60c4fac0e7659e4213a0a36e613dd46/codex-rs/protocol/src/hooks.rs),
[hook config](https://github.com/openai/codex/blob/0a94398fa60c4fac0e7659e4213a0a36e613dd46/codex-rs/config/src/hook_config.rs),
[generated SessionStart input](https://github.com/openai/codex/blob/0a94398fa60c4fac0e7659e4213a0a36e613dd46/codex-rs/hooks/schema/generated/session-start.command.input.schema.json).
This is not an earliest-version/all-modes/native operational certification.

Owner currently reports **16 Codex workflows + 7 fault tests**, with review pending.
`tests/codex_agent_migration_workflow.rs` is being added in Coven to execute the real
generated command through Coven CLI/receiver, not prove Kit/native-provider
operation. Prior Claude counts remain historical; final counts are parent-owned.
This docs-only pass ran no shell/tests/build/Git/live operations and changed no
user config; implementation evidence uses fixtures, not a fresh full-suite or
all-platform pass.

## Example usage

Reference syntax only; both hook invocations are previews:

```text
coven agent serve WORKSPACE --socket ABS_COVEN_SOCKET --duration 60 --json
coven agent hooks plan --provider codex --config-root ABS_CONFIG_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune --json
coven agent hooks install --provider codex --config-root ABS_CONFIG_ROOT --executable ABS_COVEN_BINARY --socket ABS_COVEN_SOCKET --migrate-kitsune --json
coven agent inventory --socket ABS_COVEN_SOCKET --json
```

Omit migrate when no legacy references need replacing. Inspect feature changes:
`hooks_before: null/false` → `hooks_after: true`, plus
`native_hook_approval: "user-managed; may be required; not granted by this operation"`.
Only after reviewing shared enablement/native consent and stopping writers, add
`--apply` on Linux. No configuration write promises operational/native capture.

Complete pinned native input example (inert data):

```json
{"session_id":"example-unverified-id","hook_event_name":"SessionStart","source":"startup","cwd":"/example/workspace","transcript_path":null,"model":"example-model","permission_mode":"default"}
```

The narrower two-field Coven capture is not the complete native schema.

## Related files

- `src/integration/{actions,registry,targets,types,mod}.rs`, `src/app/api/integrations.rs`
- [Claude guide](CLAUDE_HOOK_MIGRATION.md), [app boundaries](APP_BOUNDARIES.md),
  [roadmap](KITSUNE_ROADMAP.md), [generic views](INTEGRATION_VIEWS.md)
- Optional sibling [Codex contract](../coven/CODEX_HOOKS_CONTRACT.md),
  [receiver contract](../coven/AGENT_MANAGEMENT_CONTRACT.md),
  [ownership matrix](../coven/AGENT_MANAGEMENT_MIGRATION.md)
