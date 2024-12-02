# Base image for planner and builder
FROM rust:1.81-slim-bullseye AS base
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get clean && apt-get update && \
    apt-get install -y \
    build-essential \
    pkg-config \
    llvm-dev \
    libclang-dev \
    clang \
    cmake \
    zlib1g-dev \
    libssl-dev \
    libsasl2-dev \
    python3

# Planner stage - prepare recipe.json
FROM base AS planner
WORKDIR /plan
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Chef stage - cache dependencies
FROM base AS chef
WORKDIR /build
RUN cargo install cargo-chef
COPY --from=planner /plan/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Builder stage - build the application
FROM base AS builder
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_BUILD_JOBS=1
WORKDIR /buildspace
# Copy over the cached dependencies
COPY --from=chef /build/target target
COPY --from=chef /usr/local/cargo /usr/local/cargo
# Copy the source code
COPY . .
# Build the application
RUN cargo build --release --workspace

# Processor production image
FROM debian:bullseye-slim AS processor
WORKDIR /usr/local/bin
COPY --from=builder /buildspace/target/release/open-payments-processor .
CMD ["./open-payments-processor"]

# API production image  
FROM debian:bullseye-slim AS processor-api
WORKDIR /usr/local/bin
COPY --from=builder /buildspace/target/release/open-payments-processor-api .
CMD ["./open-payments-processor-api"]
