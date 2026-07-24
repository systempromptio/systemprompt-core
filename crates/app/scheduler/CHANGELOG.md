# Changelog

## [0.23.0] - 2026-07-24

### Added

- `behavioral_analysis` and `malicious_ip_blacklist` classify and log ban candidates in observe mode and ban IPs only when the job's `enforce` flag is set, so automated banning is an explicit per-deployment opt-in.
- A warning is logged at boot for every inventory-registered job with no `scheduler.jobs` entry — a job available in the build but silently never scheduled is now visible.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.20.0] - 2026-07-15

### Changed

- `cleanup_empty_contexts` spares session-bound `cli_session` contexts and collects session-orphaned ones, keyed on the new `user_contexts.kind` column instead of the display-name prefix.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.

### Changed

- The database-cleanup job's per-table deletion is extracted into a focused helper; no public API or behavioural change.

## [0.17.0] - 2026-06-24

### Breaking

- `SchedulerService::start` now returns `SchedulerStartup { handle, degraded }` instead of `Option<SchedulerHandle>`. Migrate by reading `.handle` from the returned value.

### Changed

- A scheduled job whose explicit owner does not resolve to an active user is now skipped and recorded as an `ERROR` in the `logs` table, instead of aborting the entire scheduler. A job with no explicit owner runs as the profile `system_admin`.

### Added

- `SchedulerStartup.degraded` and the exported `SkippedJob` type report jobs dropped at startup because their explicit owner did not resolve.

### Removed

- `SchedulerError::UnresolvedJobOwner`. An unresolved owner now degrades that job rather than returning an error.

## [0.16.1] - 2026-06-22

### Changed

- The database-cleanup job also prunes expired ID-JAG replay rows.

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
