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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Every agent and tool call passes one decision plane before it runs. This crate authenticates the request, resolves it against your access-control rules with deny-overrides, and records the verdict, using signing keys you hold rather than a secret shared to a vendor.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

The crate owns the request-level security shared by the HTTP API and the runtime. Tokens are signed RS256 against an in-process signing-key authority that holds the deployment's RSA private key and caches federated JWKS documents under a bounded LRU with an HTTPS allowlist. HS256 is rejected on validation. Authorization runs the same deny-overrides resolver for the gateway `/v1/messages` proxy and the MCP RBAC middleware, so one audit shape covers both enforcement sites. There is no shared secret to plumb through API surfaces.

## Modules

| Module | Purpose |
|--------|---------|
| `keys` | The signing-key plane. `RsaSigningKey` (generate, load/persist PKCS#8 PEM, deterministic `kid`), the process-wide `authority` submodule (`init`, `signing_key`, `encoding_key`, `active_kid`, `decoding_key_for_kid`), the `jwks` document builder, and the `jwks_client` federated fetcher with cache. |
| `jwt` | Admin-token minting and validation (`decode`, `mint`, `validate` submodules); `JwtService`, `AdminTokenParams`. |
| `session` | Session-scoped token generation and claim validation (`SessionGenerator`, `SessionParams`, `ValidatedSessionClaims`). |
| `extraction` | Token extraction from `Authorization` headers, MCP proxy headers, and cookies, plus header injection for context propagation. |
| `auth` | Request validation into a `RequestContext` (`AuthValidationService`) and bridge hook-token verification. |
| `authz` | Unified authorization decision plane: the deny-overrides `resolve`, the `access_control_rules` repository, the `AuthzDecisionHook` surface with built-in impls, ingestion, and Postgres audit sinks. |
| `policy` | Shared tool-use governance types (`GovernancePolicy`, `GovernanceChain`, `PolicyContext`) that produce the same `Decision` shape the authz resolver returns. |
| `at_rest` | HMAC-SHA256 keyed hashing (`hmac_sha256`, `hmac_sha256_hex`) under the deployment `oauth_at_rest_pepper`, used to store refresh-token ids and authorisation codes as digests rather than plaintext. |
| `manifest_signing` | Ed25519 bridge-manifest signing with RFC 8785 JCS canonicalisation, keyed independently of the JWT signing key. |
| `services` | Lightweight scanner / bot detection (`ScannerDetector`). |
| `error` | `AuthError`, `JwtError`, `ManifestSigningError` and their result aliases. |

Authz schema DDL and migrations live in `authz/schema/`.

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
| `JwtService` | Struct | Generates admin JWT tokens with RS256, keyed off the active signing-key authority (`keys::authority`) |
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

### `keys`

The asymmetric signing-key plane. `RsaSigningKey` holds the active keypair; the `authority` submodule exposes it process-wide.

| Export | Type | Purpose |
|--------|------|---------|
| `RsaSigningKey` | Struct | RSA keypair: generate, load/persist PKCS#8 PEM, deterministic `kid` |
| `authority::{init, signing_key, encoding_key, active_kid, decoding_key_for_kid}` | Fn | Process-wide access to the active signing key and per-`kid` decoding keys |
| `Jwks` / `Jwk` | Struct | JWKS document served at `/.well-known/jwks.json` |
| `JwksClient` | Struct | Federated JWKS fetcher with bounded LRU cache and HTTPS allowlist |

### `at_rest`

Keyed hashing so sensitive OAuth identifiers are stored as digests, never plaintext.

| Export | Type | Purpose |
|--------|------|---------|
| `hmac_sha256` | Fn | HMAC-SHA256 digest of a value under the `oauth_at_rest_pepper` |
| `hmac_sha256_hex` | Fn | Hex-encoded HMAC-SHA256 digest |

### `policy`

Shared tool-use governance types that produce the same `Decision` shape as the authz resolver.

| Export | Type | Purpose |
|--------|------|---------|
| `GovernancePolicy` | Trait | Contract every tool-call policy in the chain implements |
| `GovernanceChain` | Struct | Ordered chain of policies (secret scan, scope check, blocklist, rate limit) |
| `PolicyContext` / `McpToolInput` / `AgentScope` | Types | Inputs a policy evaluates against |

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
systemprompt-security = "0.21"
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
process-wide signing-key authority (`keys::authority`), which loads the
deployment's RSA private key from `signing_key_path` and fetches JWKS
documents for every entry in `profile.security.trusted_issuers`. There is
no shared secret to plumb through API surfaces.

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
| `rsa`, `pkcs8` | RSA keypair generation and PKCS#8 PEM encoding |
| `sha2`, `hmac`, `hex` | `kid` derivation and HMAC-SHA256 at-rest hashing |
| `lru` | Bounded JWKS document cache |
| `url` | JWKS issuer HTTPS-allowlist parsing |
| `ed25519-dalek` + `serde_jcs` | Ed25519 signing and RFC 8785 canonical JSON |
| `base64`, `serde`, `serde_json` | Encoding and serialisation |
| `axum` | HTTP types (HeaderMap, HeaderValue) |
| `sqlx` | Authz repository and audit sink queries |
| `reqwest` | `WebhookHook` outbound calls |
| `metrics` | Authz decision counters |
| `uuid` | Token and record identifiers |
| `inventory` | Extension registration |
| `chrono` | Timestamp handling |
| `async-trait`, `thiserror` | `dyn`-compatible authz hooks and typed errors |
| `tracing` | Structured logging |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-security)** · **[docs.rs](https://docs.rs/systemprompt-security)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
