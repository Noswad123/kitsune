# Kitsune docs

Kitsune started as a personal Herdr fork and is distributed as the `kitsune` command.

The hosted Herdr website, versioned website docs, hosted updater manifests, and plugin marketplace docs do not apply to this fork. Keep durable local documentation in repository docs and keep the bundled API schema in `docs/next/api/`.

## Runtime identity

Kitsune has its own runtime identity while retaining Herdr's core terminal-agent runtime.

- `kitsune` is the canonical command.
- Default config, session, socket, and log locations use Kitsune names rather than Herdr names.
- Runtime controls use `KITSUNE_*` environment variables, including pane identity and socket selection.
- Panes launched by Kitsune receive `KITSUNE_ENV`, `KITSUNE_WORKSPACE_ID`, `KITSUNE_TAB_ID`, and `KITSUNE_PANE_ID`.
- Primary CLI help/version output is Kitsune-facing.
- Config inspection/reload workflows include `config path` and socket-aware config reload.

Some deeper internal module/type names can remain Herdr-named while the fork is young. This keeps upstream changes easier to inspect and avoids large rename-only diffs until the user-facing product identity is stable.

## Session recall

Kitsune exposes session recall through the normal keybinding/action system.

- Action name: `session_recall`.
- Default binding: `prefix+shift+s`.
- UI shape: popup terminal that launches the existing session recall TUI.
- Default helper backend: `kitsune`.
- Helper context: `KITSUNE_ENV`, `KITSUNE_BIN_PATH`, `KITSUNE_SOCKET_PATH`, `KITSUNE_CLIENT_SOCKET_PATH`, and active Kitsune workspace/tab/pane variables.

The session recall helper has first-class `kitsune`, `herdr`, and `tmux` backends. Each backend uses its own environment and runtime context; Kitsune does not cross-populate `HERDR_*` variables for recall.

## Completed fork baseline

The completed Kitsune baseline includes:

- Building the `kitsune` binary.
- Showing Kitsune-facing help and version output from the command.
- Running normal sessions against Kitsune-owned config, socket, session, and log paths.
- Launching panes with Kitsune-owned environment variables.
- Opening native Kitsune session recall from the default keybinding.

Intentionally deferred work includes a source-wide Herdr-to-Kitsune internal rename, replacing all remaining release/update/documentation surfaces, removing Herdr-specific features, and moving recall in-process if that becomes product direction.
