# Changelog

## [0.2.0] - 2026-04-15

### Changed (BREAKING)
- `ContextProvider`, `UserProvider`, `RoleProvider` trait methods now take typed identifiers (`&UserId`, `&ContextId`, `Option<&SessionId>`) instead of `&str`. `ContextWithStats` fields `context_id` and `user_id` are now `ContextId` / `UserId`.

### Removed (BREAKING)
- Deleted `AuthProvider` trait (and `DynAuthProvider` alias) — single dead impl, zero callers.
- Deleted `AuthorizationProvider` trait (and `DynAuthorizationProvider` alias) — single dead impl returning stub values regardless of input, zero callers. Latent authorization footgun.
- Deleted associated dead types: `AuthAction`, `AuthPermission`, `TokenPair`, `TokenClaims`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition

### Removed
- Remove `ExtensionError` type
- Remove doc comments and inline comments from trait definitions

## [0.1.2] - 2026-02-03

### Changed
- Version bump for workspace consistency

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

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
