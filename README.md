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
kit capture darkness
kit capture workspace darkness
kit capture tab coding
kit capture pane agent
kit capture all
kit list
kit list workspaces
kit show workspace darkness
kit validate
kit restore darkness --dry-run
kit restore darkness --skip-commands
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

The default passthrough regex matches Vim/Neovim/view/fzf. Override it with:

```bash
export KITSUNE_NAV_PASSTHROUGH='(^|/)(g?view|l?n?vim?x?|fzf|hx)(diff)?$'
```

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

## Current status

- Herdr backend: doctor, capture current workspace, restore workspace, smart nav.
- Tmux backend: doctor and smart nav foundation; capture/restore pending.
- TUI: initial browser shell; richer selection/edit/compose flows pending.
