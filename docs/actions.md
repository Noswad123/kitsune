# Actions

Workspace, tab, and pane templates can define explicit named actions. Actions are
never inferred from observed foreground commands.

```yaml
actions:
  start:
    description: Start the dev server
    command: cargo run
    cwd: ~/Projects/example
```

Run actions with:

```bash
kit run start rustlings
kit run start rustlings --dry-run
kit run test rustlings-1 --kind tab --dry-run
```

Target lookup prefers workspace, then tab, then pane. So if a `rustlings`
workspace exists, `kit run start rustlings` fans out from the workspace.

Fan-out behavior:

- workspace action: runs matching workspace, tab, and pane actions
- tab action: runs matching tab and pane actions
- pane action: runs the pane action

Pane actions with a live Herdr `backend_ref.pane_id` send their configured
command plus Enter into that pane. If a saved pane ID is stale, Kitsune attempts
to remap it to the current live Herdr pane by pane name/fingerprint. Pane actions
without a live pane ref run locally.

Use `--dry-run` to preview the command plan. Use `--confirm` when you want an
extra prompt before execution.

Diagnose configured actions and live Herdr pane targeting with:

```bash
kit doctor actions
```

The report shows configured actions, whether pane actions resolve to current live
Herdr panes, and warnings for unresolved live pane targets.

Observed foreground commands remain inert and are not action config.
