# Build stage
FROM rust:1.88-alpine AS builder

WORKDIR /app

# Install build dependencies including mold linker
RUN apk add --no-cache \
    musl-dev \
    protobuf-dev \
    protoc \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    clang \
    mold

# Copy manifests and cargo config
COPY Cargo.toml Cargo.lock* ./
COPY .cargo .cargo

# Create dummy main.rs for dependency caching
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Build dependencies only
RUN cargo build --release && rm -rf src

# Copy source code
COPY . .

# Touch main.rs to force rebuild
RUN touch src/main.rs

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:3.21

WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libssl3 \
    libgcc \
    tzdata

# Copy binary from builder
COPY --from=builder /app/target/release/timecard-backend /app/timecard-backend

# Set timezone to Japan
ENV TZ=Asia/Tokyo

# Expose gRPC port
EXPOSE 50051

# Run the application
CMD ["./timecard-backend"]
