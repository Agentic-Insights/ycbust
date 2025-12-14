# ycbust justfile - Developer commands
#
# Usage: just <command> [args...]
# Run `just` or `just help` to see all available commands

# Default command - show help
default:
    @just --list

# ============================================================================
# Build Commands
# ============================================================================

# Build the library and binary (debug)
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Check code without building
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# ============================================================================
# Test Commands
# ============================================================================

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run only library tests
test-lib:
    cargo test --lib

# ============================================================================
# Run Commands
# ============================================================================

# Run the CLI tool
# Usage: just run [args...]
run +args:
    cargo run -- {{args}}

# Run the CLI help
run-help:
    cargo run -- --help

# Run the CLI to download representative subset to /tmp/ycb-test
run-demo:
    cargo run -- --subset representative --output-dir /tmp/ycb-test --overwrite

# ============================================================================
# CI/CD Commands
# ============================================================================

# Run full CI check (format, lint, test)
ci: fmt lint test

# Pre-commit check
pre-commit: fmt lint check test

# ============================================================================
# Development Commands
# ============================================================================

# Watch for changes and rebuild
watch:
    cargo watch -x check

# Generate documentation
doc:
    cargo doc --open

# Clean build artifacts
clean:
    cargo clean

# ============================================================================
# Help
# ============================================================================

# Show detailed help
help:
    @echo "ycbust - YCB Dataset Downloader and Extractor"
    @echo ""
    @echo "QUICK START:"
    @echo "  just build              # Build the project"
    @echo "  just test               # Run all tests"
    @echo "  just run-demo           # Download sample data to /tmp/ycb-test"
    @echo ""
    @echo "Run 'just --list' to see all available commands."
