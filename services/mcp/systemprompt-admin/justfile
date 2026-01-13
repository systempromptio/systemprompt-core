# Admin MCP Server
#
# BUILD REQUIREMENT: DATABASE_URL must be set for sqlx compile-time checking
#
# Before building, run:
#   export DATABASE_URL=postgres://systemprompt:123@localhost:5432/systemprompt
#
# Or use: just build-with-db

default:
    @just --list

# Build with DATABASE_URL set (recommended)
build-with-db:
    DATABASE_URL=postgres://systemprompt:123@localhost:5432/systemprompt cargo build --release

# Check with DATABASE_URL set
check-with-db:
    DATABASE_URL=postgres://systemprompt:123@localhost:5432/systemprompt cargo check

# Build (requires DATABASE_URL to be exported)
build:
    cargo build --release

# Check compilation (requires DATABASE_URL to be exported)
check:
    cargo check

# Run the server
run:
    cargo run

# Run tests
test:
    cargo test

# Format code
fmt:
    cargo fmt

# Lint with clippy
lint:
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    cargo clean
