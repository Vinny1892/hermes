#!/bin/sh
set -eu

while true; do
    if [ -n "${APP_DOMAIN:-}" ] && [ -n "${LETSENCRYPT_EMAIL:-}" ]; then
        certbot renew --webroot -w /var/www/certbot --quiet --deploy-hook "nginx -s reload" || true
    fi
    sleep "${CERTBOT_RENEW_INTERVAL_SECONDS:-43200}"
done
