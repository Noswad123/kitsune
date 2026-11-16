# Legacy agent API and detector retirement under external ownership

## Current boundary

When the startup-frozen `external_agent_management` state is true, Kitsune refuses
the following legacy controls with the standard correlated error code
`external_agent_management`:

| Operation | Boundary |
| --- | --- |
| `agent.start` | Existing refusal before validation or managed launch/input state. |
| `agent.prompt` | No target resolution, foreground-agent lookup, encoding or input submission. |
| `agent.send_keys` | No target/key validation, runtime lookup or input submission. |
| `agent.rename` | Neither setting nor clearing a name can change agent ownership or managed state. |
| `agent.focus` | No focus change or implicit acknowledgement of unseen panes. |
| `agent.view.set` / `agent.view.clear` | No validation/replacement of legacy overrides or reset of panel/mobile scroll. |
| `agent.get` | No managed-agent reconciliation before reading. |
| `agent.wait` and prompt-with-wait | The public API server returns the initial guarded `agent.get` error; it does not submit a prompt or enter status polling. |
| `agent.list`, `agent.read`, `agent.explain` | No legacy inventory projection, agent-target lookup, terminal read or manifest explanation. Use Coven observations and generic pane reads/process observations. |
| `server.agent_manifests`, `server.reload_agent_manifests` | No manifest-cache/status lookup, App summary refresh or detector-runtime reset. |

Typed-handler refusal precedes semantic validation, so a bad target, invalid name,
empty prompt or invalid key cannot make the result depend on legacy state. Wire
syntax errors can still fail before a typed request exists. Error messages use
fixed guidance and do not echo supplied targets, names, prompts or keys.

Direct App reconciliation is independently inert under external ownership, and
direct focus/rename helpers return dedicated internal policy errors. Calling a
helper instead of its API wrapper cannot bypass the boundary. Helper availability
is not consulted: Coven failure never re-enables these controls.

## Retained functionality

- Generic terminal/pane reads, input, command submission and manual labels remain.
- `terminal.inspect` and generation-checked `terminal.focus` remain available.
  Generic focus may emit normal DEC focus-protocol traffic, but does not
  acknowledge Coven attention or become permission to submit provider input.
- Generic integration leases, frames and selection messaging remain; the retired
  `agent.view.*` facade is not the generic Integrations view API.
- Historical session references, pending historical intent and terminal history
  remain readable/preserved. This cut neither executes nor discards them.
- Legacy uninstall and migration material remain available.
- `session.snapshot` remains successful, with the same schema and `agents: []`.
  Historical `panes[].agent_session` references remain unchanged for conservative
  Coven recovery vetoes; an empty agent inventory does not erase those references.
- Explicitly offline `agent explain --file ... --agent ...` remains a local
  compatibility diagnostic. It is not a supported live agent-status interface or
  a bypass into server-side detector management.

The standalone `external_agent_management = false` branches remain unchanged for
this bounded cut. This is not a new permanent legacy-mode switch or a claim that
all agent-aware code has been deleted. The target remains ordinary multiplexing
without built-in agent features when Coven is absent.

## Detector/update admission

External ownership also suppresses startup manifest/status loading, background
updater creation, initial/reload deadlines and direct App updater work. Disabled
deadlines are cleared rather than left to busy-loop. Direct updater admission
returns before catalog discovery, network access, cache writes or even writing a
"skipped" status. Queued manifest-update events and direct runtime-reset helpers
cannot restore summaries, reset epochs or create update notifications externally.

`server update-agent-manifests` preflights the **selected server's** manifest-status
API before invoking the local updater. Refusal, transport failure or an unexpected
response stops before local mutation; the CLI does not infer ownership from an
uninitialized process-global policy or unrelated current configuration. A successful
preflight must match the probe's request ID and deserialize as the manifest-status
response schema, not merely contain the expected type string. Admitted standalone
ordering is preflight, local update, server reload, final status.
This is a point-in-time preflight, not an atomic transaction across concurrent
server handoff. A newly external server still refuses reset and queued updates.

## Side effects and validation

The refused controls preserve queued input, agent names and ownership, managed
phase/deadlines, historical references/resume intent, manual labels, terminal
revisions, focus and unseen state, existing view/scroll state, session dirtiness,
save scheduling and events. Shared dispatch can still perform pre-existing generic
housekeeping, such as draining unrelated events or synchronizing terminal titles;
this is distinct from executing the refused control.

Regression coverage includes seeded state that would otherwise operate, direct
helpers, typed API dispatch, actual server wait-wrapper dispatch, and controlled
headless focus dispatch. The new control-family tests were checked failing before
the guards and passing afterward. Existing standalone, generic command/navigation,
and historical-reference behavior remains characterized.

The final focused Kit gate passed 59 tests. The composed local hosting gate passed
eight scenarios, including 32 typed legacy-control requests against the actual
headless server, with correlated refusals, unchanged focus/terminal mappings and
no sentinel input. Generic input and all seven source-native hosting scenarios
still passed. That gate uses isolated Kit/Coven binaries and Djinn source
SDK/plugin/controller components, not a physically rendered Djinn UI or a separate
Bun Worker handshake. Counts overlap earlier focused checks and are not additive.
Test-target compilation and formatting pass. Strict Clippy remains blocked by
existing warnings in untouched files; this cut does not claim a green full lint
or full-repository gate.

The subsequent read/detector cut passed a focused union of 106 Kit tests and nine
composed scenarios. The actual-server preflights covered the previous 32 control
refusals plus 13 read/manifest requests, with correlated errors and no selector
echo. The built CLI updater returned the ownership refusal with its separate
HOME/XDG tree unchanged. Focus, terminal mappings, empty snapshot inventory and
historical fields stayed intact; generic reads (including detection text), owned
shell process inspection, input and all seven source-native hosting scenarios still
worked. Catalog-network ordering is additionally covered by fake callbacks; the
composed CLI used an invalid-scheme catalog URL, not a network-hit canary.
Earlier counts above are historical and overlapping, not additive totals.
The final typed-preflight hardening separately passed eight CLI/server tests,
including malformed/correlation cases that must not invoke the updater; test-target
compilation and formatting passed again afterward.

No method, parameter schema or binary protocol version is removed in this cut.
Existing clients get an explicit compatibility refusal rather than a successful
no-op. Installation does not live-update a running server; rebuild and deliberately
restart when ready to activate the boundary.

## Next retirement work

The live read facades and public manifest controls are now refused externally.
Next remove legacy detector/status producers and consumers across the remaining
standalone paths as a coherent implementation cut, retaining necessary snapshot
decoding, uninstall/migration support and clearly labeled offline diagnostics until
their separate retirement. Generic PTY/process/session operations and integration
transport stay in Kitsune. This pass changes admission and projections, not the
global default or the complete source-code removal.

Richer Codex/Copilot/Pi support remains user-deferred and is not a prerequisite for
this retirement. See the [remaining feature decisions](../coven/docs/KITSUNE_RETIREMENT_FEATURE_DECISIONS.md).
