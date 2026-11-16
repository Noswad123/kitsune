# Legacy agent-control retirement under external ownership

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
- `agent.list`, `agent.read` and `agent.explain` remain transitional read facades.
  They are not new authoritative Coven status interfaces.

The standalone `external_agent_management = false` branches remain unchanged for
this bounded cut. This is not a new permanent legacy-mode switch or a claim that
all agent-aware code has been deleted. The target remains ordinary multiplexing
without built-in agent features when Coven is absent.

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

No method, parameter schema or binary protocol version is removed in this cut.
Existing clients get an explicit compatibility refusal rather than a successful
no-op. Installation does not live-update a running server; rebuild and deliberately
restart when ready to activate the boundary.

## Next retirement work

Audit and retire the remaining legacy read facades and their detector/status
dependencies as a separate coherent cut. Then remove agent-specific producers and
consumers together, retaining only necessary snapshot decoding and cleanup support.
Generic PTY/process/session operations and integration transport stay in Kitsune.

Richer Codex/Copilot/Pi support remains user-deferred and is not a prerequisite for
this retirement. See the [remaining feature decisions](../coven/docs/KITSUNE_RETIREMENT_FEATURE_DECISIONS.md).
