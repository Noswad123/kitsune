APP_NAME = kitsune
ALIASES = kitsune kit
INSTALL_DIR = ~/.local/bin

.PHONY: build check fmt test run install

build:
	@echo "🔨 Building $(ALIASES)..."
	cargo build $(foreach bin,$(ALIASES),--bin $(bin))

check:
	cargo check --tests

fmt:
	cargo fmt

test:
	cargo fmt --check
	cargo check --tests
	cargo test --bin $(APP_NAME) -- --test-threads=1

run: build
	env -u KITSUNE_SOCKET_PATH -u KITSUNE_CLIENT_SOCKET_PATH -u KITSUNE_ENV \
	    ./target/debug/$(APP_NAME)

install: build
	@echo "📦 Installing to $(INSTALL_DIR)"
	@mkdir -p $(INSTALL_DIR)
	@for bin in $(ALIASES); do \
		cp target/debug/$$bin $(INSTALL_DIR)/$$bin; \
	done
	@echo "✅ Installed. Run with: kit or kitsune"
