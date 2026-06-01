# Changelog

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Breaking

- `AuthenticatedUser`: `department`, `with_department`, and `department()` removed and replaced by `attributes: BTreeMap<String, serde_json::Value>` with `with_attributes` and `attributes()`.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Users repository surface unchanged.

## [0.10.0] - 2026-05-15

### Changed
- Migration SQL moved into `schema/migrations/NNN_*.sql` files, discovered by the
  crate `build.rs` and surfaced through `Extension::migrations()`.

## [0.9.2] - 2026-05-14

### Added
- API key issuance, hashing, and verification via `ApiKeyService`.
- Device certificate enrollment and rotation via `DeviceCertService`.
- `CleanupAnonymousUsersJob` re-export for scheduler registration.

### Changed
- Re-exported `UserProvider` and `RoleProvider` traits from `systemprompt-traits`.
- Migrated public errors to `thiserror`-derived `UserError` with `Result<T>` alias.

## [0.3.0] - 2026-04-22

### Changed
- Formatting cleanup in the `device_cert` repository.

## [0.1.21] - 2026-04-02

### Added
- `UserRepository::session_exists()` to check whether a session is active.
- `UserService::session_exists()` service method.
- Re-exported `CleanupAnonymousUsersJob` from the `jobs` module.

### Changed
- Exposed the `jobs` module for external registration.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to the Rust 2024 edition.

## [0.1.2] - 2026-02-03

### Changed
- Regenerated the SQLx offline query cache.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; aligned to the workspace 0.1.0 version line.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Changed
- Marked the users extension as required via `is_required() -> true`.

### Fixed
- Fixed schema validation for `VIEW`-based schemas.
- Added migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Adopted the distributed schema registration pattern; each domain crate now owns its SQL schemas via the `Extension` trait.
- Removed centralised module loaders from `systemprompt-loader`.

### Fixed
- Fixed `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
