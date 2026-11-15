# Terminal Navigation v1

These additive JSON APIs expose generic multiplexer navigation, not agent or
provider liveness. The private binary protocol remains version 18.

`session.snapshot` advertises support in `result.snapshot`:

```json
{"terminal_navigation":{"version":1}}
```

The descriptor is optional when decoding older snapshots. Absence, an unsupported
version, or an unknown-method response means unsupported. Clients must not fall
back to `agent.focus`, `pane.focus`, names, or inferred pane aliases.

## Inspect

```json
{"id":"inspect-1","method":"terminal.inspect","params":{"terminal_id":"term_exact"}}
```

```json
{"id":"inspect-1","result":{"type":"terminal_inspected","terminal_id":"term_exact","runtime_generation":"1513aa80-6070-4115-8c4c-175b3bfd6ba6","availability":"live","workspace_id":"w1","tab_id":"w1:t1","pane_id":"w1:p1"}}
```

Use the exact opaque `terminal_id` exposed by the snapshot's panes. Both methods
require their documented string fields and reject unknown parameter fields.
Neither resolves pane IDs, names, labels, or agent identities.

Availability is one of:

- `live`: attached in the current layout, with a runtime whose PTY process has not exited.
- `exited`: attached runtime with recorded child completion (or imported PTY EOF).
- `unavailable`: attached terminal without a runtime.
- `detached`: known terminal not attached in the current layout.

The three locator fields are always present, and are all `null` when detached.
`runtime_generation` is always present, and is `null` when no runtime exists.
Detached runtimes still expose their generation when present. A missing terminal
is an error, not an inferred attachment. Ambiguous layout attachments are errors.
Detached takes precedence over runtime exit/unavailability.

Each runtime construction gets a fresh random UUID v4, including spawn,
replacement, and handoff import. Moving an existing runtime does not change it.
The generation is opaque and ephemeral: it is not persisted, imported, derived
from a PID, or shared with the internal nonserialized detector epoch. An alive
shell does not establish that any native provider is alive.

## Focus

```json
{"id":"focus-1","method":"terminal.focus","params":{"terminal_id":"term_exact","expected_runtime_generation":"1513aa80-6070-4115-8c4c-175b3bfd6ba6"}}
```

```json
{"id":"focus-1","result":{"type":"terminal_focused","terminal_id":"term_exact","runtime_generation":"1513aa80-6070-4115-8c4c-175b3bfd6ba6","workspace_id":"w1","tab_id":"w1:t1","pane_id":"w1:p1"}}
```

Success means the server accepted the focus state update, not that physical
pixels were displayed or an OS window was raised. The returned locator is the
current location, even if the terminal moved since inspection. The handler checks
the exact current generation, attachment, and PTY exit state before any focus
mutation. It updates generic focus and settles terminal mode only. It does not
acknowledge agent attention, mark tabs seen, submit shell/provider commands or
user input, or query provider status. Cached foreground-focus synchronization,
including subsequent snapshot and integration-view polling, does not acknowledge
attention either. Actual client interaction and intentional legacy focus actions
retain their standalone seen-state behavior. None of these are Coven acknowledgements.
Semantics are identical with external agent management enabled or disabled.

Normal post-dispatch focus synchronization may emit generic navigation events and
DEC focus reports to terminals that enabled focus reporting (`CSI ? 1004 h`).
These protocol bytes are permitted: `ESC [ O` to the previously focused terminal
and `ESC [ I` to the newly focused terminal (or focus-lost when the outer terminal
is unfocused). They are not shell/provider commands, user input, or attention
acknowledgements. No zero-terminal-bytes guarantee applies to successful focus
when reporting is enabled; terminals without focus reporting receive no such bytes.

## Errors

Errors use the existing correlated envelope:

```json
{"id":"focus-1","error":{"code":"runtime_generation_mismatch","message":"terminal runtime has changed"}}
```

Navigation error codes are `terminal_not_found`, `terminal_ambiguous`,
`terminal_detached`, `terminal_unavailable`, `runtime_generation_mismatch`, and
`terminal_exited`. Invalid request shapes use the existing JSON parsing error
path. A rejected focus request does not change focus, terminal mode, agent seen
state, input queues, or emit navigation/agent events. Normal independently queued
runtime events may still be drained by the server's existing event loop.

The API is an observation/admission boundary, not a lease: a process can exit or a
runtime can be replaced after the response. Consumers should inspect again after
stale-generation or availability errors instead of weakening target validation.
