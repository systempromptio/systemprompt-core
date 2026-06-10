# Changelog

## [0.16.0] - 2026-06-10

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

### Changed

- Over-long functions were split into focused helpers to satisfy the workspace's 75-line function ceiling. No behavioural or API change.

### Fixed

- Authorization codes are bound to the client that requested them and are rejected when redeemed by any other client.
- Replay detection logs the stored authorization-code hash instead of the raw code.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- `DynamicRegistrationRequest::get_grant_types` and `get_response_types` now return `Vec<String>` infallibly and apply the RFC 7591 §2 server defaults — `["authorization_code"]` and `["code"]` respectively — when the request omits the field or supplies an empty array. The previous `Result<Vec<String>, String>` shape rejected spec-compliant minimal registrations from MCP clients with `invalid_client_metadata`. Call sites in `entry/api` drop the `?` propagation. The repository layer's own omission defaults are unchanged; the handler now always passes the resolved arrays through, so persisted state and the response echo agree.

## [0.12.0] - 2026-05-27

### Changed

- `access_control_rules` repository SQLx query cache refreshed for the `rule_type ('role','user')` narrowing in `systemprompt-security` migration `008_drop_department_acl.sql`.

## [0.11.0] - 2026-05-20

### Breaking
- `bridge_sessions.tenant_id` removed. Migration `003_drop_bridge_session_tenant.sql` drops the column; bridge OAuth flows no longer scope to a per-row tenant identifier.

### Added
- `provision_sync_oauth_client` service mints the `sys_sync` OAuth client used by `app/sync` for the `client_credentials` Service-JWT handshake.
- Per-tenant OAuth client provisioning, supporting the deployment-plane tenancy model now that runtime tenancy is gone from the gateway and bridge paths.

### Changed
- Long rustdoc paragraphs in module headers split for readability; no API change.

## [0.9.2] - 2026-05-14

### Added

- `BridgeSessionRepository` (`upsert`, `list_active`, `list_active_for_user`, `delete_stale`) backed by the new `bridge_sessions` table; powers `/v1/bridge/heartbeat` and `systemprompt admin bridge list`.
- `BridgeHostPrefsRepository` for per-host bridge enable/disable state.
- `setup_token` repository with `SetupTokenPurpose`, `SetupTokenRecord`, and `TokenValidationResult` for bootstrap and admin flows.

### Changed

- **Breaking:** Renamed the `cowork` surface to `bridge`. Migrate by replacing `issue_cowork_access` with `issue_bridge_access`, `issue_cowork_access_with` with `issue_bridge_access_with`, `issue_cowork_exchange_code` with `issue_bridge_exchange_code`, `exchange_cowork_session_code` with `exchange_bridge_session_code`, `CoworkAuthResult` with `BridgeAuthResult`, `CoworkExchangeCode` with `BridgeExchangeCode`, and `create_cowork_exchange_code` / `consume_cowork_exchange_code` with their `bridge_*` equivalents.
- **Breaking:** Renamed the `cowork_exchange_codes` table to `bridge_exchange_codes` with matching index renames; the idempotent `MIGRATION_002_RENAME_COWORK_TO_BRIDGE` on `OauthExtension` performs the rename in place at next bootstrap.

## [0.4.3] - 2026-04-29

### Changed

- **Breaking:** `issue_cowork_access_with` now mints `audience: vec![JwtAudience::Cowork]` instead of `JwtAudience::Api`. Migrate by adding `JwtAudience::Cowork` to validators that previously accepted cowork tokens on generic API endpoints.

## [0.2.0] - 2026-04-15

### Removed

- **Breaking:** Removed `JwtAuthProvider`, `JwtAuthorizationProvider`, and `TraitBasedAuthService` along with `src/services/auth_provider.rs` and the corresponding re-exports from `lib.rs` and `services/mod.rs`. Migrate by using `JwtValidationProviderImpl` for token validation and `JwtClaims::get_permissions()` for permission checks.

## [0.1.18] - 2026-03-27

### Changed

- Upgraded the crate to the Rust 2024 edition.

### Removed

- Removed the TUI client seed SQL files.

## [0.1.7] - 2026-03-05

### Changed

- Regenerated the SQLx offline query cache.

## [0.1.6] - 2026-02-18

### Fixed

- Fixed formatting of the chained method call in `matches_relative_uri`.

## [0.1.5] - 2026-02-11

### Changed

- Switched the OAuth login page to a fullscreen mobile layout (100dvh, no border-radius) and a 480px desktop container.
- Added responsive breakpoints at 480px and 768px with touch-friendly button sizing.
- Made the "Create New Passkey" button and divider conditionally hidden via the `{register_class}` template variable.
- Wired registration visibility to `Config.allow_registration` (sourced from profile `security.allow_registration`).

## [0.1.4] - 2026-02-04

### Added

- Added RFC 8707 Resource Indicators support for MCP OAuth 2.1 compliance.
- Added a `resource` column to `oauth_auth_codes` with the accompanying migration.
- Added the `AuthCodeValidationResult` struct for richer auth-code validation responses.
- Added the `AuthCodeParamsBuilder::with_resource()` builder method.
- Added the `JwtConfig.resource` field for resource-scoped token generation.

### Changed

- **Breaking:** `validate_authorization_code()` now returns `AuthCodeValidationResult` instead of a tuple. Migrate by destructuring the struct fields in place of the previous tuple positions.
- Extended `JwtAudience` with a `Resource(String)` variant for RFC 8707 audience binding.
- Token generation now adds the resource URI to the JWT audience claim when present.

## [0.1.2] - 2026-02-03

### Changed

- Regenerated the SQLx offline query cache.

## [0.1.1] - 2026-02-03

### Added

- Added the `SessionCreationError` typed error enum for session-creation failures.

### Fixed

- Validated user existence before creating an authenticated session to prevent FK constraint violations.

## [0.1.0] - 2026-02-02

### Changed

- First stable release; aligned the crate version with the rest of the workspace at 0.1.0.

## [0.0.13] - 2026-01-27

### Changed

- Version bump for workspace consistency.

## [0.0.11] - 2026-01-26

### Added

- Added the `fingerprint_hash` field to `AnonymousSessionInfo` for session tracking.

## [0.0.3] - 2026-01-22

### Added

- Added the migration system infrastructure.

### Fixed

- Fixed schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed

- Implemented the distributed schema registration pattern; each domain crate now owns its SQL schemas via the `Extension` trait.

### Removed

- Removed centralised module loaders from `systemprompt-loader`.

### Fixed

- Fixed `include_str!` paths that pointed outside the crate directory so the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added

- Initial release.
