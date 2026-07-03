# Changelog

## [0.19.0] - 2026-07-02

### Breaking

- `File.metadata` is `sqlx::types::Json<FileMetadata>` (was `serde_json::Value`) and the fallible `File::metadata()` accessor is removed — use the typed field directly. `FileMetadata` gains a flattened `extra` map so historical metadata shapes decode losslessly.
- `ContentFile.role` is `FileRole` (was `String`) and `parsed_role()` is removed. `FileRole` serde casing is now `snake_case`, so the OG-image role serializes as `"og_image"` (previously `"ogimage"`, which never matched the database representation).
- `InsertFileRequest.metadata` is `FileMetadata`. Fields sourced from `systemprompt_traits::{InsertAiFileParams, AiGeneratedFile}` remain `serde_json::Value` at the trait boundary.

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
- Workspace-aligned release. File storage surface unchanged.

## [0.9.2] - 2026-05-14

### Changed
- Normalize `CHANGELOG.md` format to align with workspace-wide maintainer style.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade crate to the Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed
- Promote crate to a stable `0.1.0` release aligned with the workspace baseline.

## [0.0.13] - 2026-01-27

### Changed
- Bump version for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Add migration system infrastructure for schema rollout.

### Fixed
- Fix schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Added
- Distribute schema registration so each domain crate owns its SQL via the `Extension` trait.

### Changed
- Remove the centralised module loader path from `systemprompt-loader` in favour of per-crate ownership.

### Fixed
- Correct `include_str!` paths that previously pointed outside the crate directory.
- Ensure the crate compiles standalone when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release of `systemprompt-files`.
