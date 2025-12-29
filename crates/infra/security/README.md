# systemprompt-core-security

Security module providing authentication, JWT handling, and token extraction.

## Directories

| Directory | Purpose |
|-----------|---------|
| `auth/` | Authentication validation service |
| `extraction/` | Token extraction from headers and cookies |
| `jwt/` | JWT token generation |
| `services/` | Security services (scanner detection) |

## Key Files

| File | Purpose |
|------|---------|
| `lib.rs` | Module exports |
| `auth/validation.rs` | AuthValidationService, AuthMode, TokenClaims |
| `extraction/token.rs` | TokenExtractor with fallback chain |
| `extraction/cookie.rs` | CookieExtractor for cookie-based auth |
| `extraction/header.rs` | HeaderInjector for propagating auth headers |
| `jwt/mod.rs` | JwtService for admin token generation |
| `services/scanner.rs` | ScannerDetector for bot/scanner detection |

## Dependencies

- `systemprompt-models` - Shared models (JwtClaims, UserType, Permission)
- `systemprompt-identifiers` - Typed identifiers (UserId, SessionId, etc.)
