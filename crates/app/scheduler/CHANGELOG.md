# Changelog

## [0.15.3] - 2026-06-10

### Breaking

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

### Added
- Cross-replica job claim through Postgres advisory locks. Concurrent replicas race for the lock keyed by job + tick, and at most one executes a given tick.

### Fixed
- Distributed-lock key is now tick-deterministic, removing the time-drift flake that allowed two replicas to occasionally execute the same scheduled tick.

## [0.10.0] - 2026-05-15

### Removed
- **Breaking:** `SchedulerExtension::migration_weight()`. Extension ordering is now derived solely from the dependency graph; the scheduler declares its `users` dependency via `Extension::dependencies()`.

## [0.9.2] - 2026-05-14

### Added
- Expose `GhostSessionCleanupJob` and `NoJsCleanupJob` for ghost-session and no-JavaScript fingerprint cleanup.
- Expose service orchestration primitives `ServiceReconciler`, `ServiceStateManager`, `ProcessCleanup`, and `VerifiedServiceState` from the crate root.
- Expose `ServiceManagementService` for service lifecycle queries and graceful shutdown.

### Changed
- Route job registration through `systemprompt_provider_contracts::submit_job!` for inventory-based discovery.
- Public APIs now return `SchedulerResult<T>` backed by a `thiserror`-derived `SchedulerError`; job bodies retain `ProviderResult` via a `From` impl.
- Split `services::orchestration::process_cleanup` into POSIX and Windows submodules behind `#[cfg]` shims.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.

### Fixed
- Correct scheduler extension job registration logic.

## [0.1.2] - 2026-02-03

### Changed
- Regenerate SQLx offline query cache.

## [0.1.1] - 2026-02-03

### Changed
- Lower cleanup-job log verbosity from `info` to `debug`.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; align with workspace 0.1.0.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Adopt distributed schema registration: each domain crate owns its SQL schemas via the `Extension` trait.

### Removed
- **Breaking:** Centralized module loaders in `systemprompt-loader`. Migrate by registering schemas through `Extension::schemas()`.

### Fixed
- Correct `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
