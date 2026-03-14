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

# dx build compila o cliente WASM e o binário do servidor numa única passagem,
# processando todos os asset!() e gerando os hashes corretos.
# O cargo build separado NÃO deve ser usado: o binário resultante não passa pelo
# pipeline de assets do dx e quebra as referências de asset!().
RUN dx build --platform web --release

RUN bundle_dir="$(find target/dx -type d -name 'web' | grep release | head -n 1)" \
    && test -n "$bundle_dir" \
    && mkdir -p /out/public /out/assets /out/migrations \
    && cp "$bundle_dir/hermes" /out/hermes \
    && cp -r assets/. /out/assets/ \
    && cp -r migrations/. /out/migrations/ \
    && cp -r "$bundle_dir/public"/. /out/public/

FROM debian:bookworm-slim

ENV HOST=0.0.0.0 \
    PORT=8080 \
    DATABASE_URL=sqlite:///var/lib/hermes/hermes.db?mode=rwc \
    STORAGE_DIR=/var/lib/hermes/uploads \
    RUST_LOG=hermes=info

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        gosu \
        libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --home-dir /app --shell /usr/sbin/nologin hermes \
    && mkdir -p /app /var/lib/hermes /var/lib/hermes/uploads \
    && chown -R hermes:hermes /app /var/lib/hermes

COPY --from=builder /out/hermes /app/hermes
COPY --from=builder /out/assets /app/assets
COPY --from=builder /out/public /app/public
COPY --from=builder /out/migrations /app/migrations
COPY deploy/cloudflare/entrypoint.sh /usr/local/bin/entrypoint.sh

RUN chmod +x /usr/local/bin/entrypoint.sh

WORKDIR /app

VOLUME ["/var/lib/hermes"]

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
