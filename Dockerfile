# Build stage
FROM rust:1.84-slim as builder

WORKDIR /usr/src/punching-fist-operator

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

WORKDIR /usr/local/bin

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/punching-fist-operator/target/release/punching-fist-operator .

# Create non-root user
RUN useradd -m -u 1000 appuser
USER appuser

# Expose the WebSocket port
EXPOSE 8080

# Run the operator
CMD ["punching-fist-operator"] 