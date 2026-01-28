<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-api

HTTP API gateway for systemprompt.io OS.

## Overview

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

## File Structure

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

## Module Descriptions

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

## Usage

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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-api = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
