FROM --platform=linux/amd64 rust:1.90-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install diesel_cli --no-default-features --features sqlite

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

COPY . .
RUN touch src/main.rs
RUN cargo build --release

RUN mkdir -p /app/libtorch && \
    cp -r /app/target/release/build/torch-sys-*/out/libtorch/libtorch/lib/* /app/libtorch/

FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libsqlite3-0 \
    libssl3 \
    libgomp1 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/devlogs-feed .
COPY --from=builder /app/target/release/score-post .
COPY --from=builder /usr/local/cargo/bin/diesel .
COPY --from=builder /app/libtorch /usr/lib/
COPY migrations ./migrations/
COPY diesel.toml ./
COPY settings*.ron ./

ENV RUST_LOG=info
ENV LD_LIBRARY_PATH=/usr/lib

EXPOSE 3030

CMD ["sh", "-c", "./diesel setup && ./diesel migration run && ./devlogs-feed"]
