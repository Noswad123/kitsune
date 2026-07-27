# Kitsune docs

Kitsune started as a personal Herdr fork and is distributed as the `kitsune` command.

The hosted Herdr website, versioned website docs, hosted updater manifests, and plugin marketplace docs do not apply to this fork. Keep durable local documentation in repository docs and keep the bundled API schema in `docs/next/api/`.

## Runtime identity

Kitsune has its own runtime identity while retaining Herdr's core terminal-agent runtime.

- `kitsune` is the canonical command.
- Default config, session, socket, and log locations use Kitsune names rather than Herdr names.
- Runtime controls use `KITSUNE_*` environment variables, including pane identity and socket selection.
- Panes launched by Kitsune receive `KITSUNE_ENV`, `KITSUNE_BIN_PATH`, `KITSUNE_WORKSPACE_ID`, `KITSUNE_TAB_ID`, and `KITSUNE_PANE_ID`.
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

## Agent navigation

The session navigator is the shared surface for workspace, tab, pane, and agent selection.

- Action name: `goto`; default binding: `prefix+g`.
- Action name: `agent_selector`; default binding: `prefix+a`.
- `agent_selector` opens the same navigator directly in agents-only scope.
- `Tab` toggles between the full tree and agents-only scope.
- Search and status filters apply in both scopes.
- Agents-only rows show agent status and workspace/tab/pane context.
- `r` in agents-only scope reuses the pane rename modal and renames the selected pane label, not agent metadata.

## Integrations

Current supported integration targets are:

- `pi`
- `opencode`
- `claude`
- `codex`
- `copilot`
- `djinn`

Djinn is a built-in first-class agent kind. It participates in process detection, integration status, interactive startup through `kitsune agent start --kind djinn`, full lifecycle authority when compatible Djinn builds report pane state with source `kitsune:djinn` and agent label `djinn`, resume planning for reported Djinn session ids through `djinn agent chat --resume <id>`, and authority release when compatible Djinn builds report chat exit.

Kitsune intentionally does not fetch, compare, or install Djinn versions. Install or update the managed Jamal Arcana tool suite with `arcana update` to keep Kitsune and Djinn on compatible builds. Older or unmanaged Djinn panes continue to fall back to process detection plus the generic known-agent idle behavior when lifecycle reports are unavailable.

## Windows support

Kitsune no longer ships a pinned app-local Microsoft ConPTY runtime. Native Windows builds use the system ConPTY exports from `kernel32.dll`, so Windows support depends on the ConPTY implementation included with Windows 10 October 2018 or newer. Kitsune also keeps the portable-pty patch that avoids probing a bare `conpty.dll` through `PATH`, so it should not load another application's ConPTY DLL by accident.

If pinned ConPTY behavior is needed again, reimplement it as a Kitsune-owned packaging feature: pin the Microsoft package metadata, verify every bundled DLL/host binary by hash, reject reparse points and unexpected files, load by absolute path with constrained DLL search, document the license/notice files, and restore Windows CI coverage that validates both tamper rejection and the selected console host path.

## Completed fork baseline

The completed Kitsune baseline includes:

- Building the `kitsune` binary.
- Showing Kitsune-facing help and version output from the command.
- Running normal sessions against Kitsune-owned config, socket, session, and log paths.
- Launching panes with Kitsune-owned environment variables, including `KITSUNE_BIN_PATH`.
- Opening native Kitsune session recall from the default keybinding.
- Providing a full session navigator and an agents-only selector.
- Carrying only the currently supported integration targets listed above.
- Treating Djinn-native lifecycle reports as authoritative when they are reported through Kitsune's pane agent state API.
- Delegating Kitsune/Djinn install and update compatibility to Arcana instead of embedding version-resolution logic in Kitsune.
- Removing inherited hosted update, website, plugin marketplace, and remote auto-install flows from the active Kitsune surface.
- Using system ConPTY on Windows instead of shipping a pinned app-local ConPTY bundle.

Intentionally deferred work includes source-wide internal Herdr-to-Kitsune renames only where they reduce confusion, further product-surface pruning when specific inherited behavior proves unnecessary, Djinn-side reporting adapter rollout through Arcana-managed builds, and optional Djinn screen detection if stable invariant UI evidence emerges.
