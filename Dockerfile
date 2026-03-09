FROM rust:1.88-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        clang \
        libsqlite3-dev \
        pkg-config \
        curl \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown
RUN cargo install dioxus-cli --version 0.7.1 --locked

COPY Cargo.toml Cargo.lock Dioxus.toml ./
COPY assets ./assets
COPY migrations ./migrations
COPY src ./src
COPY tailwind.css ./tailwind.css

RUN cargo build --release --features "server web"
RUN dx build --platform web --release --no-default-features --features web

RUN mkdir -p /out/public /out/assets /out/migrations \
    && cp target/release/hermes /out/hermes \
    && cp -r assets/. /out/assets/ \
    && cp -r migrations/. /out/migrations/ \
    && public_dir="$(find target/dx -type d -path '*/release/web/public' | head -n 1)" \
    && test -n "$public_dir" \
    && cp -r "$public_dir"/. /out/public/

FROM debian:bookworm-slim

ENV APP_DIR=/app \
    HOST=0.0.0.0 \
    PORT=8080 \
    DATABASE_URL=sqlite:/var/lib/hermes/hermes.db \
    STORAGE_DIR=/var/lib/hermes/uploads \
    RUST_LOG=hermes=info

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --home-dir /app --shell /usr/sbin/nologin hermes \
    && mkdir -p /app /var/lib/hermes/uploads \
    && chown -R hermes:hermes /app /var/lib/hermes

COPY --from=builder /out/hermes /app/hermes
COPY --from=builder /out/assets /app/assets
COPY --from=builder /out/public /app/public
COPY --from=builder /out/migrations /app/migrations

WORKDIR /app

VOLUME ["/var/lib/hermes"]

EXPOSE 8080

USER hermes

CMD ["/app/hermes"]
