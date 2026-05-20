# List available recipes
default:
  @just --list

# Run cargo check
check:
    cargo check --all-targets --all-features

# Format code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# Run clippy lints
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests, or test for a single crate. Ex: just test OR just test terafile
test crate='':
    @if [ -z "{{crate}}" ]; then \
        cargo nextest run --workspace; \
    else \
        cargo nextest run -p "{{crate}}"; \
    fi

# Run tests in watch mode
test-watch:
    cargo watch -x "nextest run --all-features"

# Build the project
build:
    cargo build --all-features

# Build release binary
build-release:
    cargo build --release --all-features

# Generate and open documentation
docs:
    cargo doc --all-features --no-deps --document-private-items --open

# Clean build artifacts
clean:
    cargo clean

# Run all pre-commit checks
pre-commit: fmt-check clippy test check

# Run all CI checks
ci:
    nix flake check
