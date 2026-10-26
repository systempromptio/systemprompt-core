# Changelog

## [0.73.0] - 2026-10-25

### Breaking

- **Breaking:** `token_exchange::subject::shared_jwks_client` is removed in favour of `JwksCache`, one JWKS client per trusted issuer owned by the `/oauth/token` route; `TokenIssuanceOrchestrator::new`, `handle_token_exchange`, `validate_subject_token`, `validate_id_jag_subject`, `validate_oidc_subject` and `issue_id_jag` take a `&JwksCache`. Migrate by building one `JwksCache::new()` where the OAuth routes are mounted.
- **Breaking:** `IdJagGrant` carries the `SigningKeys` it signs with (`IdJagGrant::keys`). Migrate by passing `ctx.signing_keys()`.
- **Breaking:** SSO request, owner, user and client message ids decode as typed identifiers. Migrate by reading the values through `as_str()`.
- `ClientCredentialsError::ConfigUnavailable`, `UserTokenError::Config` and `ConnectorGrantError::Secrets` are removed, because the config and secrets arrive as arguments; `UserTokenParams` gains `config: &Arc<Config>`. `credential::cache::token_for` is the method `TokenCache::token_for(&self, key, mint)`. Migrate by adding `config` to `UserTokenParams` literals, dropping the removed match arms, and calling `token_for` on the `TokenCache` the caller owns.


## [0.69.0] - 2026-10-15

### Fixed

- Token exchange keeps one JWKS client per trusted-issuer host allowlist for the process, so the issuer's `Cache-Control` TTL and kid-miss refetch throttle apply; an `id_token` for several audiences must name this client as `azp` (OIDC Core §3.1.3.7) and must carry `iat`.

## [0.67.0] - 2026-10-13

### Added

- `sso` module: `SsoDriver` (`StartOutcome`, `FinishOutcome`) owns state correlation, replay protection, identity policy, federated resolution, session minting and the login audit for every registered `SsoProvider`. `plan_resolution` (`ResolutionPlan`, `ResolvedUser`) orders the outcomes: federated link, verified-email link, provision, refuse. `RoleResolver` (`DirectoryOnlyRoleResolver` by default) lets a downstream merge out-of-band grants before roles are assigned; `SsoHookContext` carries what a hook sees. Registrations through `register_sso_provider!`, `register_auth_event_sink!` and `register_role_resolver!` (`SsoProviderRegistration`, `AuthEventSinkRegistration`, `RoleResolverRegistration`). Cookie binding (`BINDER_COOKIE_NAME`, `CookieSpec`, `binder_from_cookie_header`), `mint_browser_session`, `BrowserSessionSubject`, `permissions_for_roles`, `sanitize_return_to` and `SSO_STATE_TTL_SECS`.
- A crate-private repository bundle composes the SSO state, replay and login-event repositories for the driver.

### Changed

- `MintedSession::session_id` is a `SessionId`.

## [0.66.0] - 2026-10-13

### Changed

- The authorization-code and refresh grants refuse a service account (`OauthError::ServiceAccountInteractive`, `access_denied`).

## [0.63.0] - 2026-10-07

### Added

- New crate. Token issuance for `/oauth/token` moves here from `systemprompt-api`'s `routes::oauth::endpoints::token::{generation, validation, handler}`: `TokenIssuanceOrchestrator` dispatches by `grant_type`, `IssuanceError` is the typed failure (formerly the API's `TokenError`), and `TokenRequest`, `TokenResponse` and `RequestOrigin` are the grant input, issued token and request origin.
- `user_tokens::generate_tokens_by_user_id` takes `UserTokenParams { origin: RequestOrigin, .. }` and, like `resolve_user_permissions`, fails with the typed `UserTokenError`.
