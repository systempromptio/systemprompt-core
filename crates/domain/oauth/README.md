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

# systemprompt-oauth

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-oauth.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-oauth.svg">
    <img alt="systemprompt-oauth terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-oauth.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-oauth.svg?style=flat-square)](https://crates.io/crates/systemprompt-oauth)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-oauth?style=flat-square)](https://docs.rs/systemprompt-oauth)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

OAuth 2.0 / OIDC with PKCE, token introspection, and audience/issuer validation for systemprompt.io AI governance infrastructure. WebAuthn and JWT auth for the MCP governance pipeline with dynamic client registration, token revocation, and passwordless authentication.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [Compliance](https://systemprompt.io/features/compliance)

This crate implements a complete OAuth 2.0 authorization server with:

- Authorization Code Grant with PKCE
- Client Credentials Grant
- Refresh Token Grant
- Dynamic Client Registration (RFC 7591)
- Token Introspection (RFC 7662)
- Token Revocation (RFC 7009)
- WebAuthn/FIDO2 Passwordless Authentication
- OpenID Connect Discovery

## Usage

```toml
[dependencies]
systemprompt-oauth = "0.2.1"
```

```rust
pub use models::*;
pub use repository::OAuthRepository;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    extract_bearer_token, extract_cookie_token, is_browser_request, AnonymousSessionInfo,
    CreateAnonymousSessionInput, SessionCreationService, TemplateEngine, TokenValidator,
};
pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
```

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
- **generation**: Secure token and JWT generation
- **validation**: PKCE, client credentials, and JWT validation
- **webauthn**: FIDO2 passwordless authentication

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
| `JwtValidationProvider` | `JwtValidationProviderImpl` | Token validation |
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

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-oauth)** · **[docs.rs](https://docs.rs/systemprompt-oauth)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
