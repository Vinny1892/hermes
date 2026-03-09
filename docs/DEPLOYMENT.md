---
title: "Hermes: Deployment Guide"
description: "How to build, deploy, and run Hermes using Docker (Root vs Standalone versions)."
---

# 🚀 Hermes Deployment Guide

Hermes provides two distinct Docker-based deployment paths depending on your infrastructure needs.

## 📦 Dockerfile Overview

| Version | Path | Target Use Case | Features |
| :--- | :--- | :--- | :--- |
| **Standard** | `Dockerfile` | Cloud (K8s, ECS), Reverse Proxy | Lightweight, single process. |
| **Standalone** | `deploy/standalone/Dockerfile` | VPS, Bare Metal | Nginx, Certbot (SSL), Supervisord. |

---

## 🏗️ 1. Standard Deployment (Root Dockerfile)

This is the recommended approach if you already have a load balancer or reverse proxy (like Traefik, Caddy, or Nginx) handling SSL.

### Build and Run
```bash
# Build the image
docker build -t hermes:latest .

# Run the container
docker run -d \
  -p 8080:8080 \
  -v hermes_data:/var/lib/hermes \
  -e DATABASE_URL=sqlite:/var/lib/hermes/hermes.db \
  hermes:latest
```

### Invariants
- **Base Image**: `debian:bookworm-slim` `(Dockerfile:41)`
- **User**: Runs as non-root `hermes` user `(Dockerfile:59)`
- **Ports**: Exposes `8080` `(Dockerfile:62)`

---

## 🛠️ 2. Standalone Deployment (Recommended for VPS)

The Standalone version is "batteries-included". It bundles **Nginx** for performance/caching, **Certbot** for automatic Let's Encrypt SSL, and **Supervisord** to manage both the app and Nginx.

### Features
- **Auto-SSL**: Automatically requests and renews Let's Encrypt certificates `(deploy/standalone/Dockerfile:70)`.
- **Nginx Proxy**: Handles static file serving and reverse proxies to the Rust binary `(deploy/standalone/docker-entrypoint.sh:5)`.
- **Process Management**: Uses `supervisord` to ensure the app stays running `(deploy/standalone/Dockerfile:82)`.

### Environment Variables
| Variable | Default | Description |
| :--- | :--- | :--- |
| `APP_DOMAIN` | - | Your domain (e.g., `hermes.example.com`). Trigger SSL. |
| `NGINX_CLIENT_MAX_BODY_SIZE` | `2g` | Maximum upload size allowed by Nginx. |
| `DATABASE_URL` | `sqlite:/var/lib/hermes/hermes.db` | Path to the SQLite DB. |

### Build and Run
```bash
docker build -t hermes-standalone -f deploy/standalone/Dockerfile .

docker run -d \
  -p 80:80 -p 443:443 \
  -e APP_DOMAIN=share.mydomain.com \
  -v hermes_certs:/etc/letsencrypt \
  -v hermes_data:/var/lib/hermes \
  hermes-standalone
```

---

## 🔄 Deployment Architecture

### Standalone Internal Flow
```mermaid
flowchart TD
    Internet((Internet))
    subgraph "Standalone Container"
        NGINX[Nginx (80/443)]
        CB[Certbot]
        APP[Hermes Rust App (8080)]
        DB[(SQLite)]
        FS[[Local Storage]]
    end

    Internet -- HTTPS --> NGINX
    NGINX -- Reverse Proxy --> APP
    APP --> DB
    APP --> FS
    CB -- Challenges --> NGINX
    CB -- Renews --> FS
    
    style NGINX fill:#2d333b,stroke:#6d5dfc,color:#e6edf3
    style APP fill:#2d333b,stroke:#6d5dfc,color:#e6edf3
    style DB fill:#161b22,stroke:#30363d,color:#e6edf3
```

---

## 🛠️ Configuration Citations

### Persistence
Both Dockerfiles use a volume at `/var/lib/hermes` to persist the database and uploaded files.

> **Code Citation**: `(Dockerfile:61)` and `(deploy/standalone/Dockerfile:78)`
> ```dockerfile
> VOLUME ["/var/lib/hermes"]
> ```

### Initialization Logic
The standalone version uses a custom entrypoint to dynamically generate the Nginx configuration based on whether SSL certificates are present.

> **Code Citation**: `(deploy/standalone/docker-entrypoint.sh:4)`
> ```bash
> render_nginx_config() {
>     if [ -f "/etc/letsencrypt/live/${CERTBOT_CERT_NAME}/fullchain.pem" ]; then
>         # Renders TLS template
>     else
>         # Renders HTTP template
>     fi
> }
> ```

## 🧹 Maintenance

### Manual SSL Renewal (Standalone)
Certbot runs in the background, but you can trigger a renewal check:
```bash
docker exec <container_id> /usr/local/bin/certbot-renew.sh
```

### Database Backups
Since Hermes uses SQLite, you can simply copy the file while the container is running (for small loads) or use the `.backup` command:
```bash
docker exec <container_id> sqlite3 /var/lib/hermes/hermes.db ".backup '/var/lib/hermes/backup.db'"
```
