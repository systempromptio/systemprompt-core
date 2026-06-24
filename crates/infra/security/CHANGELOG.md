# Changelog

## [0.17.0] - 2026-06-24

### Added

- Messaging identity ingestion: Slack and Teams users are resolved to authorization entities so chat actors are governed like any other caller.

## [0.16.0] - 2026-06-22

### Breaking

- JWT validation requires a first-party audience claim (`web`, `api`, `a2a`, or `mcp`); tokens minted without an audience are rejected.
- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Removed

- `AuthMode` enum and the `AuthMode::Optional` A2A optional-auth anonymous context path. `AuthValidationService::validate_request` now takes only the headers — there is no longer a "optional auth that quietly returns an anonymous principal" mode. Callers requiring anonymous access wire the public/no-auth route flavour instead.

## [0.12.0] - 2026-05-27

### Breaking

- `RuleType::Department`, `DenyReason::DepartmentDeny`, and `MatchedBy::DepartmentAllow` removed from the authz resolver. `ResolveInput` drops its `department` field. Migration `008_drop_department_acl.sql` narrows `access_control_rules.rule_type` to `('role','user')` and deletes any existing department rows.
- `AccessControlRepository::list_role_department_rules_for_export` renamed to `list_role_rules_for_export`.
- `AppContextBuilder::with_authz_hook` is now generic over `H: AuthzDecisionHook + 'static`; callers pass owned hook values. Callers holding an `Arc<dyn AuthzDecisionHook>` use the new `with_shared_authz_hook(SharedAuthzHook)` method.
- `SharedAuthzHook` moved to `systemprompt_security::authz::hook`; the `authz` facade re-export is unchanged.
- `AuthzMode::Extension` selection at bootstrap requires a hook supplied via `with_authz_hook(...)` or registered through `register_authz_hook!`; bootstrap errors if neither is present.

### Added

- `RuleBasedHook` — the core RBAC resolver promoted to a first-class `AuthzDecisionHook`. Wraps the sync `authz::resolver::resolve` so extensions compose it via `CompositeAuthzHook`. Bootstrap composes `[RuleBasedHook, ...extensions]` automatically when a DB pool is available; `mode: webhook` composes `[RuleBasedHook, WebhookHook]`.
- `AuthzSource::RuleBased` audit-source variant (`policy = "authz_rule_based"`) so resolver decisions stay observable in `governance_decisions` alongside webhook and extension rows.
- `authz::registry` inventory site for static-init authz hook registration (`register_authz_hook!`), used when binaries delegate to `systemprompt::cli::run()` and have no builder call-site.

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
