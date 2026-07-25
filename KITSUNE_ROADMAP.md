# Kitsune fork roadmap

This fork starts as a Kitsune-branded distribution of Herdr, then diverges in
small, intentional phases. The guiding rule is to keep Herdr's runtime behavior
working while Kitsune takes ownership of the user-facing product identity.

## Product goal

Kitsune should provide the same core functionality Herdr provides today, but
users should interact with it through `kitsune` instead of `herdr` during the
initial fork phase. The shorter `kit` command is reserved for later, once it is
safe to overwrite the existing local `kit` tool. After that baseline is stable,
Kitsune can add its existing session recall flow as a native keybinding and
gradually add or remove capabilities until it becomes its own product.

## Architecture principles

- Treat this as a product fork, not a one-time source dump.
- Keep upstream Herdr changes easy to inspect while the fork is still young.
- Prefer runtime/API boundaries over coupling new behavior to TUI-only state.
- Separate product identity from runtime/session state so Kitsune and Herdr can
  coexist on the same machine.
- Avoid broad internal renames until the user-facing identity and runtime
  separation are already stable.

## Phase 1: Kitsune-branded Herdr baseline

Goal: users can build and run the fork as `kitsune` while retaining Herdr's
current feature set.

Initial scope:

- Add a `kitsune` binary target.
- Update primary CLI help/version output to say Kitsune.
- Move default config, session, socket, and log locations away from Herdr names.
- Use `KITSUNE_*` environment variables for Kitsune-owned runtime controls.
- Keep deeper internal module/type names unchanged unless they are user-visible.

Out of scope for this phase:

- Full source-wide `herdr` to `kitsune` rename.
- Website/release channel replacement.
- New session recall behavior.
- Removing Herdr-specific features.
- Claiming the global `kit` command before the existing local `kit` tool is
  intentionally replaced.

Acceptance criteria:

- `cargo build --bin kitsune` succeeds.
- `kitsune --help` and `kitsune --version` show Kitsune-facing output.
- A normal Kitsune session uses Kitsune config/socket/log paths rather than
  clashing with an installed Herdr session.

## Phase 2: native Kitsune session recall

Goal: once a Kitsune session is started, a keybinding opens the Kitsune session
recall flow.

Design direction:

- Add a first-class action for session recall.
- Bind that action through the existing keybinding/config system.
- Reuse existing modal/screen patterns rather than inventing a one-off UI.
- Keep recall state client-side unless it becomes shared runtime/session fact.

Open decisions:

- Final default keybinding.
- Whether recall opens an in-process picker, launches an existing Kitsune
  command, or bridges to an existing Kitsune recall API.
- How recalled sessions map to Herdr/Kitsune named sessions.

## Phase 3: intentional divergence

Goal: make Kitsune its own product while preserving a working runtime.

Candidate work:

- Replace Herdr release/update channels with Kitsune channels.
- Replace onboarding, docs, assets, and package metadata.
- Add Kitsune-specific workflows and defaults.
- Remove features that do not fit Kitsune's product direction.
- Rename internal modules/types only when it reduces confusion more than it
  increases merge pain.

## Near-term working branch

Use `kitsune-integration` as the initial integration branch. Keep changes small
and validate after each phase so the fork remains runnable throughout the
transition.
