# Generic interactive command submission

`pane.submit_command` queues one argument-encoded command into an explicitly pinned
existing terminal's available foreground shell. It is a general multiplexer API,
not an agent launcher, native worker admission, direct process exec or prompt API.
Existing `pane.run`, raw input and agent commands are unchanged.

## Discovery and request

The running server's `session.snapshot` advertises the optional descriptor:

```json
"command_submission": {"version": 1}
```

Require the supported integer version before submitting. Missing/malformed/future
support must not become an unguarded `pane.send_input` fallback. Protocol/package
versions or the CLI's embedded schema do not establish this capability. The new
JSON method/descriptor do not change private binary wire protocol 18.

Copy exact current `pane_id` and `terminal_id` from the same selected server:

```json
{
  "id": "one-request",
  "method": "pane.submit_command",
  "params": {
    "pane_id": "<current pane>",
    "expected_terminal_id": "<that pane's terminal>",
    "argv": ["/absolute/executable", "literal argument", ""]
  }
}
```

All three parameters are required and unknown fields are rejected. `argv` has
1..64 string entries, an absolute nonempty executable, no control characters in
any argument, and at most 32768 total UTF-8 argument bytes. Later empty arguments
remain literal empty arguments. The server encodes arguments for its observed
shell; callers must not pre-quote them or include a shell program string.

At final dispatch the server resolves only the exact current pane, verifies the
terminal pin, requires live state/runtime and an available supported foreground
shell, validates the command and queues its text plus Enter in one frame using
current terminal paste/key encoding. It creates/replaces/focuses no panes, and
creates no managed-agent identity, labels, lifecycle state or detection override.
No cwd is changed; the selected shell's existing cwd/environment/policy applies.
An absolute executable spelling is not executable validation or authentication:
ordinary native errors can still occur after queuing.

## Acknowledgement and rejection

```json
{
  "id": "one-request",
  "result": {
    "type": "pane_command_submitted",
    "pane_id": "<current pane>",
    "terminal_id": "<expected terminal>",
    "argv": ["/absolute/executable", "literal argument", ""]
  }
}
```

This means **input queued**, not process started, shell/native acceptance, correct
session selection, readiness, permission to use tools, or job completion. There
is no durable request-ID receipt or exactly-once execution contract. A lost/invalid
acknowledgement is uncertain; never automatically resubmit it or try raw keys.

Errors before successful enqueue write no part of this submission and install no
metadata: `invalid_command_argument`, `pane_not_found`, `terminal_mismatch`,
`pane_command_unavailable`, `pane_command_busy`, `unsupported_shell`,
`pane_command_input_failed`. Wrong JSON types/missing/unknown fields fail decoding.
No error-precedence guarantee is made for multiple invalid fields. Ordinary API
housekeeping remains independent of this handler.

## Limits

Select a **quiescent empty shell prompt**. Foreground-shell availability is an OS
observation before enqueue, not an atomic process/prompt lease; another writer or
foreground change can race subsequent PTY consumption. No prompt/line-buffer
inspection, automatic Ctrl-C/line clearing, sandbox, binary authentication, native
lifecycle supervision, cancellation or startup wait is provided. Do not use this
API where stronger native admission or resource/tool containment is required.

The local API's existing access boundary still applies. JSON target pins protect
against ordinary terminal replacement/moves, not malicious same-user interference.
No Coven/Djinn dependency is required; those apps may explicitly consume this
generic capability and must own their native argument and authority semantics.

## Validation

`just test-one command_submission` exercises strict decoding, old-snapshot
compatibility, boundaries, literal/empty/Unicode arguments, full-frame encoding,
target replacement and busy/unavailable/backpressure rejection without agent
metadata. It also includes a real private-server/shell integration case using a
harmless temporary executable: exact literal argv (including shell metacharacters),
busy-target rejection and terminal mismatch. No native agent/model is used.
`just test-one generated_protocol_schema_artifact_is_current` verifies the generated
API schema. Channel fixtures do not run executables or native agents.
