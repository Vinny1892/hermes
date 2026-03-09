# Hermes

Sistema de compartilhamento de arquivos entre amigos com suporte a transferência direta P2P via WebRTC e upload no servidor como fallback. Construído com Rust + [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) Fullstack.

---

## Funcionalidades implementadas (V1)

| Feature | Status |
|---------|--------|
| Upload para o servidor (multipart) | Implementado |
| Download com streaming | Implementado |
| Links compartilháveis públicos (expiração 10 min) | Implementado |
| Expiração automática de arquivos (7 dias) | Implementado |
| Transferência P2P via WebRTC DataChannel | Implementado |
| Signaling server (WebSocket relay) | Implementado |
| Seleção múltipla de arquivos | Implementado |
| Storage local plugável (trait `StorageBackend`) | Implementado |
| Cleanup periódico (arquivos e sessões expirados) | Implementado |

---

## Modos de transferência

### Modo 1 — Upload no servidor

```
Usuário A                     Servidor                     Usuário B
    |                             |                             |
    |── POST /api/upload ────────>|                             |
    |<─ { file_id, download_url } |                             |
    |── gera share link ─────────>|                             |
    |<─ /share/{token} (10 min)   |                             |
    |                             |                             |
    |   compartilha link          |                             |
    | ─────────────────────────────────────────────────────────>|
    |                             |                             |
    |                             |<── GET /share/{token} ─────|
    |                             |── redirect /f/{file_id} ──>|
    |                             |<── GET /f/{file_id} ───────|
    |                             |── stream arquivo ─────────>|
```

- Arquivo fica disponível por **7 dias**
- O link compartilhável expira em **10 minutos** (link para iniciar o download)
- Permite múltiplos downloads simultâneos

### Modo 2 — P2P direto (WebRTC DataChannel)

```
Peer A ──── WebSocket ────> Signaling Server <──── WebSocket ──── Peer B
       offer/answer/ICE                    offer/answer/ICE

Após handshake:
Peer A ──────────────── DataChannel (DTLS) ──────────────────── Peer B
         chunks de 64 KB com ACK por chunk
```

- O arquivo **não passa pelo servidor**
- Criptografia automática via DTLS
- Fallback automático: P2P direto → STUN (NAT traversal) → TURN (relay)
- Protocolo de chunking: 64 KB por chunk, stop-and-wait (ack por chunk, 3 retries)

---

## Arquitetura

### Estrutura do projeto

```
hermes/
├── src/
│   ├── main.rs              # Entry point: servidor Axum (server) ou dioxus::launch (WASM)
│   ├── lib.rs               # Re-exports para testes de integração
│   ├── api.rs               # Server functions Dioxus (client + server)
│   ├── app.rs               # Routes, componente App, Navbar
│   ├── models/
│   │   ├── file.rs          # FileRecord, UploadResponse, ShareLinkResponse, FileInfo
│   │   └── session.rs       # P2pSession, SessionState, SignalMessage (protocolo WebRTC)
│   ├── components/
│   │   ├── uploader.rs      # FileUploader (drag-and-drop, upload via fetch JS)
│   │   └── progress.rs      # ProgressBar
│   ├── pages/
│   │   ├── home.rs          # Página principal: seletor de modo + upload + P2P
│   │   ├── download.rs      # Página de download: info do arquivo + botão
│   │   └── receive.rs       # Página do receptor P2P: inicializa WebRTC JS
│   └── server/              # Código exclusivo do servidor (cfg(not(wasm32)))
│       ├── db.rs            # Pool SQLite, init_db(), global_pool(), test_pool()
│       ├── upload.rs        # Handler POST /api/upload + persist_upload()
│       ├── download.rs      # Handlers GET /f/:id e GET /share/:token
│       ├── sessions.rs      # CRUD de sessões P2P (create, get, close, purge)
│       ├── signaling.rs     # WebSocket relay + SignalingRegistry (in-memory)
│       ├── cleanup.rs       # Task periódica de limpeza (1h)
│       └── storage/
│           ├── mod.rs       # Trait StorageBackend
│           └── local.rs     # LocalStorage (filesystem)
├── migrations/
│   └── 0001_init.sql        # Schema: files, share_links, p2p_sessions
├── assets/
│   ├── webrtc.js            # Toda a lógica WebRTC/DataChannel roda aqui (browser)
│   └── main.css             # Estilos base
└── tests/
    ├── storage.rs           # Testes de integração: LocalStorage
    └── sessions.rs          # Testes de integração: sessões P2P
```

### Rotas HTTP

| Método | Path | Descrição |
|--------|------|-----------|
| `POST` | `/api/upload` | Upload multipart → `{ file_id, download_url }` |
| `GET` | `/f/:file_id` | Download do arquivo (streaming) |
| `GET` | `/share/:token` | Resolve share link → redirect 307 para `/f/:file_id` |
| `GET` | `/ws/signal/:session_id` | WebSocket de signaling para WebRTC |

### Server functions (Dioxus)

Disponíveis tanto no cliente (WASM) quanto no servidor:

| Função | Descrição |
|--------|-----------|
| `get_file_info(file_id)` | Retorna metadados do arquivo para a página de download |
| `generate_share_link(file_id)` | Cria token de 10 min e retorna a URL compartilhável |
| `create_p2p_session()` | Cria sessão de signaling e retorna a URL do WebSocket |

### Banco de dados (SQLite)

```sql
files          -- metadados dos arquivos armazenados (TTL 7 dias)
share_links    -- tokens de links públicos (TTL 10 min)
p2p_sessions   -- sessões de signaling WebRTC (TTL 10 min)
```

