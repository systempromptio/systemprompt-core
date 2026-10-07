# Changelog

## [0.63.0] - 2026-10-07

### Added

- New crate. Token issuance for `/oauth/token` moves here from `systemprompt-api`'s `routes::oauth::endpoints::token::{generation, validation, handler}`: `TokenIssuanceOrchestrator` dispatches by `grant_type`, `IssuanceError` is the typed failure (formerly the API's `TokenError`), and `TokenRequest`, `TokenResponse` and `RequestOrigin` are the grant input, issued token and request origin.
- `user_tokens::generate_tokens_by_user_id` takes `UserTokenParams { origin: RequestOrigin, .. }` and, like `resolve_user_permissions`, fails with the typed `UserTokenError`.
