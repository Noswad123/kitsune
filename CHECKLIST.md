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

- [x] Split embedded captures into reusable template files.
  - [x] `workspaces/*.yaml`
  - [x] `tabs/*.yaml`
  - [x] `panes/*.yaml`
  - [ ] `stacks/*.yaml`
- [x] Add refs between templates.
  - [x] workspace references tabs
  - [x] tab references panes
  - [ ] stack references workspaces
- [ ] Add composition commands.
  - [ ] `kit restore workspace <name>`
  - [ ] `kit add tab <name> --to <workspace>`
  - [ ] `kit restore stack <name>`

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
  - [ ] `--dry-run`
  - [ ] `--skip-commands`
  - [ ] `--confirm`
  - [ ] better layout conflict handling
  - [ ] clearer summaries before execution

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
