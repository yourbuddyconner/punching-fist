# Build stage
FROM rust:1.84-slim as builder

WORKDIR /usr/src/punching-fist-operator

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build the application with cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/punching-fist-operator/target \
    cargo build --release && \
    cp target/release/punching-fist-operator /tmp/punching-fist-operator

# Runtime stage - Use the same base as the rust image for GLIBC compatibility
FROM debian:bookworm-slim

WORKDIR /usr/local/bin

# Install runtime dependencies including Docker CLI
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 curl gnupg lsb-release && \
    # Add Docker's official GPG key
    install -m 0755 -d /etc/apt/keyrings && \
    curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg && \
    chmod a+r /etc/apt/keyrings/docker.gpg && \
    # Add the repository to Apt sources
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null && \
    # Install Docker CLI
    apt-get update && \
    apt-get install -y docker-ce-cli && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /tmp/punching-fist-operator .

# Copy static files for the UI
RUN mkdir -p /usr/local/share/punching-fist/static
COPY --from=builder /usr/src/punching-fist-operator/crates/operator/static /usr/local/share/punching-fist/static

# Create non-root user and add to docker group
RUN groupadd -g 999 docker && \
    useradd -m -u 1000 -G docker appuser && \
    chown -R appuser:appuser /usr/local/share/punching-fist
USER appuser

# Set environment variables
ENV STATIC_FILE_PATH=/usr/local/share/punching-fist/static

# Expose the WebSocket port
EXPOSE 8080

# Run the operator
CMD ["punching-fist-operator"] 