# Development commands for punching_fist_operator

# Run the operator server in development mode with live reload
# Usage: `just dev`
dev:
    cargo watch -x 'run -p punching-fist-operator'

# Run the operator server once without live reload
# Usage: `just run`
run:
    cargo run -p punching-fist-operator

# Build the project
# Usage: `just build`
build:
    cargo build

# Run tests
# Usage: `just test`
test:
    cargo test

# Check code without building
# Usage: `just check`
check:
    cargo check

# Clean build artifacts
# Usage: `just clean`
clean:
    cargo clean

# Show available commands
# Usage: `just help`
help:
    @just --list 