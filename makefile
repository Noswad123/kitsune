APP_NAME = kitsune
INSTALL_DIR = ~/.local/bin

.PHONY: build check fmt test run install

build:
	@echo "🔨 Building $(APP_NAME)..."
	cargo build --bin $(APP_NAME)

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
	@echo "📦 Installing to $(INSTALL_DIR)/$(APP_NAME)"
	@mkdir -p $(INSTALL_DIR)
	cp target/debug/$(APP_NAME) $(INSTALL_DIR)/$(APP_NAME)
	@echo "✅ Installed. Run with: $(APP_NAME)"
