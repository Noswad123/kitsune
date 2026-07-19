# Kitsune (`kit`)

![Kitsune](img/kitsune.png)
Kitsune is a multiplexer companion for named, composable working-session kits.

It starts with Herdr support and is structured so tmux/Zellij/etc. can be added
behind the same model:

```text
workspace -> tab -> pane
stack     -> many workspaces
```

## Install locally

```bash
cargo install --path .
```

This installs the daily command as `kit`.

## Commands

```bash
kit doctor
kit init
kit store path
kit store doctor
kit capture --current
kit capture --current darkness
kit capture darkness
kit capture workspace darkness
kit capture tab coding
kit capture pane agent
kit capture all
kit capture all --plan
kit capture all --append-snapshot
kit capture workspace darkness --plan
kit capture workspace darkness --append-snapshot
kit capture all --no-reuse
kit stack create morning darkness rustlings
kit add tab logs --to darkness
kit add tab logs
kit add tab logs --apply
kit apply tab logs --dry-run
kit apply tab logs --force
kit apply workspace darkness --dry-run
kit apply stack morning --dry-run
kit restore stack morning --confirm
kit restore stack morning --dry-run
kit list
kit list workspaces
kit show workspace darkness
kit tree workspace darkness
kit tree stack morning
kit validate
kit restore darkness --dry-run
kit nav left ctrl+h
kit tui
```

## Herdr smart navigation

Kitsune can replace `herdr-smart-nav`:

```toml
[[keys.command]]
key = "ctrl+h"
type = "shell"
command = "kit nav left ctrl+h"
description = "smart focus pane left"
```

The default passthrough regex matches Vim/Neovim/view/fzf, Helix, and lazygit.
Override it with:

```bash
export KITSUNE_NAV_PASSTHROUGH='(^|/)(g?view|l?n?vim?x?|fzf|hx|helix|lazygit)(diff)?$'
```

## TUI

`kit tui` opens a read-only split view of live backend state and saved Kitsune
templates. Press `r` to refresh, or `q`/Esc to quit. Capture, restore, view, and
edit actions are planned next.

## Storage

By default templates live in the platform config directory under `kitsune`, with
subdirectories for:

```text
workspaces/
tabs/
panes/
stacks/
snapshots/
```

Use `KITSUNE_STORE` or `--store` to override the location, for example:

```bash
kit --store ~/.config/kitsune capture darkness
```

## Stacks

Stacks are named refs to workspace templates. Create one from existing captured
workspaces:

```bash
kit stack create morning darkness rustlings
kit restore stack morning --dry-run
```

Stack files live in `stacks/*.yaml` and reference workspaces by name.

Add an existing tab template to an existing workspace template:

```bash
kit add tab logs --to darkness
```

Omit `--to` to add the tab to the currently focused workspace template:

```bash
kit add tab logs
```

Use `--apply` to update the template and apply the same tab to the live
multiplexer session:

```bash
kit add tab logs --apply
kit add tab logs --to darkness --apply --dry-run
```

Use `apply` for live-only changes from saved templates:

```bash
kit apply tab logs
kit apply workspace darkness --dry-run
kit apply stack morning --dry-run
```

Add `--confirm` to prompt before live restore/apply changes:

```bash
kit apply tab logs --confirm
kit restore stack morning --confirm
```

By default, live restore/apply refuses to create duplicate Herdr workspace or tab
labels. Use `--force` when you intentionally want another live object with the
same label:

```bash
kit apply tab logs --force
kit restore workspace darkness --force
kit apply stack morning --force
```

## Fingerprints

Captured workspaces, tabs, and panes include a first-pass stable identity
fingerprint. Fingerprints are for recognizing reusable components and duplicate
templates; they intentionally ignore volatile runtime details like foreground
commands and pane dimensions.

Captures are componentized: workspace files contain refs to tab files, and tab
files contain refs to pane files. Parent templates do not embed full child
details.

Pane fingerprints currently use stable-ish fields:

- normalized label
- git root when available, otherwise cwd
- agent kind

Tab fingerprints derive from pane fingerprints. Workspace fingerprints derive
from tab fingerprints. During capture, Kitsune reports matching saved tabs/panes
when it finds an existing template with the same fingerprint.

Use `--current` when you want to be explicit that Kitsune should capture the
currently focused workspace. It is equivalent to bare `kit capture`, and accepts
one optional saved name:

```bash
kit capture --current
kit capture --current darkness
```

Capture reuses matching components by default. If a pane or tab fingerprint
already exists in the store, parent refs point at the existing component instead
of creating another duplicate. Preview the write/reuse plan with:

```bash
kit capture all --plan
kit capture workspace darkness --plan
```

Force fresh component writes with:

```bash
kit capture all --no-reuse
```

Append a timestamped point-in-time capture under `snapshots/` while also saving
normal reusable templates:

```bash
kit capture all --append-snapshot
kit capture workspace darkness --append-snapshot
kit capture tab coding --append-snapshot
```

Snapshots are inert YAML records for review/auditing. They are not used by
restore/apply yet.

## Observed commands are inert

Captured foreground commands are stored as observed runtime state, not restore
behavior. This avoids rerunning whatever command happened to be focused at
capture time. Kitsune does not support command execution during restore/apply at
this time; it restores topology only: workspaces, tabs, panes, labels, cwd, and
layout.

## Current status

- Herdr backend: doctor, capture workspaces/tabs/panes, restore/apply topology,
  duplicate-label safety, smart nav.
- Tmux backend: doctor and smart nav foundation; capture/restore pending.
- TUI: initial browser shell; richer selection/edit/compose flows pending.
