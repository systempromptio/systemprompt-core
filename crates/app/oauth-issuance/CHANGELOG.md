# Changelog

## [Unreleased]

### Added

- New crate. Token issuance for `/oauth/token` moves here from `systemprompt-api`'s `routes::oauth::endpoints::token::{generation, validation, handler}`: `TokenIssuanceOrchestrator` dispatches by `grant_type`, `IssuanceError` is the typed failure (formerly the API's `TokenError`), and `TokenRequest`, `TokenResponse` and `RequestOrigin` are the grant input, issued token and request origin.
