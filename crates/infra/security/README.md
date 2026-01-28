<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-security

Security module for systemprompt.io - authentication, authorization, JWT, and token extraction.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-security.svg)](https://crates.io/crates/systemprompt-security)
[![Documentation](https://docs.rs/systemprompt-security/badge.svg)](https://docs.rs/systemprompt-security)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Infra layer in the systemprompt.io architecture.**

This crate provides security primitives for the systemprompt.io platform. It handles JWT token generation and validation, multi-method token extraction, and bot/scanner detection.

## File Structure

```
src/
├── lib.rs                     # Module exports and public API
├── auth/
│   ├── mod.rs                 # Auth module re-exports
│   └── validation.rs          # AuthValidationService, AuthMode, token validation
├── extraction/
│   ├── mod.rs                 # Extraction module re-exports
│   ├── token.rs               # TokenExtractor with fallback chain
│   ├── header.rs              # HeaderExtractor/HeaderInjector for context propagation
│   └── cookie.rs              # CookieExtractor for cookie-based auth
├── jwt/
│   └── mod.rs                 # JwtService for admin token generation
├── services/
│   ├── mod.rs                 # Services module re-exports
│   └── scanner.rs             # ScannerDetector for bot detection
└── session/
    ├── mod.rs                 # Session module re-exports
    ├── generator.rs           # SessionGenerator for session token creation
    └── claims.rs              # ValidatedSessionClaims data structure
```

## Modules

### `auth`

Authentication validation service with configurable enforcement modes.

| Export | Type | Purpose |
|--------|------|---------|
| `AuthValidationService` | Struct | Validates JWT tokens and creates RequestContext |
| `AuthMode` | Enum | `Required`, `Optional`, `Disabled` enforcement levels |

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
| `JwtService` | Struct | Generates admin JWT tokens with HS256 |
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

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Shared models (JwtClaims, UserType, Permission) |
| `systemprompt-identifiers` | Typed identifiers (UserId, SessionId, TraceId) |
| `jsonwebtoken` | JWT encoding/decoding with HS256 |
| `axum` | HTTP types (HeaderMap, HeaderValue) |
| `chrono` | Timestamp handling |
| `tracing` | Debug logging for extraction errors |

## Usage

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

let service = AuthValidationService::new(secret, issuer, audiences);
let context = service.validate_request(&headers, AuthMode::Required)?;
```

### Admin Token Generation

```rust
use systemprompt_security::{JwtService, AdminTokenParams};

let params = AdminTokenParams {
    user_id: &user_id,
    session_id: &session_id,
    email: "admin@example.com",
    jwt_secret: &secret,
    issuer: "systemprompt",
    duration: Duration::days(365),
};
let token = JwtService::generate_admin_token(&params)?;
```

### Session Token Generation

```rust
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};

let generator = SessionGenerator::new(jwt_secret, "systemprompt");
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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-security = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
