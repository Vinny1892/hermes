#!/bin/sh
set -eu

render_tls_config() {
    envsubst '${APP_PORT} ${NGINX_SERVER_NAME} ${NGINX_CLIENT_MAX_BODY_SIZE} ${CERTBOT_CERT_NAME}' \
        < /etc/nginx/templates/nginx.tls.conf.template \
        > /etc/nginx/nginx.conf
}

if [ -z "${APP_DOMAIN:-}" ] || [ -z "${LETSENCRYPT_EMAIL:-}" ]; then
    echo "bootstrap-certbot: APP_DOMAIN or LETSENCRYPT_EMAIL not set, skipping certificate bootstrap"
    exit 0
fi

if [ -f "/etc/letsencrypt/live/${CERTBOT_CERT_NAME}/fullchain.pem" ] && [ -f "/etc/letsencrypt/live/${CERTBOT_CERT_NAME}/privkey.pem" ]; then
    echo "bootstrap-certbot: existing certificate found for ${CERTBOT_CERT_NAME}"
    exit 0
fi

attempt=0
until nginx -t >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 30 ]; then
        echo "bootstrap-certbot: nginx configuration never became ready"
        exit 1
    fi
    sleep 2
done

sleep 5

certbot_args="--non-interactive --agree-tos --email ${LETSENCRYPT_EMAIL} --webroot -w /var/www/certbot --cert-name ${CERTBOT_CERT_NAME} -d ${APP_DOMAIN}"

if [ "${CERTBOT_STAGING:-0}" = "1" ]; then
    certbot_args="${certbot_args} --staging"
fi

echo "bootstrap-certbot: requesting certificate for ${APP_DOMAIN}"
# shellcheck disable=SC2086
if certbot certonly ${certbot_args}; then
    render_tls_config
    nginx -s reload
else
    echo "bootstrap-certbot: certificate request failed, container will continue in HTTP mode"
fi
