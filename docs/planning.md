# Planning

This file holds design topics that are not implementation-ready. Promote items
out of planning only after the desired behavior, user experience, safety model,
and acceptance criteria are clear.

## Tmux backend completion

Completed foundation:

- session-as-workspace save
- window-as-tab save
- pane save

Needs design before implementation:

- tmux layout restore semantics
  - Decide whether Kitsune should restore exact tmux layouts, approximate splits,
    or only topology/metadata.
  - Define how to handle existing sessions/windows/panes vs creating new ones.
  - Define duplicate-label/session collision behavior.
  - Define dry-run output and confirmation safety.

## Future backend exploration

### Zellij backend

- Decide whether Zellij fits Kitsune's workspace/tab/pane model cleanly.
- Identify capture, restore, action, and navigation primitives.
- Define what parity with Herdr/tmux would mean.

## Kitsune as a multiplexer

- Explore whether Kitsune should remain a companion/orchestrator or eventually
  own multiplexer behavior directly.
- Clarify boundaries with Herdr/tmux/Zellij before any implementation work.
