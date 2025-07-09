# Stage 1: Builder
FROM rust:1.78-slim-bullseye as builder
# Using a specific version like 1.78 (or latest stable) is good practice.
# -slim-bullseye is a good balance of size and compatibility.

WORKDIR /usr/src/app

# Install build dependencies if any (e.g., for certain crates that need system libs like openssl)
# RUN apt-get update && apt-get install -y libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*
# For -slim images, openssl might already be there or linked correctly. If not, use full debian image or install.

# Cache dependencies. Only re-run if Cargo.toml or Cargo.lock changes.
COPY Cargo.toml Cargo.lock ./
# Create a dummy main.rs or lib.rs to build only dependencies
RUN mkdir src && echo "fn main() {println!(\"dummy build for deps\")}" > src/main.rs
RUN cargo build --release --locked
RUN rm -rf src/main.rs # Remove dummy

# Copy actual source code
COPY src ./src

# Build the application
# Ensure this picks up the cached dependencies from the previous step.
RUN cargo build --release --locked

# Stage 2: Runner
FROM debian:bullseye-slim
# Using a minimal base image like debian-slim or alpine. Distroless is even smaller.
# Alpine might require musl libc if not handled during compilation (e.g. with target x86_64-unknown-linux-musl)

ARG APP_USER=appuser
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} ${APP_USER}

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/api .

# Copy .env.example, though actual .env or env vars should be provided at runtime
# COPY .env.example .env.example
# It's better not to copy .env files into the image. Environment variables should be injected.

# Ensure the binary is executable
RUN chmod +x ./api

# Set the user
USER ${APP_USER}:${APP_USER}

# Expose the port the app runs on (make sure this matches SERVER_PORT in .env or default in main.rs)
EXPOSE 3001
# The actual port can be configured via environment variable `SERVER_PORT`

# Set the entrypoint
CMD ["./api"]
