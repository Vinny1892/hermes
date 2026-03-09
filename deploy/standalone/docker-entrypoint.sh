#!/bin/sh
set -eu

render_nginx_config() {
    if [ -f "/etc/letsencrypt/live/${CERTBOT_CERT_NAME}/fullchain.pem" ] && [ -f "/etc/letsencrypt/live/${CERTBOT_CERT_NAME}/privkey.pem" ]; then
        envsubst '${APP_PORT} ${NGINX_SERVER_NAME} ${NGINX_CLIENT_MAX_BODY_SIZE} ${CERTBOT_CERT_NAME}' \
            < /etc/nginx/templates/nginx.tls.conf.template \
            > /etc/nginx/nginx.conf
    else
        envsubst '${APP_PORT} ${NGINX_SERVER_NAME} ${NGINX_CLIENT_MAX_BODY_SIZE}' \
            < /etc/nginx/templates/nginx.http.conf.template \
            > /etc/nginx/nginx.conf
    fi
}

export APP_PORT="${APP_PORT:-8080}"
export PORT="${PORT:-$APP_PORT}"
export HOST="${HOST:-0.0.0.0}"
export NGINX_CLIENT_MAX_BODY_SIZE="${NGINX_CLIENT_MAX_BODY_SIZE:-2g}"
export APP_DOMAIN="${APP_DOMAIN:-}"
export NGINX_SERVER_NAME="${NGINX_SERVER_NAME:-${APP_DOMAIN:-_}}"
export CERTBOT_CERT_NAME="${CERTBOT_CERT_NAME:-${APP_DOMAIN:-default}}"

if [ -z "${BASE_URL:-}" ] && [ -n "${APP_DOMAIN}" ]; then
    export BASE_URL="https://${APP_DOMAIN}"
fi

mkdir -p /var/www/certbot /var/lib/hermes /var/lib/hermes/uploads /etc/letsencrypt
chown -R hermes:hermes /var/lib/hermes /app

render_nginx_config

/usr/local/bin/bootstrap-certbot.sh &

exec "$@"
