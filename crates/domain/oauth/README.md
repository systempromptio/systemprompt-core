<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-oauth

OAuth 2.0 authentication and authorization module for systemprompt.io OS.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-oauth.svg)](https://crates.io/crates/systemprompt-oauth)
[![Documentation](https://docs.rs/systemprompt-oauth/badge.svg)](https://docs.rs/systemprompt-oauth)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**

This crate implements a complete OAuth 2.0 authorization server with:

- Authorization Code Grant with PKCE
- Client Credentials Grant
- Refresh Token Grant
- Dynamic Client Registration (RFC 7591)
- Token Introspection (RFC 7662)
- Token Revocation (RFC 7009)
- WebAuthn/FIDO2 Passwordless Authentication
- OpenID Connect Discovery

## File Structure

```
src/
├── lib.rs                              # Crate root, public exports
├── api/                                # HTTP API layer
│   ├── mod.rs                          # API module exports
│   ├── wellknown.rs                    # /.well-known/openid-configuration
│   └── routes/                         # Axum route handlers
│       ├── mod.rs                      # Routes module
│       ├── core.rs                     # Core OAuth router
│       ├── health.rs                   # Health check endpoint
│       ├── discovery.rs                # OpenID Connect discovery
│       ├── clients.rs                  # Client routes registration
│       ├── client/                     # Client management CRUD
│       │   ├── mod.rs
│       │   ├── create.rs               # POST /clients
│       │   ├── get.rs                  # GET /clients/{id}
│       │   ├── list.rs                 # GET /clients
│       │   ├── update.rs               # PUT /clients/{id}
│       │   └── delete.rs               # DELETE /clients/{id}
│       ├── oauth/                      # OAuth 2.0 endpoints
│       │   ├── mod.rs
│       │   ├── anonymous.rs            # Anonymous session tokens
│       │   ├── callback.rs             # OAuth callback handler
│       │   ├── consent.rs              # User consent screen
│       │   ├── introspect.rs           # Token introspection (RFC 7662)
│       │   ├── register.rs             # Dynamic client registration
│       │   ├── revoke.rs               # Token revocation (RFC 7009)
│       │   ├── userinfo.rs             # UserInfo endpoint
│       │   ├── webauthn_complete.rs    # WebAuthn OAuth completion
│       │   ├── authorize/              # Authorization endpoint
│       │   │   ├── mod.rs
│       │   │   ├── handler.rs          # Authorization request handler
│       │   │   ├── response_builder.rs # Authorization response builder
│       │   │   └── validation.rs       # Request validation, PKCE entropy
│       │   ├── client_config/          # Client configuration management
│       │   │   ├── mod.rs
│       │   │   ├── get.rs
│       │   │   ├── update.rs
│       │   │   ├── delete.rs
│       │   │   └── validation.rs
│       │   └── token/                  # Token endpoint
│       │       ├── mod.rs              # Token request/response types
│       │       ├── handler.rs          # Token grant handlers
│       │       ├── generation.rs       # JWT token generation
│       │       └── validation.rs       # Client credentials validation
│       └── webauthn/                   # WebAuthn/FIDO2 endpoints
│           ├── mod.rs
│           ├── authenticate.rs         # WebAuthn authentication
│           └── register/               # WebAuthn registration
│               ├── mod.rs
│               ├── start.rs            # Registration challenge
│               └── finish.rs           # Registration completion
├── models/                             # Data structures
│   ├── mod.rs                          # Model exports
│   ├── analytics.rs                    # Analytics data types
│   ├── cimd.rs                         # Client Identity Metadata
│   ├── clients/                        # Client models
│   │   ├── mod.rs                      # OAuthClient, OAuthClientRow
│   │   └── api.rs                      # API request/response types
│   └── oauth/                          # OAuth models
│       ├── mod.rs                      # GrantType, PkceMethod, JwtClaims
│       ├── api.rs                      # Pagination types
│       └── dynamic_registration.rs     # RFC 7591 types
├── queries/                            # SQL queries
│   ├── mod.rs
│   └── postgres/
│       └── mod.rs                      # PostgreSQL query implementations
├── repository/                         # Data access layer
│   ├── mod.rs                          # Repository exports
│   ├── webauthn.rs                     # WebAuthn credential storage
│   ├── client/                         # Client repository
│   │   ├── mod.rs                      # ClientRepository struct
│   │   ├── queries.rs                  # Read operations
│   │   ├── mutations.rs                # Write operations (create/update/delete)
│   │   ├── inserts.rs                  # Bulk insert helpers
│   │   ├── relations.rs                # Load client relations
│   │   └── cleanup.rs                  # Stale client cleanup
│   └── oauth/                          # OAuth repository
│       ├── mod.rs                      # OAuthRepository struct
│       ├── auth_code.rs                # Authorization code operations
│       ├── refresh_token.rs            # Refresh token operations
│       ├── scopes.rs                   # Scope validation
│       └── user.rs                     # User retrieval
└── services/                           # Business logic
    ├── mod.rs                          # Service exports
    ├── auth_provider.rs                # JwtAuthProvider, JwtAuthorizationProvider
    ├── generation.rs                   # Token generation utilities
    ├── http.rs                         # HTTP utilities
    ├── templating.rs                   # HTML template rendering
    ├── cimd/                           # Client metadata validation
    │   ├── mod.rs
    │   ├── fetcher.rs                  # Metadata URL fetching
    │   └── validator.rs                # Metadata validation
    ├── jwt/                            # JWT handling
    │   ├── mod.rs                      # TokenValidator trait
    │   ├── authentication.rs           # Token authentication
    │   └── authorization.rs            # Permission authorization
    ├── session/                        # Session management
    │   ├── mod.rs                      # SessionCreationService
    │   ├── lookup.rs                   # Session lookup/reuse
    │   └── creation.rs                 # New session creation
    ├── validation/                     # Request validation
    │   ├── mod.rs
    │   ├── audience.rs                 # JWT audience validation
    │   ├── client_credentials.rs       # Client secret validation
    │   ├── jwt.rs                      # JWT token validation
    │   ├── oauth_params.rs             # OAuth parameter validation
    │   └── redirect_uri.rs             # Redirect URI validation
    └── webauthn/                       # WebAuthn/FIDO2 service
        ├── mod.rs
        ├── config.rs                   # WebAuthn configuration
        ├── jwt.rs                      # JWT for WebAuthn
        ├── manager.rs                  # Credential manager
        ├── user_service.rs             # User provider integration
        └── service/                    # WebAuthn operations
            ├── mod.rs                  # WebAuthnService
            ├── authentication.rs       # Authentication flow
            ├── credentials.rs          # Credential operations
            └── registration.rs         # Registration flow
```

## Module Descriptions

### api/
HTTP API layer implementing OAuth 2.0 endpoints per RFC 6749, 7009, 7591, 7662.

### models/
Data structures for OAuth clients, tokens, and JWT claims. Includes typed enums for grant types, response types, and PKCE methods.

### queries/
SQL query definitions. PostgreSQL-specific implementations using sqlx macros.

### repository/
Data access layer with separate repositories for clients, OAuth operations, and WebAuthn credentials. All SQL uses compile-time verified sqlx macros.

### services/
Business logic including:
- **auth_provider**: Trait implementations for `AuthProvider` and `AuthorizationProvider`
- **generation**: Secure token and JWT generation
- **validation**: PKCE, client credentials, and JWT validation
- **webauthn**: FIDO2 passwordless authentication

## Public Exports

```rust
pub use models::*;
pub use repository::OAuthRepository;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    extract_bearer_token, extract_cookie_token, is_browser_request, AnonymousSessionInfo,
    CreateAnonymousSessionInput, JwtAuthProvider, JwtAuthorizationProvider,
    SessionCreationService, TemplateEngine, TokenValidator, TraitBasedAuthService,
};
pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
```

## Database Tables

| Table | Purpose |
|-------|---------|
| `oauth_clients` | Registered OAuth clients |
| `oauth_client_redirect_uris` | Allowed redirect URIs per client |
| `oauth_client_grant_types` | Supported grant types per client |
| `oauth_client_response_types` | Supported response types per client |
| `oauth_client_scopes` | Allowed scopes per client |
| `oauth_client_contacts` | Contact emails per client |
| `oauth_auth_codes` | Authorization codes (600s TTL) |
| `oauth_refresh_tokens` | Refresh tokens |
| `webauthn_credentials` | FIDO2/WebAuthn credentials |
| `webauthn_challenges` | WebAuthn challenge storage |

## Trait Implementations

Implements traits from `systemprompt-traits`:

| Trait | Implementation | Purpose |
|-------|----------------|---------|
| `AuthProvider` | `JwtAuthProvider` | Token validation |
| `AuthorizationProvider` | `JwtAuthorizationProvider` | Permission checks |
| `UserProvider` | Consumed via `Arc<dyn UserProvider>` | User lookup |

## Dependencies

### Internal Crates
- `systemprompt-runtime` - AppContext, Config
- `systemprompt-users` - UserProviderImpl
- `systemprompt-logging` - Logging infrastructure
- `systemprompt-database` - DbPool
- `systemprompt-analytics` - Session analytics

### Shared Crates
- `systemprompt-traits` - Auth traits
- `systemprompt-models` - Shared types
- `systemprompt-identifiers` - Typed identifiers

### External
- `jsonwebtoken` - JWT encoding/decoding
- `bcrypt` - Password hashing
- `webauthn-rs` - FIDO2/WebAuthn
- `axum` - HTTP framework

## Security Features

- PKCE required for authorization code flow
- S256 challenge method enforced (plain disallowed)
- Entropy validation for code challenges
- Constant-time client secret comparison
- Secure cookie attributes (HttpOnly, Secure, SameSite)
- Token revocation support
- WebAuthn for passwordless authentication

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-oauth = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
