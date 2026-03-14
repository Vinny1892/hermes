#!/bin/sh
set -e

# Fix ownership of the volume mount point (runs as root before dropping privileges).
chown -R hermes:hermes /var/lib/hermes
mkdir -p /var/lib/hermes/uploads

exec gosu hermes /app/hermes "$@"
