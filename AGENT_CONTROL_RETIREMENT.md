# Global live agent detector and consumer retirement

## Current Retirement, 2026-11-16

Live retirement is global, with or without Coven. This supersedes the conditional
ownership cuts recorded below; it is source removal, not a new legacy-mode flag.
The earlier remote-updater deletion was a separate cut. The large existing dirty
diff also contains that and other prior work, not solely this retirement.

- Normal and imported/handoff pane detection loops, `pane/agent_detection.rs`,
  `detector_producer.rs` and its tests are deleted. Terminal/state live evidence
  arbitration consumers and internal DetectorUpdate/StateChanged/HookLifecycle
  events are gone. No native AgentStatus is inferred from screen, PTY activity,
  OSC or foreground hints.
- All 12 live `agent.*` typed methods return correlated `feature_retired` with
  fixed method context in both ownership modes, before lookup, validation,
  reconciliation or housekeeping. Convenience CLI controls/reads and raw API CLI
  use early local retirement helpers: no socket/protocol probe or native program.
  Valid legacy grammar/JSON flags remain; help exits 0 and invalid syntax exits 2.
- AgentStatus subscriptions and the `events.wait` agent matcher are retired.
  Mixed generic/agent requests reject atomically before any pattern compilation,
  probe or stream. Generic output/scroll/workspace subscriptions, generic event
  streaming and `pane.wait_for_output` remain. Previously unsupported generic
  `events.wait` matchers remain unsupported, not newly implemented.
- The four legacy pane hook methods return `feature_retired` standalone;
  external mode retains its older `external_agent_management` precedence for
  those hooks only. Generic `pane.report_metadata` is token-only with existing
  sequencing, TTL, events and revision semantics. Any request mixing tokens with
  retired agent-presentation fields rejects atomically, with no partial token write.
- Snapshot `agents: []` applies in both modes. Historical pane `agent_session`,
  names and argv remain data, not readiness, aliases, inventory projections or
  authorization. No provider autostart comes from these fields or history.
  The agent resume execution scheduler, managed reconciler and idle-to-finished
  sounds/toasts/notifications are deleted. Explicit `notification.show` remains generic.
- Manual pane labels and OSC titles remain; navigator historical agent-name
  fallback, mobile agent counts/"no agents" and agent:shell menu clauses are removed.
  Old `session.resume_agents_on_restore` and `experimental.cjk_ime_agents` values,
  including wrong legacy types, are ignored with diagnostics without breaking
  unrelated valid config. Explicit generic CJK reveal remains, without agent filtering.
- Fresh legacy installation for all six targets on every platform retires through
  actions/API/CLI before discovery, version probes, filesystem writes or logging.
  Recommendation/update policy and default-app discovery/version-probe modules
  are removed. Status/uninstall, migration fingerprints, frozen assets, licenses
  and upstream fixtures remain for explicit cleanup. No automatic uninstall occurs.
  `status --outdated-only` is accepted but supplies no updater candidate. Legacy
  settings now guide cleanup, not installation. This does not qualify Coven macOS
  or Windows replacements; generic Integrations sources/leases/views are separate.

### Generic Runtime And Restore

`TerminalRuntime` retains its generic UUID independently of PID. The 100 ms generic
terminal maintenance tick restores quiet-shell colors/notifies without agent
meaning. OSC 0/2 title tracking, CWD, clipboard, history and actual child wait/import
EOF handling remain generic. Old native references do not suppress bounded primary
screen history on handoff; alternate-screen history follows existing generic
policy, with no new pixel-recovery claim.

The optional, default-absent JSON snapshot field `respawn_shell_on_exit` records
actual shell policy. Import honors only `Some(true)`, without argv/FD inference;
cold restore remains false even if saved true. Old missing values mean false.
Existing binary DTOs and client protocol 18 are unchanged. Generic launch metadata
is not executed on restore. Historical native references retain Coven recovery's
conservative skip veto, never permission to resume.

