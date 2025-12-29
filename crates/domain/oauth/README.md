# OAuth Module

OAuth 2.0 authorization server with WebAuthn passwordless authentication.

## Structure

```
oauth/
├── Cargo.toml
├── README.md
├── status.md                        # Review status (116 checks)
└── src/
    ├── lib.rs                       # Module exports
    ├── api/                         # HTTP API layer
    │   ├── mod.rs
    │   ├── wellknown.rs             # /.well-known/openid-configuration
    │   └── routes/                  # Axum route handlers
    │       ├── mod.rs
    │       ├── core.rs              # Core OAuth routes
    │       ├── health.rs            # Health check endpoint
    │       ├── discovery.rs         # OpenID Connect discovery
    │       ├── clients.rs           # Client routes registration
    │       ├── client/              # Client management endpoints
    │       │   ├── mod.rs
    │       │   ├── create.rs        # POST /clients
    │       │   ├── get.rs           # GET /clients/{id}
    │       │   ├── list.rs          # GET /clients
    │       │   ├── update.rs        # PUT /clients/{id}
    │       │   └── delete.rs        # DELETE /clients/{id}
    │       ├── oauth/               # OAuth 2.0 endpoints
    │       │   ├── mod.rs
    │       │   ├── anonymous.rs     # Anonymous session token
    │       │   ├── callback.rs      # OAuth callback handler
    │       │   ├── consent.rs       # User consent screen
    │       │   ├── introspect.rs    # Token introspection
    │       │   ├── register.rs      # Dynamic client registration
    │       │   ├── revoke.rs        # Token revocation
    │       │   ├── userinfo.rs      # User info endpoint
    │       │   ├── webauthn_complete.rs
    │       │   ├── authorize/       # Authorization endpoint
    │       │   │   ├── mod.rs
    │       │   │   ├── handler.rs
    │       │   │   ├── response_builder.rs
    │       │   │   └── validation.rs
    │       │   ├── client_config/   # Client configuration
    │       │   │   ├── mod.rs
    │       │   │   ├── get.rs
    │       │   │   ├── update.rs
    │       │   │   ├── delete.rs
    │       │   │   └── validation.rs
    │       │   └── token/           # Token endpoint
    │       │       ├── mod.rs
    │       │       ├── handler.rs
    │       │       ├── generation.rs
    │       │       └── validation.rs
    │       └── webauthn/            # WebAuthn/FIDO2 endpoints
    │           ├── mod.rs
    │           ├── authenticate.rs
    │           └── register/
    │               ├── mod.rs
    │               ├── start.rs
    │               └── finish.rs
    ├── models/                      # Data structures
    │   ├── mod.rs
    │   ├── analytics.rs             # Analytics data types
    │   ├── cimd.rs                  # Client Identity & Metadata
    │   ├── clients/
    │   │   ├── mod.rs               # OAuthClient, OAuthClientRow
    │   │   └── api.rs               # API request/response types
    │   └── oauth/
    │       ├── mod.rs               # GrantType, PkceMethod, ResponseType, etc.
    │       ├── api.rs               # Pagination types
    │       └── dynamic_registration.rs
    ├── queries/                     # SQL queries
    │   ├── mod.rs
    │   ├── postgres/
    │   │   └── mod.rs
    │   └── seed/                    # SQL seed files
    │       ├── test_client.sql
    │       ├── webauthn_client.sql
    │       └── webauthn_client_scopes.sql
    ├── repository/                  # Data access layer
    │   ├── mod.rs
    │   ├── webauthn.rs              # WebAuthn credential operations
    │   ├── client/                  # Client management
    │   │   ├── mod.rs               # ClientRepository
    │   │   ├── queries.rs           # Read operations
    │   │   ├── mutations.rs         # Write operations
    │   │   ├── relations.rs         # Load client relations
    │   │   └── cleanup.rs           # Stale client cleanup
    │   └── oauth/                   # OAuth repository
    │       ├── mod.rs               # OAuthRepository
    │       ├── auth_code.rs         # Authorization code operations
    │       ├── refresh_token.rs     # Refresh token operations
    │       ├── scopes.rs            # Scope operations
    │       └── user.rs              # User retrieval
    └── services/                    # Business logic
        ├── mod.rs
        ├── auth_provider.rs         # JwtAuthProvider, JwtAuthorizationProvider
        ├── generation.rs            # Token generation utilities
        ├── http.rs                  # HTTP utilities
        ├── templating.rs            # HTML template rendering
        ├── cimd/                    # Client metadata validation
        │   ├── mod.rs
        │   ├── fetcher.rs
        │   └── validator.rs
        ├── jwt/                     # JWT handling
        │   ├── mod.rs
        │   ├── authentication.rs
        │   ├── authorization.rs
        │   └── extraction.rs
        ├── session/                 # Session management
        │   └── mod.rs               # SessionCreationService
        ├── validation/              # Request validation
        │   ├── mod.rs
        │   ├── audience.rs
        │   ├── jwt.rs
        │   ├── oauth_params.rs
        │   └── redirect_uri.rs
        └── webauthn/                # WebAuthn/FIDO2 service
            ├── mod.rs
            ├── config.rs
            ├── jwt.rs
            ├── manager.rs
            ├── user_service.rs      # Uses UserProvider trait
            └── service/
                ├── mod.rs
                ├── authentication.rs
                ├── credentials.rs
                └── registration.rs
```

## Key Files

| File | Purpose |
|------|---------|
| `lib.rs` | Public API exports |
| `models/oauth/mod.rs` | OAuth enums (GrantType, PkceMethod, etc.) |
| `repository/oauth/mod.rs` | OAuthRepository with client and token operations |
| `repository/client/mutations.rs` | Client CRUD operations |
| `services/auth_provider.rs` | Trait implementations for auth |
| `services/webauthn/service/mod.rs` | WebAuthn authentication service |

## Database Tables

- `oauth_clients` - Registered OAuth clients
- `oauth_client_redirect_uris` - Allowed redirect URIs
- `oauth_client_grant_types` - Supported grant types
- `oauth_client_response_types` - Supported response types
- `oauth_client_scopes` - Allowed scopes
- `oauth_client_contacts` - Contact emails
- `oauth_auth_codes` - Authorization codes (600s TTL)
- `oauth_refresh_tokens` - Refresh tokens
- `webauthn_credentials` - FIDO2 credentials
- `webauthn_challenges` - WebAuthn challenges

## Public Exports

```rust
pub use models::*;
pub use repository::OAuthRepository;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    extract_bearer_token, extract_cookie_token, AnonymousSessionInfo,
    BrowserRedirectService, JwtAuthProvider, JwtAuthorizationProvider,
    SessionCreationService, TemplateEngine, TokenValidator, TraitBasedAuthService,
};
pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
```

## Trait Implementations

From `systemprompt-traits`:

- `AuthProvider` - Token validation via `JwtAuthProvider`
- `AuthorizationProvider` - Permission checks via `JwtAuthorizationProvider`
- `UserProvider` - User lookup (consumed via `Arc<dyn UserProvider>`)

## Dependencies

Internal:
- `systemprompt-core-system` - AppContext, Config
- `systemprompt-core-users` - UserProviderImpl
- `systemprompt-core-logging` - Logging
- `systemprompt-core-database` - DbPool

Shared:
- `systemprompt-traits` - Auth traits
- `systemprompt-models` - Shared types
- `systemprompt-identifiers` - Typed identifiers
