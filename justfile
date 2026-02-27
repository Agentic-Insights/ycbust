# ycbust - YCB Dataset Downloader and Extractor
#
# Usage: just [command] [args...]
# Run `just` to see all available commands

set shell := ["bash", "-uc"]
set quiet := false

# Default command - show help with formatting
default:
    @echo "ycbust - Developer Commands"
    @echo ""
    @just --list --unsorted

# ============================================================================
# Build Commands
# ============================================================================

# Build the library and binary (debug mode)
[group('build')]
build:
    cargo build

# Build with S3 streaming support
[group('build')]
build-s3:
    cargo build --features s3

# Build in release mode (optimized)
[group('build')]
build-release:
    cargo build --release

# Build release with S3 support
[group('build')]
build-release-s3:
    cargo build --release --features s3

# Check code without building
[group('build')]
check:
    cargo check

# Check code with S3 feature
[group('build')]
check-s3:
    cargo check --features s3

# Format code with rustfmt
[group('build')]
fmt:
    cargo fmt

# Run clippy linter with strict warnings
[group('build')]
lint:
    cargo clippy -- -D warnings

# Run clippy with S3 feature
[group('build')]
lint-s3:
    cargo clippy --features s3 -- -D warnings

# ============================================================================
# Test Commands
# ============================================================================

# Run all tests with quiet output
[group('test')]
test:
    cargo test --quiet

# Run all tests with S3 feature
[group('test')]
test-s3:
    cargo test --features s3 --quiet

# Run all tests with output
[group('test')]
test-verbose:
    cargo test -- --nocapture --test-threads=1

# Run only library tests
[group('test')]
test-lib:
    cargo test --lib --quiet

# Run integration tests only
[group('test')]
test-integration:
    cargo test --test '*' --quiet

# ============================================================================
# Run Commands
# ============================================================================

# Run the CLI tool with arguments: just run [args...]
[group('run')]
run +args:
    cargo run --quiet -- {{args}}

# Run the CLI help
[group('run')]
run-help:
    cargo run --quiet -- --help

# Run the CLI version
[group('run')]
run-version:
    cargo run --quiet -- --version

# Run demo: download representative subset to /tmp/ycb-test
[group('run')]
run-demo:
    @echo "📦 Downloading YCB representative subset..."
    cargo run --quiet -- --subset representative --output-dir /tmp/ycb-test --overwrite

# ============================================================================
# Development & Maintenance
# ============================================================================

# Watch for changes and rebuild (requires cargo-watch)
[group('dev')]
watch:
    cargo watch -x check -x fmt -x lint

# Generate and open API documentation
[group('dev')]
doc:
    cargo doc --no-deps --open

# Run full CI pipeline: format, lint, test
[group('dev')]
ci: fmt lint test
    @echo "✅ CI check passed!"

# Run full CI pipeline with S3 feature
[group('dev')]
ci-s3: fmt lint-s3 test-s3
    @echo "✅ CI check (with S3) passed!"

# Pre-commit check: format, lint, check, test
[group('dev')]
pre-commit: fmt lint check test
    @echo "✅ Ready to commit!"

# Clean build artifacts
[group('dev')]
clean:
    cargo clean

# Show detailed help
[group('dev')]
help:
    @echo "ycbust - YCB Dataset Downloader & Extractor"
    @echo ""
    @echo "Quick Start:"
    @echo "  just build           Build the project"
    @echo "  just test            Run all tests"
    @echo "  just run-demo        Download sample data"
    @echo ""
    @echo "Full command list: just --list"

# ============================================================================
# Utility Commands (hidden)
# ============================================================================

# Update dependencies
[group('util')]
update:
    cargo update

# Check for security vulnerabilities (requires cargo-audit)
[group('util')]
audit:
    cargo audit

# Show dependency tree
[group('util')]
deps:
    cargo tree
