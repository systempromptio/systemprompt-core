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

Axum-based HTTP server and API gateway for systemprompt.io AI governance infrastructure. Exposes governed agents, MCP, A2A, and admin endpoints with rate limiting and RBAC. Serves as the entry point for all HTTP requests to systemprompt.io OS.

**Layer**: Entry — application boundary. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Part of the Entry layer in the systemprompt.io architecture.
**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

This crate serves as the entry point for all HTTP requests to systemprompt.io OS. It provides:

- **Route Configuration:** Mounts all API endpoints from domain crates
- **Middleware Stack:** Authentication, rate limiting, analytics, CORS, session management
- **Proxy Services:** Forwards requests to MCP servers and A2A agents
- **Static Content:** Serves the web frontend and handles SPA routing
- **Server Lifecycle:** Manages startup, health checks, and graceful shutdown

## Architecture

The API crate follows the Entry layer pattern:
- Handlers extract request data and delegate to domain services
- No direct database access (uses repositories through injected services)
- Middleware handles cross-cutting concerns

```
src/
├── lib.rs                                    # Crate exports
├── models/
│   └── mod.rs                                # ServerConfig
├── routes/
│   ├── mod.rs                                # Route exports
│   ├── wellknown.rs                          # /.well-known/* endpoints
│   ├── admin/
│   │   ├── mod.rs                            # Admin route exports
│   │   └── cli.rs                            # CLI gateway endpoint
│   ├── analytics/
│   │   ├── mod.rs                            # Analytics route exports
│   │   ├── events.rs                         # Event tracking endpoints
│   │   └── stream.rs                         # SSE analytics stream
│   ├── engagement/
│   │   ├── mod.rs                            # Engagement route exports
│   │   └── handlers.rs                       # Engagement tracking handlers
│   ├── proxy/
│   │   ├── mod.rs                            # Proxy route exports
│   │   ├── agents.rs                         # A2A agent proxy routes
│   │   └── mcp.rs                            # MCP server proxy routes
│   ├── stream/
│   │   ├── mod.rs                            # SSE stream exports
│   │   └── contexts.rs                       # Context state streaming
│   └── sync/
│       ├── mod.rs                            # Sync route exports
│       ├── types.rs                          # Request/response types
│       ├── auth.rs                           # Sync authentication
│       └── files.rs                          # File sync endpoints
└── services/
    ├── mod.rs                                # Service exports
    ├── health/
    │   ├── mod.rs                            # Health service exports
    │   ├── checker.rs                        # HTTP health checker
    │   └── monitor.rs                        # Process health monitor
    ├── middleware/
    │   ├── mod.rs                            # Middleware exports
    │   ├── analytics/
    │   │   ├── mod.rs                        # Analytics middleware
    │   │   ├── detection.rs                  # Bot/scanner detection
    │   │   └── events.rs                     # Event emission
    │   ├── auth.rs                           # Route-level authentication
    │   ├── bot_detector.rs                   # Bot identification
    │   ├── context/
    │   │   ├── mod.rs                        # Context middleware exports
    │   │   ├── middleware.rs                 # Context extraction middleware
    │   │   ├── requirements.rs               # Context requirement levels
    │   │   ├── extractors/
    │   │   │   ├── mod.rs                    # Extractor exports
    │   │   │   ├── traits.rs                 # ContextExtractor trait
    │   │   │   ├── a2a_extractor.rs          # A2A protocol extractor
    │   │   │   └── header_extractor.rs       # Header-based extractor
    │   │   └── sources/
    │   │       ├── mod.rs                    # Source exports
    │   │       ├── headers.rs                # Header source
    │   │       └── payload.rs                # Payload source
    │   ├── cors.rs                           # CORS configuration
    │   ├── ip_ban.rs                         # IP ban middleware
    │   ├── jwt/
    │   │   ├── mod.rs                        # JWT middleware exports
    │   │   ├── context.rs                    # JWT context extraction
    │   │   └── token.rs                      # Token validation
    │   ├── rate_limit.rs                     # Rate limiting
    │   ├── session.rs                        # Session management
    │   ├── throttle.rs                       # Request throttling
    │   ├── trace.rs                          # Trace header injection
    │   └── trailing_slash.rs                 # Path normalization
    ├── proxy/
    │   ├── mod.rs                            # Proxy service exports
    │   ├── auth.rs                           # Proxy authentication
    │   ├── backend.rs                        # Request/response transform
    │   ├── client.rs                         # HTTP client pool
    │   ├── engine.rs                         # ProxyEngine core
    │   └── resolver.rs                       # Service endpoint resolution
    ├── server/
    │   ├── mod.rs                            # Server exports
    │   ├── builder.rs                        # ApiServer construction
    │   ├── readiness.rs                      # Readiness probe state
    │   ├── routes.rs                         # Route tree configuration
    │   ├── runner.rs                         # Server entry point
    │   └── lifecycle/
    │       ├── mod.rs                        # Lifecycle exports
    │       ├── agents.rs                     # Agent reconciliation
    │       ├── reconciliation.rs             # Service startup coordination
    │       └── scheduler.rs                  # Bootstrap job execution
    └── static_content/
        ├── mod.rs                            # Static content exports
        ├── config.rs                         # StaticContentMatcher
        ├── fallback.rs                       # 404 and SPA routing
        ├── homepage.rs                       # Homepage serving
        ├── session.rs                        # Static route sessions
        └── vite.rs                           # Vite asset serving
```

