# =========================
# CONFIG
# =========================
RUST_DIR := rust
PY_DIR := python-gui
FRONTEND_DIR := frontend

# =========================
# TOOLS SETUP
# =========================
PYTHON_TOOLS = mbake
RUST_TOOLS = taplo typos
FRONTEND_TOOLS = prettier

# Target to install all necessary tools
setup:
	@echo "Setting up development tools..."
	# Install Python tools using uv (assumes uv is installed and in PATH)
	cd $(PY_DIR) && uv pip install --system $(PYTHON_TOOLS)
	# Install Rust tools globally
	cargo install $(RUST_TOOLS)
	# Install Node.js tools globally
	npm install -g $(FRONTEND_TOOLS)
	@echo "Tools setup complete."

# =========================
# DEFAULT
# =========================
.PHONY: all
all: setup format lint build

# =========================
# FORMAT
# =========================

.PHONY: format format-rust format-python format-frontend format-makefile format-toml

format: format-rust format-python format-frontend format-makefile format-toml

format-rust:
	@echo "Formatting Rust..."
	cd $(RUST_DIR) && cargo fmt

format-python:
	@echo "Formatting Python (ruff)..."
	# Use ruff for formatting, replace black
	cd $(PY_DIR) && uv run ruff format .

format-frontend:
	@echo "Formatting frontend..."
	cd $(FRONTEND_DIR) && [ -d node_modules ] || npm install
	cd $(FRONTEND_DIR) && npm run format

format-makefile:
	@echo "Formatting Makefile..."
	mbake format Makefile

format-toml:
	@echo "Formatting Toml..."
	taplo format

# =========================
# LINT
# =========================

.PHONY: lint lint-rust lint-python lint-frontend lint-makefile lint-typos

lint: lint-rust lint-python lint-frontend lint-makefile lint-typos

lint-rust:
	@echo "Linting Rust..."
	cd $(RUST_DIR) && cargo clippy --all-targets --all-features -- -D warnings

lint-python:
	@echo "Linting Python (ruff)..."
	cd $(PY_DIR) && uv run ruff check .

lint-frontend:
	@echo "Linting frontend..."
	cd $(FRONTEND_DIR) && [ -d node_modules ] || npm install
	# Lint CSS files (excluding minified/node_modules)
	cd $(FRONTEND_DIR) && find . -type f -name "*.css" \
		-not -path "*/min/*" \
		-not -path "*/node_modules/*" \
		| xargs ./node_modules/.bin/stylelint
	# Lint HTML files (excluding minified/node_modules)
	cd $(FRONTEND_DIR) && find . -type f -name "*.html" \
		-not -path "*/min/*" \
		-not -path "*/node_modules/*" \
		| xargs ./node_modules/.bin/htmlhint

lint-makefile:
	@echo "Linting Makefile..."
	mbake validate Makefile

lint-typos:
	@echo "Checking for typos..."
	typos -j4 --exclude node_modules --exclude target rust/ python-gui/ frontend/static/ frontend/non-static/ .github/

# =========================
# BUILD
# =========================

.PHONY: build build-rust build-python build-frontend

build: build-rust build-python build-frontend

# Use cargo build --workspace for consistency
build-rust:
	@echo "Building Rust workspace..."
	cd $(RUST_DIR) && cargo build --workspace

# build-backend, build-shared, build-tui are redundant with --workspace, removed.

build-python:
	@echo "Preparing Python..."
	cd $(PY_DIR) && uv sync

build-frontend:
	@echo "Building frontend..."
	cd $(FRONTEND_DIR) && [ -d node_modules ] || npm install
	# Build JS minification
	cd $(FRONTEND_DIR) && for file in $$(find static/js/full -name '*.js'); do \
		out=$$(echo $$file | sed 's|static/js/full/|static/js/min/|'); \
		mkdir -p $$(dirname $$out); \
		./node_modules/.bin/terser $$file -o $$out --compress; \
	done
	cd $(FRONTEND_DIR) && for file in $$(find non-static/full -name '*.js'); do \
		out=$$(echo $$file | sed 's|non-static/full/|non-static/min/|'); \
		mkdir -p $$(dirname $$out); \
		./node_modules/.bin/terser $$file -o $$out --compress; \
	done
	# Build CSS minification
	cd $(FRONTEND_DIR) && for file in $$(find static/css/full -name '*.css'); do \
		out=$$(echo $$file | sed 's|static/css/full/|static/css/min/|'); \
		mkdir -p $$(dirname $$out); \
		./node_modules/.bin/csso $$file -o $$out; \
	done

# =========================
# RUN TARGETS
# =========================

.PHONY: backend tui gui frontend

backend:
	@echo "Running Rust backend..."
	cd $(RUST_DIR)/server && cargo run --bin server ./config/config.toml

tui:
	@echo "Running Rust TUI..."
	cd $(RUST_DIR)/tui && cargo run --bin tui

gui:
	@echo "Running Python GUI..."
	cd $(PY_DIR) && uv run python main.py

frontend:
	@echo "Serving frontend..."
	cd $(FRONTEND_DIR) && python -m http.server 8080

# =========================
# DEV TOOLS
# =========================

.PHONY: dev-rust dev-python dev

dev-rust:
	@echo "Starting bacon (Rust)..."
	cd $(RUST_DIR) && bacon

dev-python:
	@echo "Python lint watcher..."
	cd $(PY_DIR) && uv run ruff check . --watch

dev: dev-rust

# =========================
# TESTS
# =========================

.PHONY: test test-rust test-python

test: test-rust test-python

test-rust:
	cd $(RUST_DIR) && cargo test --workspace

test-python:
	@echo "No Python tests are currently implemented. Skipping."
	# cd $(PY_DIR) && uv run pytest # Original line, commented out

# =========================
# CLEAN
# =========================

.PHONY: clean clean-rust clean-python clean-frontend

clean: clean-rust clean-python clean-frontend

clean-rust:
	cd $(RUST_DIR) && cargo clean

clean-python:
	@echo "Cleaning Python environment..."
	rm -rf $(PY_DIR)/.venv
	rm -rf $(PY_DIR)/__pycache__
	rm -rf $(PY_DIR)/.ruff_cache
	# Re-added the --clean option for the main.py script, assuming it handles cleanup.
	cd $(PY_DIR) && uv run python main.py --clean

clean-frontend:
	find $(FRONTEND_DIR) -name "*.cache" -type d -exec rm -rf {} +

# =========================
# EXTRA
# =========================

.PHONY: ci check doctor

# Ensure setup is part of CI if it's intended for initial setup before lint/test
ci: setup format lint test

check:
	cd $(RUST_DIR) && cargo check --workspace

doctor:
	@echo "Checking tools..."
	@command -v cargo >/dev/null || echo "cargo missing"
	@command -v uv >/dev/null || echo "uv missing"
	@command -v ruff >/dev/null || echo "ruff missing"
	@command -v mbake >/dev/null || echo "mbake missing"
	@command -v taplo >/dev/null || echo "taplo missing"
	@command -v typos >/dev/null || echo "typos missing"
	@command -v stylelint >/dev/null || echo "stylelint missing"
	@command -v htmlhint >/dev/null || echo "htmlhint missing"
	@command -v prettier >/dev/null || echo "prettier missing"