### Remaining Compatibility

Static bundled/local rule evaluation and `agent explain --file ... --agent ...`
remain offline compatibility maintenance, not live status. Old request/response
and snapshot decoding plus explicit cleanup assets/status/uninstall also remain.
Windows generic process selection still uses `identify_agent_in_job` and advertises
`legacy_agent_aware`: this separate known acquisition contract is rejected by Coven
and is not a lifecycle producer. Inert Agent-named types/source residue await
cleanup; the entire detect tree is not deleted. No global runtime agent-manager
fallback exists. External ownership now expresses reporting-route intent and the
legacy-hook error distinction, not permission to re-enable built-ins.

Richer providers and remote hosting remain user-deferred. Mobile adoption remains
unapproved. No new native A3 capability is delivered by deletion. See
[Coven's decisions](../coven/docs/KITSUNE_RETIREMENT_FEATURE_DECISIONS.md).

### Operational Reads

Use `kitsune pane read <pane> --source detection --format text` for generic
bottom-buffer captures, not viewport or native classification. Detection text and
ANSI currently both return plain text; do not promise styling. The capture helper's
`--agent` path is retired locally; `--pane` uses only generic pane APIs, never
agent get/list/explain. Recorded source commands are provenance metadata;
compatibility identity fields stay empty rather than claiming detected identity.
See [operational examples](docs/next/README.md#generic-api-examples).

## Latest Recorded Validation

These are supplied implementation results, not reruns by this docs-only refresh.
Do not aggregate earlier/overlapping focused gates with these totals.

| Gate | Result And Limits |
| --- | --- |
| Full Kit native Linux suite | 2,739 passed executions, 5 intentional ignores, 0 failures; one job/two threads. 2,546 unit plus 193 integration executions: 157 target-specific plus 36 shared-support executions (4 x 9). Deduplicated integration cases: 161; unit plus deduplicated integration: 2,707. All 11 integration targets ran, including 14 `auto_detect` tests for generic server autospawn, not agent detection. No doctest targets. |
| Fresh Coven -> Kit -> Djinn source gate | 12 scenarios passed across primary Coven-server and standalone NoCoven modes. Primary: 69 global feature-retirement requests plus four hooks retaining the external error. NoCoven: 14 feature-retirement requests including four hooks. CLI: 22 local refusals plus four usage errors, zero API/client canary connections, unchanged HOME/XDG. Actual SDK/plugin/controller fixture, not physically rendered UI or separate Worker handshake. |
| Focused checks | Kit 185, Coven 75, Bun 39 and fixture 1 passed; overlapping, not additive. Generic metadata TTL/events, historical handoff references and no input/native execution covered. |
| Python and frozen assets | 77 maintenance plus 6 capture tests passed, separate from Rust counts. 15 frozen JS asset tests passed previously; not rerun in the latest full gate because source was unchanged. Frozen uninstall material preserved. |
| Static checks | Formatting and test-target checks compiled warning-free. Strict Clippy still has five unchanged baseline findings at ghostty 670/684, handoff 381, terminal_theme 125 and platform/linux 633; no new retirement lint. Not a green strict lint/full-CI claim. |

Native validation used Rust 1.99.1, not the repository's 1.96.1 pin: no pinned
toolchain or cross-OS qualification follows. No real providers/models, user
endpoints or installs were exercised. All owned artifacts were cleaned; the latest
cache check was empty, so no retained run log/build tree is linked. The durable
source-gate description is [Coven's harness record](../coven/tools/normal-workflow-check/README.md#a2-source-native-interop).
This refresh checks documentation only, not builds, installs, restarts or user config.

## Historical Cuts

Everything below records earlier scoped cuts and overlapping validation. Statements
that live/standalone behavior remains or is the next cut are historical, superseded
by the current global retirement above; they are not operational instructions.

## Globally removed: remote detector-manifest management

The remote manifest subsystem is physically deleted, not disabled by another
ownership guard. This applies with or without Coven. The updater module, catalog
discovery/fetching, remote cache/status readers and writers, startup thread,
recurring deadlines, completion event, update toast and management state are gone.

Both manifest API methods remain recognized for compatibility but return a
correlated `feature_retired` error in either ownership mode. All three old CLI
spellings (`server agent-manifests`, `server reload-agent-manifests`, and
`server update-agent-manifests`) fail locally without connecting to a server or
reading configuration, catalogs or status. Supported `--json` forms retain the
standard error envelope; invalid syntax still returns usage errors.

Historical successful-response DTOs remain decodable, not producible by these
handlers. The binary protocol remains 18; retaining request/response decoding is
not retaining a working update feature.

**Live pane screen/status detectors still remain temporarily.** Their static
rule source is now local override, then bundled rules; invalid local overrides
fall back to bundled rules with warnings. Cached remote rules and status files
are ignored and left unchanged on disk. The remaining in-memory static-rule cache
is not the removed remote disk cache. Restart to load changed local rules; no
live manifest-reload operation remains.

## External agent-ownership boundary

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
  compatibility diagnostic using local/bundled static rules only. Old remote
  explanation keys are fixed null/false compatibility fields, not status readers.
  This is not a supported live agent-status interface or a bypass into server-side
  detector management.

The other standalone `external_agent_management = false` agent branches remain
for subsequent removal; remote manifest management is gone in that mode too.
This is not a new permanent legacy-mode switch or a claim that all agent-aware
code has been deleted. The target remains ordinary multiplexing without built-in
agent features when Coven is absent.

## Configuration compatibility

Old `[agent_detection]` and legacy `[update]` settings are accepted as inert input
with a retirement diagnostic, including old enablement values or wrong field
types in syntactically valid TOML. They create no runtime flag or deadline and
cannot restore the deleted subsystem. Unrelated settings remain intact; malformed
whole-file TOML retains the existing fail-closed ownership behavior.

The generated configuration no longer advertises `manifest_check`, and executable
code no longer reads the catalog environment variable. No user configuration or
cache file is automatically edited, removed or migrated. The offline bundled-rule
maintenance script reads its engine version from the retained static evaluator,
not a replacement updater implementation.

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

An earlier control-only Kit gate passed 59 tests. Its composed local hosting gate passed
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
Those preflight helpers and updater tests were subsequently removed with the global
subsystem; these are historical records, not current operational promises.

The global source-removal cut passed 238 focused Rust checks and 77 maintenance
tests, including default offline validator execution and unchanged/ignored remote
files. The composed hosting gate passed nine scenarios: 43 agent-ownership
refusals, two permanent API retirements, zero connections to CLI API/client socket
canaries across valid and invalid command forms, intact generic operations, and
all seven source-native hosting scenarios. The CLI HOME/XDG tree and seeded remote
files were unchanged. These gates are scoped and not additive with older counts.

No method, parameter schema or binary protocol version is removed in this cut.
Existing clients get an explicit compatibility refusal rather than a successful
no-op. Installation does not live-update a running server; rebuild and deliberately
restart when ready to activate the boundary.

## Next retirement work

The next physical boundary is the live pane detector producer pipeline for both
ordinary and imported/handoff runtimes, followed by the remaining status/control
and UI consumers. Keep generic PTY/process observation, actual child-exit/CWD
handling, session operations and integration transport. Preserve necessary
snapshot decoding and uninstall/migration support separately from live policy.
Static/offline rule evaluation can disappear when its remaining consumers retire.
The remote updater must not be reintroduced as a compatibility shim or fallback.

Richer Codex/Copilot/Pi support remains user-deferred and is not a prerequisite for
this retirement. See the [remaining feature decisions](../coven/docs/KITSUNE_RETIREMENT_FEATURE_DECISIONS.md).
