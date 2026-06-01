# Changelog

## [0.14.0] - 2026-06-01

### Changed

- The profile loader validates the provider registry before any layer that references it, and resolves the gateway section against the registry rather than against an embedded catalog directory.

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

### Added
- Config types now derive `JsonSchema` (via `systemprompt-models`), so the loaded profile can be introspected and validated against a generated schema document.

## [0.9.2] - 2026-05-14

### Added
- Introduce `BootstrapSequence` type-state helper that enforces profile-before-secrets ordering at compile time.
- Add `SkillConfigValidator` implementing `DomainConfig` to walk `skills/` and report missing or malformed manifests.
- Expose `profile_gateway` module and `load_profile_with_catalog` for catalog-aware profile loading.

### Changed
- Relocate profile and secrets I/O from `systemprompt-models` into this crate so the bootstrap layer owns disk access.
- Replace ad-hoc error returns with the unified `ConfigError` / `ConfigResult<T>` API composing bootstrap, profile, secrets, schema-validation, and `serde`/`std::io` errors via `#[from]`.
- Tighten module visibility: `config_loader`, `profile_loader`, `services`, and `skill_validator` are now crate-private with re-exports through `lib.rs`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed
- Align all workspace crates at the 0.1.0 release milestone.

## [0.0.13] - 2026-01-27

### Changed
- Bump version for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Add migration system infrastructure.

### Fixed
- Fix schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration so each domain crate owns its SQL schemas via the `Extension` trait.
- Remove centralised module loaders from `systemprompt-loader`.

### Fixed
- Fix `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
