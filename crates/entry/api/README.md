<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-api

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/entry-api.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/entry-api.svg">
    <img alt="systemprompt-api terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/entry-api.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-api.svg?style=flat-square)](https://crates.io/crates/systemprompt-api)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-api?style=flat-square)](https://docs.rs/systemprompt-api)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Axum-based HTTP server and API gateway for systemprompt.io AI governance infrastructure. Exposes governed agents, MCP, A2A, OAuth, the Claude gateway, marketplace, sync, analytics, and admin endpoints behind a unified middleware stack with authentication, rate limiting, RBAC, content negotiation, and security headers.

**Layer**: Entry — application boundary. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

> This crate exposes a public library surface (`ApiServer`, route routers, middleware extractors) consumed by `entry/cli`.

## Overview

The Entry layer turns an `AppContext` into a running Axum server. Responsibilities:

- **Route mounting** — every domain crate's router is composed under one tree by `services/server/routes.rs`.
- **Middleware stack** — JWT, sessions, CORS, IP ban, rate limiting, throttling, bot detection, analytics emission, context extraction, content negotiation, security headers, and trailing-slash normalization.
- **Gateway** — proxies Claude API traffic with quota enforcement, safety filtering, OTel ingest, audit, and pricing capture.
- **Static content** — serves the prebuilt web frontend with ETag, SPA fallback, and per-route session handling.
- **Server lifecycle** — readiness probes, agent reconciliation, and scheduler bootstrap.

## Source layout

```
src/
├── lib.rs                          # Re-exports: ApiServer, HealthChecker, ContextMiddleware, ServerConfig
├── models/
│   └── mod.rs                      # ServerConfig
├── routes/
│   ├── mod.rs
│   ├── wellknown.rs                # /.well-known/* (agent cards, OAuth metadata)
│   ├── marketplace.rs              # Marketplace catalog endpoints
│   ├── admin/                      # CLI gateway, keys
│   ├── agent/                      # A2A: artifacts, contexts (+ events, notifications, webhook), registry, tasks, responses
│   ├── analytics/                  # Event ingestion + SSE stream
│   ├── content/                    # Blog, query, link tracking
│   ├── engagement/                 # Engagement metrics
│   ├── gateway/                    # Claude gateway: auth, bridge (data, heartbeat, manifest, profile usage, whoami), messages, OTel
│   ├── mcp/                        # MCP server registry
│   ├── oauth/                      # OAuth2/OIDC: discovery, endpoints, clients, webauthn, wellknown, health
│   ├── proxy/                      # A2A and MCP request forwarding
│   ├── stream/                     # SSE for context updates
│   └── sync/                       # File and auth sync for offline-first clients
└── services/
    ├── mod.rs
    ├── validation.rs               # Cross-route validation helpers
    ├── gateway/                    # ClaudeGatewayService — audit, captures, parse, policy, pricing, protocol, quota, registry, safety, stream_tap
    ├── health/                     # HealthChecker, ProcessMonitor
    ├── middleware/
    │   ├── analytics/              # Bot/scanner detection + event emission
    │   ├── context/                # Context extraction with header/A2A extractors and header/payload sources
    │   ├── jwt/                    # Token validation + JWT context
    │   ├── negotiation/            # Accept-header content negotiation
    │   ├── session/                # Lifecycle and skip rules
    │   ├── auth.rs                 # Route-level auth gate
    │   ├── bot_detector.rs         # Bot fingerprinting
    │   ├── cors.rs
    │   ├── ip_ban.rs
    │   ├── rate_limit.rs
    │   ├── security_headers.rs
    │   ├── session.rs              # Session middleware entry
    │   ├── site_auth.rs            # Site-wide auth gate
    │   ├── throttle.rs
    │   ├── trace.rs
    │   └── trailing_slash.rs
    ├── proxy/                      # ProxyEngine: auth, backend transform, client pool, resolver, MCP session
    ├── server/
    │   ├── builder.rs              # ApiServer construction
    │   ├── discovery.rs            # Extension router discovery
    │   ├── health.rs               # Health endpoint (incl. portable disk usage)
    │   ├── health_detail.rs        # Detailed health payload
    │   ├── readiness.rs            # Readiness probe state
    │   ├── routes.rs / routes/     # Route tree, extension mount, protocol mount, static setup
    │   ├── runner.rs               # Server entry point (run_server)
    │   └── lifecycle/              # Agent reconciliation + scheduler bootstrap
    └── static_content/             # SPA fallback, homepage, static files (cache + responses), session handling
```

## Route surface

