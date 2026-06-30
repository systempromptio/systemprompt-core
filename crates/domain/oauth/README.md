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
systemprompt-oauth = "0.17.1"
```

```rust
use systemprompt_oauth::{
    OAuthRepository, OAuthState, OauthError, OauthExtension, OauthResult,
    SessionCreationService, TokenValidator, validate_jwt_token,
};
use systemprompt_oauth::services::{
    issue_bridge_access, issue_bridge_exchange_code, exchange_bridge_session_code,
};
```

## File Structure

```
src/
├── lib.rs                              # Crate root, public exports
├── constants.rs                        # Shared constants (TTLs, claims, headers)
├── error.rs                            # OauthError / OauthResult
├── extension.rs                        # OauthExtension (schemas + migrations)
├── state.rs                            # OAuthState handle
├── models/                             # Data structures
│   ├── mod.rs                          # Model exports
│   ├── analytics.rs                    # Session / login analytics types
│   ├── cimd.rs                         # Client-Initiated Metadata Discovery types
│   ├── clients/                        # OAuth client models
│   │   ├── mod.rs                      # OAuthClient, OAuthClientRow, ClientRelations
│   │   └── api.rs                      # Create/Update/Response DTOs
│   └── oauth/                          # OAuth protocol models
│       ├── mod.rs                      # GrantType, PkceMethod, JwtClaims, ResponseType...
│       ├── api.rs                      # Pagination types
│       └── dynamic_registration.rs     # RFC 7591 request / response
├── queries/                            # SQL query layer
│   ├── mod.rs
│   └── postgres/
│       └── mod.rs                      # PostgreSQL query implementations
├── repository/                         # Data access layer
│   ├── mod.rs                          # Repository exports
│   ├── bridge_host_prefs.rs            # Per-host bridge enable/disable
│   ├── bridge_session.rs               # Bridge heartbeat sessions
│   ├── exchange_code.rs                # Bridge exchange-code persistence
│   ├── setup_token.rs                  # Bootstrap / admin setup tokens
│   ├── webauthn.rs                     # WebAuthn credential storage
│   ├── client/                         # Client repository
│   │   ├── mod.rs                      # ClientRepository
│   │   ├── queries.rs                  # Read operations
│   │   ├── mutations.rs                # Write operations
│   │   ├── inserts.rs                  # Bulk insert helpers
│   │   ├── relations.rs                # Load client relations
│   │   └── cleanup.rs                  # Stale client cleanup
│   └── oauth/                          # OAuth repository
│       ├── mod.rs                      # OAuthRepository
│       ├── auth_code.rs                # Authorization codes
│       ├── refresh_token.rs            # Refresh tokens
│       ├── scopes.rs                   # Scope validation
│       ├── user.rs                     # User retrieval
│       └── cleanup.rs                  # Expired-record cleanup
└── services/                           # Business logic
    ├── mod.rs                          # Service exports
    ├── bridge.rs                       # Bridge access tokens + exchange codes
    ├── generation.rs                   # Token / JWT / secret generation
    ├── http.rs                         # HTTP utilities (bearer / cookie extraction)
    ├── providers.rs                    # JwtValidationProviderImpl
    ├── templating.rs                   # HTML template rendering
    ├── cimd/                           # Client metadata validation
    │   ├── mod.rs
    │   ├── fetcher.rs                  # Metadata URL fetching
    │   └── validator.rs                # Metadata validation
    ├── jwt/                            # JWT handling
    │   ├── mod.rs                      # TokenValidator trait, AuthService
    │   ├── authentication.rs           # Token authentication
    │   └── authorization.rs            # Permission authorization
    ├── session/                        # Session management
    │   ├── mod.rs                      # SessionCreationService
    │   ├── lookup.rs                   # Session lookup / reuse
    │   └── creation.rs                 # New session creation
    ├── validation/                     # Request validation
    │   ├── mod.rs
    │   ├── audience.rs                 # JWT audience validation
    │   ├── client_credentials.rs       # Client secret validation
    │   ├── jwt.rs                      # JWT token validation
    │   ├── oauth_params.rs             # OAuth parameter validation
    │   └── redirect_uri.rs             # Redirect URI validation
    └── webauthn/                       # WebAuthn / FIDO2 service
        ├── mod.rs
        ├── config.rs                   # WebAuthnConfig
        ├── jwt.rs                      # JwtTokenValidator for WebAuthn
        ├── registry.rs                 # Credential registry
        ├── token.rs                    # WebAuthn token helpers
        ├── user_service.rs             # UserCreationService
        └── service/                    # WebAuthn operations
            ├── mod.rs                  # WebAuthnService
            ├── authentication.rs       # Authentication flow
            ├── credentials.rs          # Credential operations
            ├── link.rs                 # Account linking
            └── registration.rs         # Registration flow
```

## Module Descriptions

### models/
Data structures for OAuth clients, tokens, JWT claims, CIMD metadata, and analytics. Includes typed enums for grant types, response types, and PKCE methods.

### queries/
PostgreSQL query implementations using compile-time-verified `sqlx` macros.

### repository/
Data access layer with separate repositories for clients (`ClientRepository`), OAuth protocol records (`OAuthRepository`), bridge sessions (`BridgeSessionRepository`), bridge host preferences (`BridgeHostPrefsRepository`), exchange codes, setup tokens, and WebAuthn credentials.

### services/
Business logic including:
- **bridge**: Bridge access-token issuance and short-lived exchange codes for the desktop bridge.
- **cimd**: Client-Initiated Metadata Discovery fetcher and validator.
- **generation**: Secure token, JWT, and client-secret generation.
- **jwt**: `TokenValidator` and `AuthService` for token authentication and authorisation.
- **providers**: `JwtValidationProviderImpl` implementing the `JwtValidationProvider` trait.
- **session**: Anonymous and authenticated session creation and lookup.
- **templating**: HTML template rendering for the OAuth consent / login pages.
- **validation**: Audience, client-credential, JWT, redirect-URI, and OAuth-parameter validation.
- **webauthn**: FIDO2 passwordless authentication, registration, and account linking.

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
| `bridge_exchange_codes` | Short-lived bridge session exchange codes |
| `bridge_sessions` | Bridge heartbeat / active-session records |
| `setup_tokens` | Bootstrap and admin setup tokens |
| `webauthn_credentials` | FIDO2 / WebAuthn credentials |
| `webauthn_challenges` | WebAuthn challenge storage |

## Trait Implementations

Implements traits from `systemprompt-traits`:

| Trait | Implementation | Purpose |
|-------|----------------|---------|
| `JwtValidationProvider` | `JwtValidationProviderImpl` | Token validation |
| `UserProvider` | Consumed via `Arc<dyn UserProvider>` | User lookup |

## Dependencies

### Internal Crates
- `systemprompt-config` — Profile and config loading
- `systemprompt-database` — `DbPool` and SQLx abstraction
- `systemprompt-extension` — Extension framework
- `systemprompt-logging` — Tracing setup
- `systemprompt-security` — Crypto and auth primitives

### Shared Crates
- `systemprompt-traits` — Auth and provider traits
- `systemprompt-models` — Shared domain types
- `systemprompt-identifiers` — Typed identifiers (with `sqlx` feature)

### External
- `jsonwebtoken` — JWT encoding / decoding
- `bcrypt` — Password and secret hashing
- `webauthn-rs` — FIDO2 / WebAuthn
- `axum`, `http`, `reqwest` — HTTP server and client types
- `sqlx` — Compile-time-verified PostgreSQL queries
- `validator`, `rand`, `base64`, `sha2` — Validation and crypto helpers

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
