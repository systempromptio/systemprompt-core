# Changelog

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

### Changed

- Over-long functions were split into focused helpers to satisfy the workspace's 75-line function ceiling. No behavioural or API change.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Cloud deployment tenancy is unchanged; the tenancy strip in `domain/ai` and `domain/oauth` covers the runtime data plane only.

## [0.9.2] - 2026-05-14

### Changed
- Normalize changelog formatting and section headings.

## [0.1.24] - 2026-04-14

### Added
- Add `CredentialsBootstrap::init_empty()` to mark credentials as intentionally absent for local-only profiles so `get()` returns `Ok(None)` instead of `Err(NotInitialized)`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.

### Fixed
- Fix CLI session store edge cases.

## [0.1.4] - 2026-02-11

### Changed
- Add 10s connect timeout and 30s request timeout to the cloud API HTTP client.

## [0.1.3] - 2026-02-03

### Added
- Add `CloudApiClient::report_activity()` for sending activity events to the management API.
- Add `post_no_response()` helper for POST requests without a response body.

## [0.1.1] - 2026-02-03

### Changed
- `get_cloud_paths()` now resolves via `ProjectContext` and the typed `ProjectPath` enum.
- Credentials always resolve to `.systemprompt/credentials.json` via `ProjectPath::LocalCredentials`.
- Add `CloudPaths::from_project_context()` constructor for typed path resolution.

### Removed
- Remove profile-relative path resolution from `get_cloud_paths()` in favour of `ProjectContext::discover()`.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; align all crates at 0.1.0.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.11] - 2026-01-26

### Changed
- Improve `SessionStore` management and key handling.
- Refactor CLI session store for reliability.

## [0.0.4] - 2026-01-23

### Added
- Add `cancel_subscription` method to `CloudApiClient`.
- Add `update_from_tenant_info` to preserve credentials during tenant sync.

### Fixed
- Fix schema validation for VIEW-based schemas.
- Add migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration; each domain crate owns its SQL schemas via the `Extension` trait.

### Removed
- Remove centralized module loaders from `systemprompt-loader`.

### Fixed
- Fix `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
