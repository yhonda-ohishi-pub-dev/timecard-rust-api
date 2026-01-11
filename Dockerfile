# Build stage
FROM rust:1.88-alpine AS builder

WORKDIR /app

# Build arguments for version info
ARG GIT_COMMIT=unknown
ARG GIT_COMMIT_SHORT=unknown
ARG BUILD_DATE=unknown

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

# Set build-time environment variables for Rust
ENV GIT_COMMIT=${GIT_COMMIT}
ENV GIT_COMMIT_SHORT=${GIT_COMMIT_SHORT}
ENV BUILD_DATE=${BUILD_DATE}

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:3.21

WORKDIR /app

# Build arguments (need to re-declare in runtime stage)
ARG GIT_COMMIT=unknown
ARG GIT_COMMIT_SHORT=unknown
ARG BUILD_DATE=unknown

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

# Set version info as runtime environment variables
ENV GIT_COMMIT=${GIT_COMMIT}
ENV GIT_COMMIT_SHORT=${GIT_COMMIT_SHORT}
ENV BUILD_DATE=${BUILD_DATE}

# Expose gRPC port
EXPOSE 50051

# Run the application
CMD ["./timecard-backend"]
