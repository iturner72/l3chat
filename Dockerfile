FROM debian:bookworm as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    wget \
    git \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Install specific nightly Rust version
ENV RUST_VERSION=nightly
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain $RUST_VERSION
ENV PATH="/root/.cargo/bin:${PATH}"

# Verify the Rust version
RUN rustc --version && cargo --version
RUN rustup target add wasm32-unknown-unknown

# Install cargo-binstall
RUN wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz \
    && tar -xvf cargo-binstall-x86_64-unknown-linux-musl.tgz \
    && cp cargo-binstall /root/.cargo/bin

# Install wasm-bindgen-cli with specific version
RUN cargo install wasm-bindgen-cli --version 0.2.100

# Install cargo-leptos
RUN cargo binstall cargo-leptos --version 0.2.35 -y

# Make an /app dir, which everything will eventually live in
RUN mkdir -p /app
WORKDIR /app
COPY . .

# Build the app
ENV LEPTOS_ENV="PROD"
RUN LEPTOS_TAILWIND_VERSION=v3.4.10 cargo leptos build --release -vv

FROM debian:bookworm-slim as runtime
RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates libssl3 pkg-config \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/l3chat /app/

# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /app/target/site /app/site

# Copy Cargo.toml if it's needed at runtime
COPY --from=builder /app/Cargo.toml /app/

COPY ./scripts/start.sh /app/start.sh
RUN chmod +x /app/start.sh


# Set any required env variables and
ENV RUST_LOG="info" \
    LEPTOS_SITE_ADDR="0.0.0.0:8080" \
    LEPTOS_SITE_ROOT="site" \
    LEPTOS_OUTPUT_NAME="l3chat" \
    LEPTOS_ENV="PROD"
EXPOSE 8080

# Run the server
CMD ["/app/start.sh"]
