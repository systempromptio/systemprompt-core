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

# systemprompt-security

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-security — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-security.svg?style=flat-square)](https://crates.io/crates/systemprompt-security)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-security?style=flat-square)](https://docs.rs/systemprompt-security)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Security infrastructure for systemprompt.io AI governance: JWT, OAuth2 token extraction, scope enforcement, the four-layer tool-call governance pipeline, the unified authz decision plane (deny-overrides resolver + `AuthzDecisionHook`) shared by gateway and MCP enforcement, Ed25519 bridge manifest signing, and bot/scanner detection.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides security primitives for the systemprompt.io platform. It handles JWT token generation and validation, multi-method token extraction, the unified authorization decision plane (resolver, repository, hook surface, audit sinks), Ed25519 manifest signing for the bridge, and bot/scanner classification.

## Architecture

```
src/
├── lib.rs                     # Module exports and public API
├── error.rs                   # AuthError, JwtError, ManifestSigningError
├── manifest_signing.rs        # Ed25519 signing + RFC 8785 JCS canonicalisation
├── auth/
│   ├── mod.rs                 # Auth module re-exports
│   ├── validation.rs          # AuthValidationService, AuthMode
│   └── hook_token.rs          # HookTokenValidator, ValidatedHookClaims
├── extraction/
│   ├── mod.rs                 # Extraction module re-exports
│   ├── token.rs               # TokenExtractor with fallback chain
│   ├── header.rs              # HeaderExtractor/HeaderInjector for context propagation
│   └── cookie.rs              # CookieExtractor for cookie-based auth
├── jwt/
│   └── mod.rs                 # JwtService for admin token generation
├── session/
│   ├── mod.rs                 # Session module re-exports
│   ├── generator.rs           # SessionGenerator for session token creation
│   └── claims.rs              # ValidatedSessionClaims data structure
├── services/
│   ├── mod.rs                 # Services module re-exports
│   └── scanner.rs             # ScannerDetector for bot detection
└── authz/
    ├── mod.rs                 # Authz module re-exports
    ├── config.rs              # AccessControlConfig, RuleEntry, DepartmentEntry
    ├── error.rs               # AuthzError, AuthzBootstrapError
    ├── extension.rs           # AuthzExtension (registers schemas + migrations)
    ├── hook.rs                # AuthzDecisionHook trait + Allow/Deny/Webhook impls
    ├── ingestion.rs           # AccessControlIngestionService
    ├── repository.rs          # AccessControlRepository, UpsertRuleParams
    ├── resolver.rs            # Deny-overrides resolve() entrypoint
    ├── runtime.rs             # build_authz_hook (config → SharedAuthzHook)
    ├── types.rs               # Access, AccessRule, AuthzDecision, AuthzRequest, EntityKind
    ├── audit/
    │   ├── mod.rs             # AuthzAuditSink, AuthzSource
    │   ├── db_sink.rs         # DbAuditSink (Postgres)
    │   └── repository.rs      # GovernanceDecisionRepository, insert_governance_decision
    └── schema/                # SQL DDL + migrations
```

### `auth`

Authentication validation with configurable enforcement modes plus bridge hook-token verification.

| Export | Type | Purpose |
|--------|------|---------|
| `AuthValidationService` | Struct | Validates JWT tokens and constructs `RequestContext` |
| `AuthMode` | Enum | `Required`, `Optional`, `Disabled` enforcement levels |
| `HookTokenValidator` | Struct | Verifies short-lived hook tokens minted for the bridge |
| `ValidatedHookClaims` | Struct | Claims extracted from a verified hook token |

### `extraction`

Token extraction from multiple sources with fallback chain support.

| Export | Type | Purpose |
|--------|------|---------|
| `TokenExtractor` | Struct | Extracts tokens from headers/cookies with configurable fallback |
| `ExtractionMethod` | Enum | `AuthorizationHeader`, `McpProxyHeader`, `Cookie` |
| `TokenExtractionError` | Enum | Specific error types for extraction failures |
| `HeaderExtractor` | Struct | Extracts trace_id, context_id, agent_name from headers |
| `HeaderInjector` | Struct | Injects RequestContext fields into outgoing headers |
| `HeaderInjectionError` | Struct | Error type for header injection failures |
| `CookieExtractor` | Struct | Dedicated cookie-based token extraction |
| `CookieExtractionError` | Enum | Cookie-specific error types |

### `jwt`

JWT token generation for administrative access.

| Export | Type | Purpose |
|--------|------|---------|
| `JwtService` | Struct | Generates admin JWT tokens with RS256, keyed off the active `TokenAuthority` |
| `AdminTokenParams` | Struct | Configuration for admin token creation |

### `services`

Security services for request classification.

| Export | Type | Purpose |
|--------|------|---------|
| `ScannerDetector` | Struct | Detects bot/scanner requests by path, user-agent, velocity |

### `session`

Session token generation and claim validation.

| Export | Type | Purpose |
|--------|------|---------|
| `SessionGenerator` | Struct | Generates session-scoped JWT tokens |
| `SessionParams` | Struct | Configuration for session token creation |
| `ValidatedSessionClaims` | Struct | Extracted claims after JWT validation |

### `authz`

Unified authorization decision plane shared by the gateway `/v1/messages` proxy and the MCP RBAC middleware.

| Export | Type | Purpose |
|--------|------|---------|
| `resolve` | Fn | Deny-overrides resolver against `access_control_rules` |
| `AuthzDecisionHook` | Trait | Pluggable decision surface; concrete impl is carried on `AppContext` as `SharedAuthzHook` |
| `AllowAllHook` / `DenyAllHook` / `WebhookHook` | Struct | Built-in `AuthzDecisionHook` implementations |
| `AccessControlRepository` | Struct | CRUD over `access_control_rules` |
| `AccessControlIngestionService` | Struct | Loads rule sets from configuration |
| `AuthzExtension` | Struct | Registers schemas and migrations via the extension framework |
| `AuthzAuditSink` / `DbAuditSink` / `NullAuditSink` | Trait + impls | Sinks for governance decision audit records |
| `GovernanceDecisionRepository` | Struct | Reads `governance_decisions` audit rows |
| `Access` / `AccessRule` / `AuthzDecision` / `AuthzRequest` / `Decision` / `EntityKind` / `RuleType` | Types | Authz request and decision data model |
| `build_authz_hook` | Fn | Constructs the active `SharedAuthzHook` (Deny/Allow/Webhook) from `governance.authz` for the caller to store on its context |
| `SharedAuthzHook` | Type alias | `Arc<dyn AuthzDecisionHook>` — the value threaded through DI |

### `manifest_signing`

Ed25519 signing for bridge manifests, keyed independently of the JWT signing key.

| Export | Type | Purpose |
|--------|------|---------|
| `sign_value<T: Serialize>` | Fn | RFC 8785 canonicalise + Ed25519 sign |
| `canonicalize<T: Serialize>` | Fn | RFC 8785 JCS canonical JSON |
| `signing_key` | Fn | Loads the Ed25519 signing key from `manifest_signing_secret_seed` |

## Usage

```toml
[dependencies]
systemprompt-security = "0.13.1"
```

### Token Extraction

```rust
use systemprompt_security::{TokenExtractor, ExtractionMethod};

let extractor = TokenExtractor::standard();
let token = extractor.extract(&headers)?;

let browser_extractor = TokenExtractor::browser_only()
    .with_cookie_name("auth_token".to_string());
```

### Authentication Validation

```rust
use systemprompt_security::{AuthValidationService, AuthMode};

let service = AuthValidationService::new(issuer, audiences);
let context = service.validate_request(&headers, AuthMode::Required)?;
```

Token signing and verification both resolve the active `kid` through the
process-wide `TokenAuthority` cache, which loads the deployment's RSA
private key from `signing_key_path` and fetches JWKS documents for every
entry in `profile.security.trusted_issuers`. There is no shared secret to
plumb through API surfaces.

### Admin Token Generation

```rust
use systemprompt_security::{JwtService, AdminTokenParams};

let params = AdminTokenParams {
    user_id: &user_id,
    session_id: &session_id,
    email: "admin@example.com",
    issuer: "systemprompt",
    duration: Duration::days(365),
    client_id: None,
};
let token = JwtService::generate_admin_token(&params)?;
```

### Session Token Generation

```rust
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};

let generator = SessionGenerator::new("systemprompt");
let params = SessionParams {
    user_id: &user_id,
    session_id: &session_id,
    email: "user@example.com",
    duration: Duration::hours(24),
    user_type: UserType::User,
    permissions: vec![Permission::Read, Permission::Write],
    roles: vec!["user".to_string()],
    rate_limit_tier: RateLimitTier::Standard,
};
let token = generator.generate(&params)?;
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Shared models (JwtClaims, UserType, Permission) |
| `systemprompt-identifiers` | Typed identifiers (UserId, SessionId, TraceId) |
| `systemprompt-config` | Profile and secrets access for signing keys |
| `systemprompt-database` | `DbPool` for authz repositories and audit sinks |
| `systemprompt-extension` | Extension trait used by `AuthzExtension` |
| `jsonwebtoken` | JWT encoding/decoding with RS256 |
| `ed25519-dalek` + `serde_jcs` | Ed25519 signing and RFC 8785 canonical JSON |
| `axum` | HTTP types (HeaderMap, HeaderValue) |
| `sqlx` | Authz repository and audit sink queries |
| `reqwest` | `WebhookHook` outbound calls |
| `inventory` | Extension registration |
| `chrono` | Timestamp handling |
| `tracing` | Structured logging |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-security)** · **[docs.rs](https://docs.rs/systemprompt-security)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
