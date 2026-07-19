# Templates

Kitsune stores reusable working-session templates as YAML.

```text
workspace -> tab refs
tab       -> pane refs
stack     -> workspace refs
```

Template identity:

- `name`: Kitsune/template identity
- `label`: live/display intent
- `backend_ref`: compact link to live backend IDs

Saved templates keep editable intent separate from live backend state. Normal
saves do not serialize full backend raw blobs into workspace/tab/pane files.

## Fingerprints

Saved workspaces, tabs, and panes include stable-ish fingerprints for component
reuse and duplicate detection.

Pane fingerprints use:

- normalized label
- git root when available, otherwise cwd
- agent kind

Pane fingerprints intentionally ignore volatile runtime details like foreground
commands and pane dimensions. Tab fingerprints derive from pane fingerprints.
Workspace fingerprints derive from tab fingerprints.

## Observed commands

Saved foreground commands are stored as observed runtime state only:

```yaml
observed:
  foreground_command: nvim
```

Restore/apply does not run observed commands. Use explicit [Actions](actions.md)
for commands that should run.
