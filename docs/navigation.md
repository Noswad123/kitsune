# Navigation

Kitsune can replace the old `herdr-smart-nav` wrapper.

Use the Herdr-only fast path for keybindings:

```toml
[[keys.command]]
key = "ctrl+h"
type = "shell"
command = "~/.local/bin/kit herdr-nav left ctrl+h"
description = "smart focus pane left"
```

Use equivalent bindings for `down`, `up`, and `right`.

`kit herdr-nav` bypasses the full CLI/config startup path and uses
`KITSUNE_NAV_PASSTHROUGH` or the built-in passthrough regex. Use `kit nav` for
the normal backend-aware command.

The default passthrough regex matches Vim/Neovim/view/fzf, Helix, and lazygit.
Configure backend-aware nav in `config.yaml`:

```yaml
schema: kitsune.config.v1
nav:
  passthrough_regex: '(^|/)(g?view|l?n?vim?x?|fzf|hx|helix|lazygit)(diff)?$'
```

`KITSUNE_NAV_PASSTHROUGH` remains a one-off environment override.
