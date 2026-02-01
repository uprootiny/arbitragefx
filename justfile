# arbitragefx build/test/validate pipeline
# Uses CARGO_TARGET_DIR to avoid cross-device link errors

export CARGO_TARGET_DIR := "/tmp/arbitragefx-target"

# Default recipe: full pipeline
default: check test

# Format code
fmt:
    cargo fmt

# Check formatting
fmt-check:
    cargo fmt -- --check

# Lint with clippy
lint:
    cargo clippy --all-targets -- -D warnings

# Type check without building
check:
    cargo check --all-targets

# Run all tests
test:
    cargo test --lib
    cargo test --bins

# Run tests with output
test-verbose:
    cargo test --lib -- --nocapture

# Run specific test
test-one NAME:
    cargo test {{NAME}} -- --nocapture

# Build debug
build:
    cargo build

# Build release
build-release:
    cargo build --release

# Run backtest
backtest FILE:
    cargo run --bin backtest -- {{FILE}}

# Run engine loop (live mode)
live SYMBOL="BTCUSDT":
    SYMBOL={{SYMBOL}} cargo run --bin engine_loop

# Run trials (parameter sweep)
trials:
    cargo run --bin trials

# Run path alignment check
path-check:
    cargo run --bin path_check

# Full validation pipeline
validate: fmt-check lint test
    @echo "All checks passed"

# Clean build artifacts
clean:
    cargo clean
    rm -rf /tmp/arbitragefx-target

# Watch for changes and test
watch:
    cargo watch -x "test --lib"

# Generate test coverage (requires cargo-tarpaulin)
coverage:
    cargo tarpaulin --out Html --output-dir coverage/

# Security audit (requires cargo-audit)
audit:
    cargo audit

# Count lines of code
loc:
    @echo "=== Source ==="
    @find src -name "*.rs" | xargs wc -l | tail -1
    @echo "=== Tests ==="
    @grep -r "#\[test\]" src | wc -l
    @echo " test functions"

# Show module structure
modules:
    @echo "=== Engine modules ==="
    @ls -1 src/engine/*.rs | sed 's/src\/engine\//  /' | sed 's/\.rs//'
    @echo ""
    @echo "=== Binaries ==="
    @ls -1 src/bin/*.rs | sed 's/src\/bin\//  /' | sed 's/\.rs//'

# Pre-commit hook (run before committing)
pre-commit: fmt validate
    @echo "Ready to commit"

# CI pipeline (what CI should run)
ci: fmt-check lint test
    @echo "CI passed"
