# Kitsune Checklist

This is the maintained top-level roadmap for Kitsune (`kit`). Keep it current as
features land or priorities change.

## Immediate

- [x] Make config storage explicit.
  - [x] `kit store path`
  - [x] `kit store doctor`
  - [x] clearly show whether `~/.config/kitsune` is stowed/symlinked
- [x] Add validation commands.
  - [x] `kit validate`
  - [x] detect missing directories
  - [x] detect invalid YAML/templates
  - [x] detect broken refs
  - [x] detect unsupported backend features
- [x] Add tree inspection for saved templates.
  - [x] `kit tree workspace <name>`
  - [x] `kit tree tab <name>`
  - [x] `kit tree pane <name>`
  - [x] `kit tree stack <name>`

## Capture

- [x] Improve capture naming.
  - [x] `kit capture workspace <name>`
  - [x] `kit capture tab <name>`
  - [x] `kit capture pane <name>`
- [ ] Add broader capture modes.
  - [ ] `kit capture --current`
  - [x] `kit capture all`
  - [ ] `kit capture --append-snapshot`

## Composable templates

- [x] Add first-pass stable fingerprints for component matching.
  - [x] pane fingerprints ignore command and dimensions
  - [x] tab fingerprints derive from pane fingerprints
  - [x] workspace fingerprints derive from tab fingerprints
  - [x] capture reports matching saved tabs/panes
  - [x] validation warns on duplicate fingerprints
- [x] Add capture planning and component reuse.
  - [x] `kit capture all --plan`
  - [x] `kit capture workspace <name> --plan`
  - [x] `kit capture tab <name> --plan`
  - [x] reuse matching pane refs by fingerprint
  - [x] reuse matching tab refs by fingerprint
  - [x] `--no-reuse` escape hatch

- [x] Split embedded captures into reusable template files.
  - [x] `workspaces/*.yaml`
  - [x] `tabs/*.yaml`
  - [x] `panes/*.yaml`
  - [x] `stacks/*.yaml`
- [x] Add refs between templates.
  - [x] workspace references tabs
  - [x] tab references panes
  - [x] stack references workspaces
- [ ] Add composition commands.
  - [x] `kit restore workspace <name>`
  - [x] `kit add tab <name> --to <workspace>`
  - [x] `kit add tab <name>` defaults to current workspace
  - [x] `kit add tab <name> --apply`
  - [x] `kit apply tab <name>`
  - [x] `kit apply workspace <name>`
  - [x] `kit apply stack <name>`
  - [x] `kit restore stack <name>`
  - [x] `kit stack create <name> <workspace...>`

## TUI

- [ ] Build the TUI around live state vs saved kits.
  - [ ] live backend state pane
  - [ ] saved Kitsune templates pane
  - [ ] capture action
  - [ ] restore action
  - [ ] view action
  - [ ] diff action
  - [ ] edit action
  - [ ] multi-select

## Restore safety

- [ ] Harden restore flows.
  - [x] `--dry-run`
  - [x] `--skip-commands`
  - [x] `--confirm`
  - [ ] better layout conflict handling
  - [x] clearer summaries before execution

## Navigation

- [ ] Replace `herdr-smart-nav` with `kit nav` once trusted.
- [ ] Move nav passthrough settings into Kitsune config.
  - [ ] Vim/Neovim/view/fzf defaults
  - [ ] Helix support
  - [ ] lazygit support

## Backends

- [ ] Complete tmux backend.
  - [ ] session-as-workspace capture
  - [ ] window-as-tab capture
  - [ ] pane capture
  - [ ] layout restore
- [ ] Explore future Zellij backend.