### Routes

| Module | Description |
|--------|-------------|
| `admin` | Administrative endpoints for CLI gateway and system management |
| `analytics` | Event tracking and real-time analytics streaming |
| `engagement` | User engagement metrics collection |
| `proxy` | Request forwarding to MCP servers and A2A agents |
| `stream` | Server-Sent Events for real-time context updates |
| `sync` | Database synchronization for offline-first clients |
| `wellknown` | Standard discovery endpoints (agent cards, OAuth metadata) |

### Services

| Module | Description |
|--------|-------------|
| `health` | Process monitoring and HTTP health checks |
| `middleware` | Request processing pipeline (auth, rate limiting, analytics) |
| `proxy` | HTTP client pooling and request transformation |
| `server` | Server lifecycle, route mounting, and startup coordination |
| `static_content` | SPA serving, content matching, and session handling |

## Usage

```toml
[dependencies]
systemprompt-api = "0.2.1"
```

```rust
use systemprompt_api::services::server::{run_server, setup_api_server};
use systemprompt_runtime::AppContext;

// Initialize and run
let ctx = AppContext::new().await?;
run_server(ctx, None).await?;
```

## Configuration

The API server is configured through `systemprompt-runtime::Config`:

- `api_external_url` - Public URL for the API
- `rate_limits` - Per-endpoint rate limit configuration
- `jwt_secret` - JWT signing secret
- `cors` - CORS allowed origins

## Notes

- No direct repository access in handlers (uses service injection)
- All routes mounted through `services/server/routes.rs`
- Middleware order is significant (see `services/server/builder.rs`)
- Static content requires prebuilt web assets in `WEB_DIR`

## Dependencies

### Internal Crates

- `systemprompt-runtime` - Application context and configuration
- `systemprompt-oauth` - Authentication and session management
- `systemprompt-agent` - Agent registry and orchestration
- `systemprompt-mcp` - MCP server registry and proxy
- `systemprompt-content` - Content repository and serving
- `systemprompt-analytics` - Session and event tracking
- `systemprompt-scheduler` - Background job execution
- `systemprompt-database` - Connection pooling
- `systemprompt-models` - Shared types and configuration
- `systemprompt-identifiers` - Type-safe ID wrappers
- `systemprompt-security` - Token extraction and validation
- `systemprompt-users` - User services and IP banning
- `systemprompt-events` - Event broadcasting
- `systemprompt-logging` - Structured logging
- `systemprompt-traits` - Shared traits and interfaces
- `systemprompt-files` - File system configuration
- `systemprompt-extension` - Extension loading

### External Crates

- `axum` - HTTP framework
- `tokio` - Async runtime
- `tower` - Middleware utilities
- `reqwest` - HTTP client
- `jsonwebtoken` - JWT handling
- `governor` - Rate limiting

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-api)** · **[docs.rs](https://docs.rs/systemprompt-api)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Entry layer · Own how your organization uses AI.</sub>

</div>
