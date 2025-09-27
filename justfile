#!/usr/bin/env just

# Default recipe - show available commands
default:
    @just --list

# Build the gravity binary
build:
    cargo build --release

# Run all tests
test: test-unit

# Run unit tests
test-unit:
    cargo test --workspace

# Test all examples
test-examples: build
    @echo "Building and generating bindings for all examples..."
    cd examples && go generate ./...
    @echo "Running tests for all examples..."
    cd examples && go test ./...
    @echo "✅ All example tests passed!"

# Clean build artifacts
clean:
    cargo clean

# Run linting
lint:
    cargo clippy --workspace -- -D warnings
    cargo fmt --check

# Format code
fmt:
    cargo fmt

# Watch for changes and run tests
watch:
    cargo watch -x test -x "test --test ui"

# Initialize crates structure (for refactoring)
init-crates:
    mkdir -p crates/gravity-go/src
    mkdir -p crates/gravity-go/tests
    mkdir -p crates/gravity-codegen/src
    mkdir -p crates/gravity-codegen/tests
    mkdir -p crates/gravity-wit/src
    mkdir -p crates/gravity-wit/tests
    mkdir -p cmd/gravity/src
    mkdir -p cmd/gravity/tests/integration

# Run full CI pipeline
ci: fmt lint test test-examples
    @echo "✅ CI pipeline complete!"
