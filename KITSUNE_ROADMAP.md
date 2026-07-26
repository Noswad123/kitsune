# Kitsune fork roadmap

This fork started as a Kitsune-branded distribution of Herdr. Phase 1 is now
complete: Kitsune has its own runtime identity, config/socket/log locations,
environment variables, and CLI entry points. The next work is to replace the
remaining compatibility shims and deliberately decide where Kitsune should
diverge from Herdr.

## Product goal

Kitsune should provide Herdr's core terminal-agent runtime while presenting a
Kitsune-owned product surface. Users can interact through the canonical
`kitsune` command or the short `kit` command. Both entry points run the same
runtime and target Kitsune-owned state rather than Herdr state.

The remaining product goal is no longer basic rebranding; it is reducing
compatibility shims, making session recall first-class, and choosing the
defaults and workflows that make Kitsune its own product.

## Architecture principles

- Treat this as a product fork, not a one-time source dump.
- Keep upstream Herdr changes easy to inspect while the fork is still young.
- Prefer runtime/API boundaries over coupling new behavior to TUI-only state.
- Separate product identity from runtime/session state so Kitsune and Herdr can
  coexist on the same machine.
- Avoid broad internal renames until the user-facing identity and runtime
  separation are already stable.

## Phase 1: Kitsune-branded Herdr baseline — complete

Goal: users can build and run the fork as Kitsune while retaining Herdr's
current feature set.

Completed scope:

- Added the canonical `kitsune` binary target and the short `kit` entry point.
- Updated primary CLI help/version output to say Kitsune.
- Moved default config, session, socket, and log locations away from Herdr
  names.
- Uses `KITSUNE_*` environment variables for Kitsune-owned runtime controls,
  including pane identity and socket selection.
- Kept deeper internal module/type names unchanged unless they are
  user-visible, preserving mergeability while the fork is still young.
- Added config inspection/reload support needed for the fork workflow, including
  `config path` and socket-aware config reload.

Still intentionally out of scope after Phase 1:

- Full source-wide `herdr` to `kitsune` rename.
- Removing all Herdr compatibility variables/shims at once.
- Replacing every website/release/update surface.
- Removing Herdr-specific features.

Acceptance criteria met:

- `cargo build --bin kitsune` and `cargo build --bin kit` succeed.
- `kitsune --help`, `kit --help`, `kitsune --version`, and `kit --version` show
  Kitsune-facing output.
- A normal Kitsune session uses Kitsune config/socket/log paths rather than
  clashing with an installed Herdr session.
- Panes launched by Kitsune receive `KITSUNE_ENV`, `KITSUNE_WORKSPACE_ID`,
  `KITSUNE_TAB_ID`, and `KITSUNE_PANE_ID`.

## Phase 2: native Kitsune session recall — complete

Goal: once a Kitsune session is started, a keybinding opens the Kitsune session
recall flow.

Completed scope:

- Added a first-class action for session recall: `session_recall`.
- Bound that action through the existing keybinding/config system. The default
  remains `prefix+shift+s`.
- Reused existing modal/screen patterns rather than inventing a one-off UI.
  (Done initially by launching the existing Kitsune recall TUI in a popup.)
- Provided Kitsune-owned environment and socket context when launching the
  recall helper.
- Made the recall helper launch path Kitsune-owned. It is invoked with
  `--backend kitsune` by default and receives `KITSUNE_ENV`,
  `KITSUNE_BIN_PATH`, `KITSUNE_SOCKET_PATH`, `KITSUNE_CLIENT_SOCKET_PATH`, and
  active Kitsune workspace/tab/pane context.
- Added a first-class Kitsune backend to the external session recall helper so
  Kitsune can diverge from Herdr without relying on Herdr backend names.
- Removed the Kitsune-side `HERDR_*` compatibility shim. Herdr, tmux, and
  Kitsune are selected as normal helper backends rather than by cross-populating
  each other's environment variables.

Completion decisions:

- Keep the external recall helper for this phase rather than building an
  in-process picker immediately.
- Treat recalled session selection as client-side unless it becomes a shared
  runtime/session fact later.
- Prefer neutral server/API concepts over TUI-only coupling if recall grows into
  shared runtime behavior.

Deferred to Phase 3:

- Revisit whether `prefix+shift+s` remains the long-term binding.
- Replace the external helper with an in-process picker/native Kitsune recall
  API if that becomes product direction.

## Phase 3: intentional divergence — not started

Goal: make Kitsune its own product while preserving a working runtime.

Candidate work:

- Finish replacing Herdr release/update surfaces with Kitsune-owned channels.
- Replace or remove remaining Herdr-branded docs, assets, integration text, and
  package metadata.
- Add Kitsune-specific workflows and defaults.
- Remove features that do not fit Kitsune's product direction.
- Rename internal modules/types only when it reduces confusion more than it
  increases merge pain.
- Keep `kit` and `kitsune` behavior aligned so editor integrations, shell
  aliases, and remote workflows can use either entry point predictably.

## Near-term working branch

Use `kitsune-integration` as the integration branch until the first Kitsune
release line is stable. Keep changes small and validate after each phase so the
fork remains runnable throughout the transition.