Migrations em `migrations/` são aplicadas automaticamente na inicialização via `sqlx::migrate!`.

---

## Protocolo de transferência P2P

### Signaling (via WebSocket)

```
Peer A conecta → /ws/signal/{session_id}   (slot 'a')
Peer B conecta → /ws/signal/{session_id}   (slot 'b')

A → Servidor: { "type": "offer",         "sdp": "..." }
Servidor → B: { "type": "offer",         "sdp": "..." }
B → Servidor: { "type": "answer",        "sdp": "..." }
Servidor → A: { "type": "answer",        "sdp": "..." }
A ↔ B via Servidor: { "type": "ice-candidate", "candidate": "..." }
Conexão P2P estabelecida
```

- Sessão expira em 10 minutos sem ambos os peers
- Se um peer cair, o servidor envia `{ "type": "bye" }` para o outro
- Máximo de 2 peers por sessão; terceira conexão é rejeitada

### Chunking (via DataChannel)

```
Sender → Receiver: { "type": "file-start", "name": "...", "size": N, "total_chunks": M }
Sender → Receiver: { "type": "chunk",      "index": 0, "data": "<base64 64KB>" }
Receiver → Sender: { "type": "ack",        "index": 0 }
Sender → Receiver: { "type": "chunk",      "index": 1, "data": "..." }
Receiver → Sender: { "type": "ack",        "index": 1 }
...
Sender → Receiver: { "type": "file-end" }
```

- Chunks de 64 KB
- Stop-and-wait: sender aguarda `ack` antes de enviar o próximo chunk
- Timeout de 10s por ack; até 3 tentativas antes de encerrar com erro

---

## Como rodar

### Pré-requisitos

```sh
# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Instalar o CLI do Dioxus
curl -sSL http://dioxus.dev/install.sh | sh
```

### Desenvolvimento

```sh
# Servidor + frontend com hot-reload
dx serve --platform web
```

A aplicação estará disponível em `http://localhost:8080`.

### Build de produção

```sh
dx build --platform web --release
```

### Apenas o servidor (sem frontend WASM)

```sh
cargo run --features server
```

---

## Testes

```sh
# Todos os testes (unit + integração)
cargo test --features server

# Suíte de storage
cargo test --features server --test storage

# Suíte de sessões P2P
cargo test --features server --test sessions
```

### Cobertura atual

| Suíte | Testes | O que cobre |
|-------|--------|-------------|
| `server::storage::local` | 6 | put/get/delete, overwrite, path traversal, concurrent writes |
| `server::signaling` | 5 | registro de peers, relay de mensagens, bye ao desconectar |
| `server::sessions` | 5 | criação, transição de estado, close, purge |
| `server::cleanup` | 2 | purge de arquivos expirados, preservação de válidos |
| `pages::download` | 4 | formatação de tamanho e expiração |
| `tests/storage` | 8 | integração completa do storage (inclui concorrência) |
| `tests/sessions` | 9 | integração completa das sessões |
| **Total** | **39** | |

---

## Variáveis de ambiente

| Variável | Padrão | Descrição |
|----------|--------|-----------|
| `DATABASE_URL` | `sqlite:hermes.db` | Caminho do banco SQLite |
| `PORT` | `8080` | Porta HTTP |
| `BASE_URL` | `http://localhost:8080` | Base para construir URLs de WebSocket nas sessões |
| `RUST_LOG` | — | Filtro de logs (ex: `hermes=debug`) |

---

## Roadmap

### V2 (planejado)
- Autenticação: senha + TOTP + FIDO2/WebAuthn
- Sistema invite-only
- Roles: Admin, User, Guest
- Rate limiting (4 tentativas → bloqueio)
- Audit log (uploads, downloads, falhas de auth)
- Limites de storage por usuário (padrão 1 GB)
- Histórico de arquivos por usuário

### V3 (futuro)
- Criptografia client-side
- Upload resumable
- Backend S3-compatible (AWS, Cloudflare R2, MinIO)
- Cliente desktop dedicado

---

## Deploy mínimo

```
Nginx (TLS)
  └── Hermes server :8080
        └── SQLite (hermes.db)
        └── storage/uploads/

coturn (TURN server, para redes restritivas)
```

Servidor mínimo recomendado: 2 vCPU, 2 GB RAM.

## Deploy com Docker

O `Dockerfile` raiz agora gera uma imagem só da aplicação Hermes, sem `nginx` e sem `certbot`.

### Build da imagem

```sh
docker build -t hermes-app .
```

### Run direto

```sh
docker run -d \
  --name hermes \
  -p 8080:8080 \
  -e BASE_URL=https://files.exemplo.com \
  -v hermes_data:/var/lib/hermes \
  hermes-app
```

### Variáveis de ambiente do container

| Variável | Padrão | Descrição |
|----------|--------|-----------|
| `BASE_URL` | `http://localhost:8080` | URL pública usada pelo Hermes |
| `HOST` | `0.0.0.0` | Interface de bind do servidor |
| `PORT` | `8080` | Porta HTTP exposta pelo binário |
| `DATABASE_URL` | `sqlite:/var/lib/hermes/hermes.db` | Banco SQLite persistido em volume |
| `STORAGE_DIR` | `/var/lib/hermes/uploads` | Diretório persistente dos arquivos enviados |
| `RUST_LOG` | `hermes=info` | Nível de logs da aplicação |

### Variante completa

Se você ainda quiser a imagem all-in-one com `nginx` + `certbot`, ela foi preservada em `deploy/standalone/Dockerfile`:

```sh
docker build -f deploy/standalone/Dockerfile -t hermes-standalone .
```