| Module | Description |
|--------|-------------|
| `admin` | CLI gateway and key-management endpoints. |
| `agent` | A2A protocol — artifacts, contexts, tasks, registry, webhook broadcasts, notifications. |
| `analytics` | Event ingestion, batch processing, and SSE streaming. |
| `content` | Blog, content queries, and link redirect tracking. |
| `engagement` | Engagement metrics fan-out from analytics events. |
| `gateway` | Claude API gateway: bridge auth/data/heartbeat/manifest/profile-usage/whoami, message dispatch, OTLP ingest. |
| `marketplace` | Marketplace catalog and asset endpoints. |
| `mcp` | MCP server registry. |
| `oauth` | OAuth2/OIDC authorize, token, clients, WebAuthn, discovery, and `.well-known` metadata. |
| `proxy` | Forwards requests to MCP servers and A2A agents through `ProxyEngine`. |
| `stream` | Server-Sent Events for live context updates. |
| `sync` | File and auth sync for offline-first clients (tar+gzip payloads). |
| `wellknown` | Standard discovery endpoints (agent cards, OAuth protected resource). |

## Service surface

| Module | Description |
|--------|-------------|
| `gateway` | Claude gateway service — quota, audit, safety, pricing, stream tap, OTel capture. |
| `health` | Process monitoring and HTTP health checks. |
| `middleware` | Request pipeline: JWT, session, context, analytics, CORS, IP ban, rate limiting, throttling, security headers, content negotiation, trailing-slash normalization. |
| `proxy` | HTTP client pool and request transformation for upstream MCP and A2A targets. |
| `server` | Builder, route tree, readiness, lifecycle (agent reconciliation + scheduler), and runner. |
| `static_content` | SPA serving, homepage, static-file cache and response building, fallback routing. |

## Usage

```toml
[dependencies]
systemprompt-api = "0.12.0"
```

```rust
use systemprompt_api::services::server::{run_server, setup_api_server};
use systemprompt_runtime::AppContext;

let ctx = AppContext::new().await?;
run_server(ctx, None).await?;
```

## Configuration

The API server is configured through `systemprompt-runtime::Config` and the active profile:

- `api_external_url` — public URL advertised in discovery metadata.
- `rate_limits` — per-endpoint rate limit configuration.
- `security.signing_key_path` — RSA private key the in-process `TokenAuthority` uses to sign RS256 access tokens. The matching public set is published at `/.well-known/jwks.json`; `systemprompt admin keys generate` mints the keypair.
- `security.trusted_issuers` — additional issuer → JWKS URI entries consulted by the RFC 8693 token-exchange grant when validating non-self-issued subject tokens.
- `oauth_at_rest_pepper` — HMAC pepper (>= 32 chars, loaded via the secrets bootstrap) under which refresh-token ids and authorisation codes are stored as HMAC-SHA-256 digests.
- `cors` — allowed origins.
- `paths.system` — root used by `static_content` to locate prebuilt web assets.

## Notes

- Handlers extract request data and delegate to domain services; no direct repository access.
- All routes are composed in `services/server/routes.rs`; extensions are discovered via `services/server/discovery.rs`.
- Middleware order is significant — see `services/server/builder.rs`.
- The gateway path mints a UUID v5 `ContextId` from `GatewayConversationId`; it does not read upstream `x-context-id`.
- Static content requires prebuilt web assets under the configured system path.

## Dependencies

### Internal crates

- `systemprompt-runtime` — application context and configuration
- `systemprompt-oauth` — authentication and session management
- `systemprompt-agent` — agent registry, A2A protocol, orchestration
- `systemprompt-mcp` — MCP server registry and proxy
- `systemprompt-content` — content repository and serving
- `systemprompt-analytics` — session and event tracking
- `systemprompt-scheduler` — background job execution
- `systemprompt-marketplace` — marketplace catalog
- `systemprompt-ai` — Claude gateway integrations
- `systemprompt-database` — connection pooling
- `systemprompt-security` — token extraction and validation
- `systemprompt-users` — user services and IP banning
- `systemprompt-events` — event broadcasting
- `systemprompt-files` — file system configuration
- `systemprompt-extension` — extension loading and routing
- `systemprompt-config`, `systemprompt-loader`, `systemprompt-logging`, `systemprompt-models`, `systemprompt-identifiers`, `systemprompt-traits`

### External crates

- `axum`, `tower`, `tower-http`, `tower_governor`, `governor` — HTTP framework and middleware
- `tokio`, `tokio-stream`, `async-stream`, `futures-util` — async runtime
- `reqwest` — upstream HTTP client
- `rmcp` — MCP transport
- `jsonwebtoken`, `webauthn-rs`, `bcrypt`, `ed25519-dalek` — auth primitives
- `opentelemetry-proto`, `prost` — OTLP ingest
- `flate2`, `tar` — sync payload (de)compression
- `sqlx` — database access

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-api)** · **[docs.rs](https://docs.rs/systemprompt-api)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Entry layer · Own how your organization uses AI.</sub>

</div>
