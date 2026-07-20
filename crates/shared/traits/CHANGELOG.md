# Changelog

## [0.22.0] - 2026-07-20

### Breaking

- **Breaking:** `AnalyticsProvider::extract_analytics` takes an `ExtractSignals<'_>` bundle (request URI plus resolved caller IP) instead of parsing hop headers. Migrate by constructing `ExtractSignals` with the caller IP resolved at the HTTP boundary.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- `ContentProvider::get_content`, `get_content_by_slug`, and `get_content_by_source_and_slug` are renamed to `find_content`, `find_content_by_slug`, and `find_content_by_source_and_slug`.
- `LogService::get_recent` and `get_by_id` are renamed to `list_recent` and `find_by_id`.

## [0.16.0] - 2026-06-22

### Breaking

- **Breaking:** The `artifact` module (the `ArtifactSupport` trait and the `schemas` helpers) is removed. No migration; it had no consumers.
- **Breaking:** `ContentProvider::get_content` takes `&ContentId` instead of `&str`. Migrate by constructing the id with `ContentId::new`.
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
- Trait surface aligned to the 0.11.0 workspace: sync and gateway provider abstractions follow the tenancy strip in `domain/ai` and the Service-JWT handshake in `domain/oauth`. Implementors no longer thread a runtime `tenant_id` through provider calls.

## [0.2.0] - 2026-04-15

### Breaking
- **Breaking:** `ContextProvider`, `UserProvider`, and `RoleProvider` trait methods now take typed identifiers (`&UserId`, `&ContextId`, `Option<&SessionId>`) instead of `&str`, and `ContextWithStats::context_id` / `ContextWithStats::user_id` are now `ContextId` / `UserId`. Migrate by replacing string arguments and field accesses with the corresponding typed identifier from `systemprompt-identifiers`.

### Removed
- **Breaking:** Removed `AuthProvider` and its `DynAuthProvider` alias. Migrate by depending on `UserProvider` and `RoleProvider` directly.
- **Breaking:** Removed `AuthorizationProvider` and its `DynAuthorizationProvider` alias. Migrate by implementing authorization in the calling domain.
- **Breaking:** Removed `AuthAction`, `AuthPermission`, `TokenPair`, and `TokenClaims`. Migrate by switching to `AgentJwtClaims` and the JWT provider trait.

## [0.1.18] - 2026-03-27

### Changed
- Bumped to the Rust 2024 edition.

### Removed
- Removed the `ExtensionError` concrete type from this crate; downstream errors now implement the `ExtensionError` trait directly.

## [0.1.2] - 2026-02-03

### Changed
- Synchronised the crate version with the workspace.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; aligned with the workspace 0.1.0 baseline.

## [0.0.13] - 2026-01-27

### Changed
- Synchronised the crate version with the workspace.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure to support distributed schema registration.

### Fixed
- Schema validation now accepts `VIEW`-based schemas.

## [0.0.2] - 2026-01-22

### Added
- Distributed schema registration via the `Extension` trait; each domain crate owns its SQL schemas.

### Changed
- Centralised module loaders previously hosted in `systemprompt-loader` are no longer exposed from this crate.

### Fixed
- `include_str!` paths now resolve inside the crate directory, allowing the crate to build standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
