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
kit save --current
kit save --current darkness
kit save darkness
kit save workspace darkness
kit save tab coding
kit save pane agent
kit save all
kit save all --plan
kit save all --append-snapshot
kit save workspace darkness --plan
kit save workspace darkness --append-snapshot
kit save all --no-reuse
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
Configure it in `config.yaml`:

```yaml
schema: kitsune.config.v1
nav:
  passthrough_regex: '(^|/)(g?view|l?n?vim?x?|fzf|hx|helix|lazygit)(diff)?$'
```

`KITSUNE_NAV_PASSTHROUGH` remains a one-off environment override:

```bash
export KITSUNE_NAV_PASSTHROUGH='(^|/)(g?view|l?n?vim?x?|fzf|hx|helix|lazygit)(diff)?$'
```

## TUI

`kit tui` opens a read-only split view with full-width top tabs for live backend
state and saved Kitsune templates. Use Tab to switch tabs, `h`/`l` to focus the
list or metadata pane, ↑/↓ or `j`/`k` to select rows or scroll metadata, `r` to
refresh, `s` to save the currently focused workspace, Space to multi-select
saved workspace/tab/stack templates, Enter twice to restore/apply the selected
template or selection, `c` to compare a saved workspace/tab/pane against matching
live metadata, `e` to open the current metadata in a Neovim temp buffer, or
`q`/Esc to quit. If temp-buffer
contents changed for a saved template, Kitsune asks for `y`/`n` confirmation and
persists only after validation passes. The selected live component or saved
template automatically previews full YAML metadata on the right. If temp-buffer
contents changed for a live workspace, tab, or pane, Kitsune asks for `y`/`n`
confirmation and `y` applies the display-label rename directly to Herdr via
`backend_ref.workspace_id`, `backend_ref.tab_id`, or `backend_ref.pane_id`.
Confirmed saved workspace/tab/pane template edits also attempt the same live
Herdr metadata sync. If the live object no longer exists, the saved edit remains
and the TUI reports the live-sync failure.

## Storage

By default templates and config live in the platform config directory under
`kitsune`:

```text
config.yaml
workspaces/
tabs/
panes/
stacks/
snapshots/
```

Use `KITSUNE_STORE` or `--store` to override the location, for example:

```bash
kit --store ~/.config/kitsune save darkness
```

## Stacks

Stacks are named refs to workspace templates. Create one from existing saved
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

Saved workspaces, tabs, and panes include a first-pass stable identity
fingerprint. Fingerprints are for recognizing reusable components and duplicate
templates; they intentionally ignore volatile runtime details like foreground
commands and pane dimensions.

Saved templates are componentized: workspace files contain refs to tab files, and tab
files contain refs to pane files. Parent templates do not embed full child
details.

Pane fingerprints currently use stable-ish fields:

- normalized label
- git root when available, otherwise cwd
- agent kind

Tab fingerprints derive from pane fingerprints. Workspace fingerprints derive
from tab fingerprints. During save, Kitsune reports matching saved tabs/panes
when it finds an existing template with the same fingerprint.

Use `--current` when you want to be explicit that Kitsune should save the
currently focused workspace. It is equivalent to bare `kit save`, and accepts
one optional saved name:

```bash
kit save --current
kit save --current darkness
```

Save reuses matching components by default. If a pane or tab fingerprint
already exists in the store, parent refs point at the existing component instead
of creating another duplicate. Preview the write/reuse plan with:

```bash
kit save all --plan
kit save workspace darkness --plan
```

Force fresh component writes with:

```bash
kit save all --no-reuse
```

Append a timestamped point-in-time save under `snapshots/` while also saving
normal reusable templates:

```bash
kit save all --append-snapshot
kit save workspace darkness --append-snapshot
kit save tab coding --append-snapshot
```

Snapshots are inert YAML records for review/auditing. They are not used by
restore/apply yet.

## Observed commands are inert

Saved foreground commands are stored as observed runtime state, not restore
behavior. This avoids rerunning whatever command happened to be focused at
save time. Kitsune does not support command execution during restore/apply at
this time; it restores topology only: workspaces, tabs, panes, labels, cwd, and
layout.

## Template metadata vs backend metadata

Saved templates keep editable intent separate from live backend state. Normal
saves do not serialize full Herdr `raw` blobs into workspace/tab/pane files.
Instead, Kitsune stores a compact `backend_ref` with only the IDs needed for
focus detection and live pane sync. Optional fields such as `label` are omitted
when absent, and labels matching `name` are not duplicated.

Herdr-generated pane labels that look like runtime fallback names, for example
`rustlings-1-wy-p5`, are treated as unnamed panes during save. New saves
use compact tab-scoped fallback names such as `rustlings-1-pane-1`. Existing
saved templates with old names remain valid; use `--no-reuse` or edit/migrate
them if you want fresh clean component filenames.

## Current status

- Herdr backend: doctor, save workspaces/tabs/panes, restore/apply topology,
  duplicate-label safety, smart nav.
- Tmux backend: doctor and smart nav foundation; save/restore pending.
- TUI: initial browser shell; richer selection/edit/compose flows pending.
