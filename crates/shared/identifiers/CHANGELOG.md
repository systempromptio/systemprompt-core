# Changelog

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** `ClientId::sync()` and the `sys_sync` client identifier are removed along with the cloud-sync feature that was their only consumer. Migrate by dropping references; there is no replacement.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.

### Changed

- Workspace version bump; no API changes in this crate.

## [0.17.0] - 2026-06-24

### Added

- Typed Slack (`SlackChannelId`, `SlackUserId`, `SlackWorkspaceId`) and Teams identifiers.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Removed

- `systemprompt_identifiers::bootstrap::{anonymous, bot, unknown, default, empty_sentinel}` are deleted, along with `UserId::{anonymous, system, bootstrap, is_anonymous, is_system}`. `UserId` values must originate from a row in the `users` table; the middleware persists one before constructing a request context.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Added
- `ClientId::sync()` and `ClientId::bridge()` constructors for the well-known OAuth client identifiers used by the Service-JWT sync handshake and the bridge session flow.
- Typed header newtypes used by the sync client (`Authorization`, request identifiers) so call sites no longer pass borrowed `&str`.

## [0.10.0] - 2026-05-14

### Changed
- Version bump for the 0.10.0 workspace release. No changes to the public API.

## [0.9.2] - 2026-05-14

### Breaking
- **Breaking:** `SessionSource::Cowork` renamed to `SessionSource::Bridge`; `as_str` now returns `"bridge"` and `SessionSource::from_client_id("sp_bridge")` resolves to `SessionSource::Bridge`. Migrate by replacing `SessionSource::Cowork` with `SessionSource::Bridge` at call sites.
- **Breaking:** `ClientId::cowork()` renamed to `ClientId::bridge()` and returns `"sp_bridge"`. Migrate by replacing `ClientId::cowork()` with `ClientId::bridge()`.

## [0.1.18] - 2026-03-27

### Added
- `PROXY_VERIFIED` and `USER_PERMISSIONS` header constants for the proxy-verified identity flow.

### Changed
- Upgraded to the Rust 2024 edition.

### Removed
- Removed unused session ID helper methods.

## [0.1.3] - 2026-02-19

### Added
- `HookId` typed identifier with `generate()` support for hook catalogue entries.

## [0.1.2] - 2026-02-03

### Added
- `SessionSource::Mcp` variant for identifying MCP protocol sessions.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; all workspace crates aligned at 0.1.0.

## [0.0.13] - 2026-01-26

### Added
- `PlaybookId` typed identifier for the playbook domain.

## [0.0.3] - 2026-01-22

### Fixed
- Schema validation now handles VIEW-based schemas.

### Added
- Migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait, replacing the centralised module loaders in `systemprompt-loader`.

### Fixed
- `include_str!` paths no longer point outside the crate directory, allowing the crate to compile standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
