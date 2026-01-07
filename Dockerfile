# Build stage
FROM rust:1.82-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev clang libc-dev

WORKDIR /app

# Copy only Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --locked

# Copy source and build
COPY src ./src
RUN cargo build --release --locked

# Runtime stage
FROM alpine:3.19 AS runtime

# Install runtime dependencies
RUN apk add --no-cache openssl

# Create non-root user
RUN addgroup -g 1000 app && adduser -u 1000 -G app -s /bin/sh -D app

# Copy binary from builder
COPY --from=builder /app/target/release/bte /usr/local/bin/bte

# Use non-root user
USER app

ENTRYPOINT ["bte"]
