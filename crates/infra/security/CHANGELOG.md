# Changelog

## [0.4.3] - 2026-04-29

### Added

- `manifest_signing::sign_value<T: Serialize>` and `canonicalize<T>` for RFC 8785 (JCS) canonical JSON.

### Changed

- `manifest_signing::signing_key` reads its ed25519 seed directly from `manifest_signing_secret_seed`. JWT HMAC compromise no longer compromises manifest signatures.

### Removed

- **Breaking**: `DOMAIN_SEPARATOR` constant and the `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation path.

### Internal

- `serde_jcs = "0.1"` added.

## [0.3.0] - 2026-04-22

### Fixed
- `signing_key`: removes redundant clone and replaces `expect` with proper concurrent-init handling via `OnceLock::set` match

## [0.1.18] - 2026-03-27

### Added
- `client_id` parameter on `AdminTokenParams` for configurable JWT client ID

### Changed
- Upgrade to Rust 2024 edition

### Removed
- Remove hardcoded `sp_tui` client ID from JWT generation

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

## [0.0.11] - 2026-01-26

### Fixed
- Fix clippy warnings in security scanner module

## [0.0.3] - 2026-01-22

### Fixed
- Fix schema validation for VIEW-based schemas
- Add migration system infrastructure

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration pattern
- Each domain crate now owns its SQL schemas via Extension trait
- Remove centralized module loaders from systemprompt-loader

### Fixed
- Fix `include_str!` paths that pointed outside crate directory
- Ensure crate compiles standalone when downloaded from crates.io

## [0.0.1] - 2026-01-21

- Initial release
