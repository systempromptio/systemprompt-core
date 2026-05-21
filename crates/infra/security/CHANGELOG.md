# Changelog

## [0.11.0] - 2026-05-20

### Breaking

- `SessionGenerator::new` now takes only `issuer`; the `jwt_secret` argument is gone. Tokens are signed via the process-wide `TokenAuthority` (RS256) and there is no shared secret to plumb through.
- `AuthValidationService::new` likewise drops the leading `secret` parameter and now takes `(issuer, audiences)`.
- `AdminTokenParams` no longer carries `jwt_secret`. Token signing reads the active RSA key from the `TokenAuthority` cache.

### Added

- `at_rest` module exposing `hmac_sha256` and `hmac_sha256_hex` for storing identifiers (refresh-token ids, authorisation codes) as peppered HMAC-SHA-256 digests rather than plaintext.
- Authorisation policy plumbing supporting the new compile-time `RouterExt::with_auth(_, AuthzPolicy::*)` middleware in `entry/api`. Every authenticated route declares its policy at registration.

### Changed

- `repository.rs` query sites use compile-time-verified `query!` / `query_scalar!` macros throughout, in line with the repository-pattern rule.

### Fixed

- Authz `bootstrap.rs` tests are no longer flaky: a process-wide `tokio::sync::Mutex` serialises the shared global hook slot, so concurrent tests no longer observe half-installed hooks.

## [0.9.2] - 2026-05-14

### Added

- `authz` module: deny-overrides resolver, `access_control_rules` repository, and `AuthzDecisionHook` extension surface shared by gateway and MCP enforcement.
- `authz::audit` submodule: `AuthzAuditSink`, `DbAuditSink`, `NullAuditSink`, and `GovernanceDecisionRepository` for governance decision persistence.
- `authz::ingestion::AccessControlIngestionService` for loading rule sets from configuration.
- `AllowAllHook`, `DenyAllHook`, and `WebhookHook` implementations of `AuthzDecisionHook`.
- `auth::HookTokenValidator` and `ValidatedHookClaims` for bridge hook-token minting and verification.
- `JwtAudience::Cowork` audience variant wired through `AuthValidationService`.

### Changed

- Crate description reframed around the four-layer governance pipeline and unified authz decision plane.

## [0.4.3] - 2026-04-29

### Breaking

- **Breaking:** Removed `DOMAIN_SEPARATOR` and the `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation path. Migrate by configuring `manifest_signing_secret_seed` directly.

### Added

- `manifest_signing::sign_value<T: Serialize>` and `canonicalize<T>` for RFC 8785 JCS canonical JSON.
- `serde_jcs` dependency.

### Changed

- `manifest_signing::signing_key` reads its ed25519 seed from `manifest_signing_secret_seed`, isolating manifest signatures from JWT HMAC compromise.

## [0.3.0] - 2026-04-22

### Fixed

- `signing_key` removes a redundant clone and handles concurrent initialisation via `OnceLock::set` instead of `expect`.

## [0.1.18] - 2026-03-27

### Breaking

- **Breaking:** Removed hardcoded `sp_tui` client ID from JWT generation. Migrate by passing `client_id` on `AdminTokenParams`.

### Added

- `client_id` field on `AdminTokenParams` for configurable JWT client ID.

### Changed

- Upgraded to Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed

- First stable release at workspace-aligned version.

## [0.0.13] - 2026-01-27

### Changed

- Version bump for workspace alignment.

## [0.0.11] - 2026-01-26

### Fixed

- Resolved clippy warnings in the security scanner module.

## [0.0.3] - 2026-01-22

### Added

- Migration system infrastructure.

### Fixed

- Schema validation now accepts VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed

- Each domain crate now owns its SQL schemas via the `Extension` trait; centralised module loaders removed from `systemprompt-loader`.

### Fixed

- Corrected `include_str!` paths that pointed outside the crate directory so the crate compiles standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added

- Initial release.
