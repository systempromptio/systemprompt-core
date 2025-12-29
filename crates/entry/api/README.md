# API Module

HTTP API gateway for SystemPrompt OS.

## Structure

```
src/
├── lib.rs                          # Crate exports
├── models/
│   └── mod.rs                      # ServerConfig
├── api/
│   ├── mod.rs                      # API module exports
│   └── routes/
│       ├── mod.rs                  # Route aggregation
│       ├── wellknown.rs            # /.well-known/* endpoints (agent cards)
│       ├── proxy/
│       │   ├── mod.rs              # Proxy route exports
│       │   ├── agents.rs           # Agent proxy routes
│       │   └── mcp.rs              # MCP proxy routes, execution lookup
│       ├── stream/
│       │   ├── mod.rs              # SSE stream handlers (A2A, AgUI)
│       │   └── contexts.rs         # Context state streaming
│       └── sync/
│           ├── mod.rs              # Database sync exports
│           ├── types.rs            # Request/response types
│           ├── export.rs           # Database export endpoint
│           └── import.rs           # Database import endpoint
└── services/
    ├── mod.rs                      # Service exports
    ├── middleware/
    │   ├── mod.rs                  # Middleware chain
    │   ├── analytics.rs            # Request tracking, metrics
    │   ├── auth.rs                 # Route-level authentication
    │   ├── bot_detector.rs         # Bot/crawler identification
    │   ├── cors.rs                 # CORS headers
    │   ├── rate_limit.rs           # Rate limiting per endpoint type
    │   ├── session.rs              # Session creation/lookup
    │   ├── trailing_slash.rs       # Path normalization
    │   └── jwt/
    │       ├── mod.rs              # JWT processing
    │       ├── token.rs            # Token extraction from headers
    │       └── context.rs          # RequestContext from JWT claims
    ├── proxy/
    │   ├── mod.rs                  # Proxy exports
    │   ├── engine.rs               # ProxyEngine - request forwarding
    │   ├── resolver.rs             # Service endpoint resolution
    │   ├── auth.rs                 # OAuth validation for proxy
    │   ├── backend.rs              # Request/response transformation
    │   └── client.rs               # HTTP client connection pool
    ├── server/
    │   ├── mod.rs                  # Server exports
    │   ├── builder.rs              # ApiServer construction
    │   ├── runner.rs               # Server entry point
    │   ├── routes.rs               # Route tree configuration
    │   ├── readiness.rs            # Readiness probe state
    │   └── lifecycle/
    │       ├── mod.rs              # Lifecycle exports
    │       ├── agents.rs           # Agent process reconciliation
    │       ├── reconciliation.rs   # Service startup coordination
    │       └── scheduler.rs        # Bootstrap job execution
    └── static_content/
        ├── mod.rs                  # Static serving exports
        ├── config.rs               # StaticContentMatcher configuration
        ├── vite.rs                 # SPA/Vite asset serving
        ├── fallback.rs             # 404 and SPA routing
        └── session.rs              # Static route session handling
```

## Dependencies

Internal crates:
- systemprompt-core-system (AppContext)
- systemprompt-core-users (user lookup)
- systemprompt-core-oauth (session, auth)
- systemprompt-core-logging (CLI output, analytics)
- systemprompt-core-agent (agent registry, orchestration)
- systemprompt-core-mcp (MCP registry)
- systemprompt-core-content (content serving)
- systemprompt-core-database (connection pool)
- systemprompt-core-scheduler (job execution)

## Notes

- No repository directory: delegates persistence to domain modules
- Uses CliService for CLI output during startup
- Uses tracing for request logging
