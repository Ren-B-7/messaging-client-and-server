# =========================
# CONFIG
# =========================
RUST_DIR := rust
PY_DIR := python-gui
FRONTEND_DIR := frontend

# =========================
# DEFAULT
# =========================
.PHONY: all
all: format lint build

# =========================
# FORMAT
# =========================

.PHONY: format format-rust format-python format-frontend

format: format-rust format-python format-frontend

format-rust:
	@echo "Formatting Rust..."
	cd $(RUST_DIR) && cargo fmt

format-python:
	@echo "Formatting Python (black)..."
	cd $(PY_DIR) && uv run black .

format-frontend:
	@echo "Formatting frontend..."
	cd $(FRONTEND_DIR) && prettier --write "**/*.css"
	cd $(FRONTEND_DIR) && prettier --write "**/*.html"
	cd $(FRONTEND_DIR) && prettier --write "**/*.js"

format-makefile:
	@echo "Formatting Makefile..."
	mbake format Makefile

# =========================
# LINT
# =========================

.PHONY: lint lint-rust lint-python lint-frontend

lint: lint-rust lint-python lint-frontend

lint-rust:
	@echo "Linting Rust..."
	cd $(RUST_DIR) && cargo clippy --all-targets --all-features -- -D warnings

lint-python:
	@echo "Linting Python (ruff)..."
	cd $(PY_DIR) && uv run ruff check .

lint-frontend:
	@echo "Linting frontend..."
	cd $(FRONTEND_DIR) && stylelint --config ../.stylelintrc.yml "**/*.css"
	cd $(FRONTEND_DIR) && htmlhint --config ../.htmlhintrc"**/*.html"

lint-makefile:
	@echo "Linting Makefile..."
	mbake validate Makefile

# =========================
# BUILD
# =========================

.PHONY: build build-rust build-python build-frontend

build: build-rust build-python build-frontend

build-rust:
	@echo "Building Rust..."
	cd $(RUST_DIR) && cargo build --workspace

build-backend:
	@echo "Building server..."
	cd $(RUST_DIR) && cargo b -p server

build-shared:
	@echo "Building shared..."
	cd $(RUST_DIR) && cargo b -p shared

build-tui:
	@echo "Building tui..."
	cd $(RUST_DIR) && cargo b -p tui

build-python:
	@echo "Preparing Python..."
	cd $(PY_DIR) && uv sync

build-frontend:
	@echo "Frontend has no build step (static)"

# =========================
# RUN TARGETS
# =========================

.PHONY: backend tui python gui frontend

backend:
	@echo "Running Rust backend..."
	cd $(RUST_DIR)/server && cargo run --bin server ./config/config.toml

tui:
	@echo "Running Rust TUI..."
	cd $(RUST_DIR)/tui && cargo run --bin tui

python:
	@echo "Running Python main..."
	cd $(PY_DIR) && uv run python main.py

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
	cd $(PY_DIR) && uv run pytest

# =========================
# CLEAN
# =========================

.PHONY: clean clean-rust clean-python clean-frontend

clean: clean-rust clean-python clean-frontend

clean-rust:
	cd $(RUST_DIR) && cargo clean

clean-python:
	rm -rf $(PY_DIR)/.venv
	rm -rf $(PY_DIR)/__pycache__
	rm -rf $(PY_DIR)/.ruff_cache

clean-frontend:
	find $(FRONTEND_DIR) -name "*.cache" -type d -exec rm -rf {} +

# =========================
# EXTRA
# =========================

.PHONY: ci check doctor

ci: format lint test

check:
	cd $(RUST_DIR) && cargo check --workspace

doctor:
	@echo "Checking tools..."
	@command -v cargo >/dev/null || echo "cargo missing"
	@command -v uv >/dev/null || echo "uv missing"
	@command -v black >/dev/null || echo "black missing"
	@command -v ruff >/dev/null || echo "ruff missing"
	@command -v npx >/dev/null || echo "node/npx missing"
