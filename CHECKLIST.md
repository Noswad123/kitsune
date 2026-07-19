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

## Save

- [x] Improve save naming.
  - [x] `kit save workspace <name>`
  - [x] `kit save tab <name>`
  - [x] `kit save pane <name>`
- [ ] Add broader save modes.
  - [x] `kit save --current`
  - [x] `kit save all`
  - [x] `kit save --append-snapshot`

## Composable templates

- [x] Add first-pass stable fingerprints for component matching.
  - [x] pane fingerprints ignore command and dimensions
  - [x] tab fingerprints derive from pane fingerprints
  - [x] workspace fingerprints derive from tab fingerprints
  - [x] save reports matching saved tabs/panes
  - [x] validation warns on duplicate fingerprints
- [x] Add save planning and component reuse.
  - [x] `kit save all --plan`
  - [x] `kit save workspace <name> --plan`
  - [x] `kit save tab <name> --plan`
  - [x] reuse matching pane refs by fingerprint
  - [x] reuse matching tab refs by fingerprint
  - [x] `--no-reuse` escape hatch

- [x] Split embedded saved state into reusable template files.
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
  - [x] live backend state pane
  - [x] saved Kitsune templates pane
  - [x] save action
  - [x] restore action
  - [x] view action
  - [x] diff action
  - [x] edit action
  - [x] multi-select

## Restore safety

- [ ] Harden restore flows.
  - [x] `--dry-run`
  - [x] `--confirm`
  - [x] saved/observed commands are inert and never run during restore/apply
  - [x] better layout conflict handling
    - [x] prevent duplicate live Herdr workspace/tab labels by default
    - [x] `--force` escape hatch for intentional duplicate labels
  - [x] clearer summaries before execution

## Actions

- [ ] Add action config for workspaces, tabs, and panes.
  - [ ] support named actions such as `start`, `stop`, `test`, or `dev`
  - [ ] add command shape like `kit run <action> <target>`
  - [ ] allow workspace-level actions to fan out to tab/pane actions
  - [ ] allow tab-level actions to fan out to pane actions
  - [ ] allow pane-level actions to run configured commands
  - [ ] example: `kit run start rustlings-workspace`
  - [ ] preserve restore/apply safety: actions must be explicit config, not observed commands
  - [ ] design confirmation/dry-run behavior before execution

## Navigation

- [x] Replace `herdr-smart-nav` with `kit nav` once trusted.
- [x] Move nav passthrough settings into Kitsune config.
  - [x] Vim/Neovim/view/fzf defaults
  - [x] Helix support
  - [x] lazygit support
