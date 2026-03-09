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
RUN dx build --platform web --release --features "server web"

RUN mkdir -p /out/public /out/assets /out/migrations \
    && cp target/release/hermes /out/hermes \
    && cp -r assets/. /out/assets/ \
    && cp -r migrations/. /out/migrations/ \
    && public_dir="$(find target/dx -type d -path '*/release/web/public' | head -n 1)" \
    && test -n "$public_dir" \
    && cp -r "$public_dir"/. /out/public/

FROM debian:bookworm-slim

ENV APP_DIR=/app \
    APP_PORT=8080 \
    HOST=0.0.0.0 \
    PORT=8080 \
    DATABASE_URL=sqlite:/var/lib/hermes/hermes.db \
    STORAGE_DIR=/var/lib/hermes/uploads \
    RUST_LOG=hermes=info

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        certbot \
        gettext-base \
        nginx \
        supervisor \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --home-dir /app --shell /usr/sbin/nologin hermes \
    && mkdir -p \
        /app \
        /etc/nginx/templates \
        /etc/supervisor/conf.d \
        /var/lib/hermes \
        /var/www/certbot \
        /var/log/supervisor \
    && chown -R hermes:hermes /app /var/lib/hermes

COPY --from=builder /out/hermes /app/hermes
COPY --from=builder /out/assets /app/assets
COPY --from=builder /out/public /app/public
COPY --from=builder /out/migrations /app/migrations

COPY deploy/standalone/nginx.http.conf.template /etc/nginx/templates/nginx.http.conf.template
COPY deploy/standalone/nginx.tls.conf.template /etc/nginx/templates/nginx.tls.conf.template
COPY deploy/standalone/supervisord.conf /etc/supervisor/conf.d/hermes.conf
COPY deploy/standalone/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
COPY deploy/standalone/bootstrap-certbot.sh /usr/local/bin/bootstrap-certbot.sh
COPY deploy/standalone/certbot-renew.sh /usr/local/bin/certbot-renew.sh

RUN chmod +x \
        /usr/local/bin/docker-entrypoint.sh \
        /usr/local/bin/bootstrap-certbot.sh \
        /usr/local/bin/certbot-renew.sh

WORKDIR /app

VOLUME ["/var/lib/hermes", "/etc/letsencrypt", "/var/www/certbot"]

EXPOSE 80 443

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/hermes.conf"]
