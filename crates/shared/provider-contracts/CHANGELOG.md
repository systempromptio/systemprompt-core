# Changelog

## [0.23.0] - 2026-07-24

### Added

- `JobContext::enforce()` and `JobContext::with_enforce()` carry the job configuration's enforcement consent. A job whose actions are destructive or outward-facing must take them only when `enforce()` is `true`; the default is `false`.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- `ToolProvider::call_tool` takes a typed `McpServerId`; `ToolProviderError` configuration variants carry their source instead of flattening to a string; target/routable lookups adopt the `list_` prefix.

## [0.16.0] - 2026-06-22

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

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
- Workspace-aligned release. Provider trait definitions track the tenancy strip in `domain/ai` and `domain/oauth`: provider call sites no longer thread a runtime `tenant_id`.

## [0.9.2] - 2026-05-14

### Changed
- Normalized changelog format to maintainer style with explicit categories.

## [0.1.0] - 2026-02-02

### Changed
- Aligned to workspace 0.1.0 release.

## [0.0.13] - 2026-01-27

### Changed
- Workspace version bump.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation now handles VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait.

### Removed
- Centralized module loaders from `systemprompt-loader`.

### Fixed
- Corrected `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
