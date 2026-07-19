# Kitsune usage

## Install locally

```bash
cargo install --path .
```

This installs the daily command as `kit`.

## Common commands

```bash
kit doctor
kit doctor actions
kit init
kit store path
kit store doctor
kit validate
kit tui
```

Save live state into reusable templates:

```bash
kit save --current
kit save darkness
kit save workspace darkness
kit save tab coding
kit save pane agent
kit save all --plan
kit save all --append-snapshot
kit save all --no-reuse
```

Inspect saved templates:

```bash
kit list
kit list workspaces
kit show workspace darkness
kit tree workspace darkness
kit tree stack morning
```

Compose and apply templates:

```bash
kit stack create morning darkness rustlings
kit add tab logs --to darkness
kit add tab logs --apply
kit apply tab logs --dry-run
kit apply workspace darkness --dry-run
kit apply stack morning --dry-run
kit restore stack morning --confirm
```

By default, Herdr restore/apply refuses duplicate live workspace or tab labels.
Use `--force` only when an intentional duplicate is desired.

## TUI

`kit tui` opens a live/template browser.

Core keys:

- `Tab` / `Shift-Tab`: switch Live/Templates
- `h` / `l`: focus browser/detail pane
- `j` / `k` or arrows: move selection or scroll metadata
- `s`: save focused workspace
- `Space`: multi-select saved workspace/tab/stack templates
- `Enter`: arm, then restore/apply selected template(s)
- `c`: compare saved template to matching live metadata
- `e`: edit metadata in a Neovim temp buffer
- `y` / `n`: confirm/discard changed edit buffer
- `r`: refresh
- `q` / `Esc`: quit

## Storage

By default templates and config live under the platform config directory:

```text
~/.config/kitsune/
  config.yaml
  workspaces/
  tabs/
  panes/
  stacks/
  snapshots/
```

Use `KITSUNE_STORE` or `--store` to override the store path:

```bash
kit --store ~/.config/kitsune save darkness
```
