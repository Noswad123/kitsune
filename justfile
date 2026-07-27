# kitsune task runner

app_name := "kitsune"

# Format code in place
fmt:
    cargo fmt

# Run cargo's test-target type checks, matching the old makefile `check`
check-tests:
    cargo check --tests

# Build a signed debug binary for local development, matching the old makefile `build`
dev-build:
    @echo "🔨 Building {{app_name}}..."
    cargo build --bin {{app_name}}
    scripts/sign_dev_binary.sh target/debug/{{app_name}}

# Run the debug binary with inherited Kitsune socket/runtime env cleared
run: dev-build
    env -u KITSUNE_SOCKET_PATH -u KITSUNE_CLIENT_SOCKET_PATH -u KITSUNE_ENV \
        ./target/debug/{{app_name}}

# Install the signed debug binary to INSTALL_DIR, defaulting to ~/.local/bin
install: dev-build
    @install_dir="${INSTALL_DIR:-$HOME/.local/bin}"; \
    echo "📦 Installing to $install_dir"; \
    mkdir -p "$install_dir"; \
    cp target/debug/{{app_name}} "$install_dir/{{app_name}}"; \
    scripts/sign_dev_binary.sh "$install_dir/{{app_name}}"; \
    echo "✅ Installed. Run with: {{app_name}}"

# Run the focused bin test loop from the old makefile `test`
test-bin:
    cargo fmt --check
    cargo check --tests
    cargo test --bin {{app_name}} -- --test-threads=1

# Start a dev live handoff. Optional env: BIN=/path/to/kitsune HANDOFF_ARGS='--dry-run'
handoff:
    @set -eu; \
    if [ -n "${BIN:-}" ]; then \
        set -- --bin "$BIN"; \
    else \
        set --; \
    fi; \
    if [ -n "${HANDOFF_ARGS:-}" ]; then \
        set -- "$@" $HANDOFF_ARGS; \
    fi; \
    scripts/live_handoff_dev.sh "$@"

# Run tests
test:
    cargo nextest run --locked --status-level fail --final-status-level fail --failure-output final --success-output never
    python3 -m unittest scripts.test_agent_detection_manifest_check scripts.test_changelog scripts.test_hermes_integration_asset scripts.test_package_windows_conpty scripts.test_vendor_libghostty_vt scripts.test_vendor_portable_pty
    just integration-assets-test
    just plugin-marketplace-test

# Run one nextest filter, e.g. `just test-one codex_stale_working`
test-one filter:
    cargo nextest run --locked "{{filter}}" --status-level fail --final-status-level fail --failure-output final --success-output never

# Run fast local lint checks
lint:
    cargo fmt --check
    cargo clippy --all-targets --locked -- -D warnings

# Run PR CI checks
ci filter='all()': lint
    cargo nextest run --locked -E "{{filter}}" --status-level fail --final-status-level slow --failure-output final --success-output never
    just integration-assets-test
    just plugin-marketplace-test

# Run Windows target lint from Unix/macOS to catch cfg(windows) compile and clippy failures before CI
windows-lint:
    rustup target add x86_64-pc-windows-msvc
    LIBGHOSTTY_VT_SIMD=false cargo clippy --bin kitsune --locked --target x86_64-pc-windows-msvc -- -D warnings

# Check formatting + run unit tests + Windows target lint + maintenance script tests
check: ci windows-lint
    python3 -m unittest scripts.test_agent_detection_manifest_check scripts.test_changelog scripts.test_hermes_integration_asset scripts.test_package_windows_conpty scripts.test_vendor_libghostty_vt scripts.test_vendor_portable_pty

# Install repo-local git hooks
install-hooks:
    git config core.hooksPath .githooks
    chmod +x .githooks/pre-commit
    chmod +x .githooks/commit-msg
    @echo "installed git hooks from .githooks"

# Build release binary
build:
    cargo build --release --locked

# Test bundled agent integration assets
integration-assets-test:
    bun test src/integration/assets/kitsune-agent-state.test.ts
    bun test src/integration/assets/opencode/kitsune-agent-state.test.ts

# Run plugin marketplace Worker tests
plugin-marketplace-test:
    cd workers/plugin-marketplace && bun test

# Build the vendored libghostty-vt source dist
build-libghostty-vt:
    scripts/build_vendored_libghostty_vt.sh

# Prepare the release commit without tagging or pushing (usage: just release-prepare 0.1.1)
release-prepare version:
    @printf '%s\n' '{{version}}' | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { \
        echo "error: version must look like 0.6.6 without a v prefix"; \
        exit 1; \
    }
    @if [ -n "$(git status --porcelain)" ]; then \
        echo "error: commit your changes first"; \
        exit 1; \
    fi
    @git fetch origin master --tags
    @if git rev-parse "v{{version}}" >/dev/null 2>&1; then \
        echo "error: tag v{{version}} already exists"; \
        exit 1; \
    fi
    python3 scripts/changelog.py prepare --version {{version}}
    cp CHANGELOG.md docs/next/CHANGELOG.md
    sed -i.bak 's/^version = ".*"/version = "{{version}}"/' Cargo.toml && rm -f Cargo.toml.bak
    cargo update -p kitsune --offline
    just check
    git add CHANGELOG.md docs/next/CHANGELOG.md Cargo.toml Cargo.lock
    git diff --cached --quiet || git commit -m "release: v{{version}}"
    @echo "v{{version}} release commit prepared. Review it, then run: just release-publish {{version}}"

# Tag and push an already-prepared release commit (usage: just release-publish 0.1.1)
release-publish version:
    @printf '%s\n' '{{version}}' | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { \
        echo "error: version must look like 0.6.6 without a v prefix"; \
        exit 1; \
    }
    @if [ -n "$(git status --porcelain)" ]; then \
        echo "error: working tree must be clean before publishing"; \
        exit 1; \
    fi
    @branch="$(git branch --show-current)"; \
    if [ "$branch" != "master" ]; then \
        echo "error: release-publish must run from master, got $branch"; \
        exit 1; \
    fi
    @git fetch origin master --tags
    @if git rev-parse "v{{version}}" >/dev/null 2>&1; then \
        echo "error: tag v{{version}} already exists"; \
        exit 1; \
    fi
    @cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"; \
    if [ "$cargo_version" != "{{version}}" ]; then \
        echo "error: Cargo.toml version $cargo_version does not match {{version}}"; \
        exit 1; \
    fi
    python3 scripts/changelog.py extract --version {{version}} --output /tmp/kitsune-release-notes-check.md
    rm -f /tmp/kitsune-release-notes-check.md
    @local_head="$(git rev-parse HEAD)"; \
    remote_head="$(git rev-parse origin/master)"; \
    if ! git merge-base --is-ancestor "$remote_head" "$local_head"; then \
        echo "error: origin/master is not an ancestor of HEAD; pull or rebase before publishing"; \
        exit 1; \
    fi; \
    if [ "$local_head" != "$remote_head" ]; then \
        echo "pushing release commit to origin/master"; \
        git push origin HEAD:master; \
    fi
    git tag -a v{{version}} -m "v{{version}}"
    git push origin v{{version}}
    @echo "v{{version}} released — GitHub Actions building binaries"

# Prepare, verify, tag, push, and trigger the GitHub Release workflow (usage: just release 0.1.1)
release version:
    just release-prepare {{version}}
    just release-publish {{version}}

# Print default config
default-config:
    cargo run --release --locked -- --default-config
