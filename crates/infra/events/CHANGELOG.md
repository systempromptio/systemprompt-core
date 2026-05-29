# Changelog

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Added
- Postgres event outbox. A new outbox repository persists domain events under a transactional contract; a `LISTEN`/`NOTIFY` bridge on the `systemprompt_events` channel relays them to subscribers on every replica.
- `OUTBOX_CHANNEL` constant naming the relay channel for in-process subscribers.

## [0.9.2] - 2026-05-14

### Added
- `EventError` and `EventResult` as the crate's public, `thiserror`-derived error surface.
- `AnalyticsBroadcaster`, `ANALYTICS_BROADCASTER`, and `EventRouter::route_analytics` for analytics-event fan-out.
- `ConnectionGuard` RAII wrapper that unregisters SSE connections on drop.
- `standard_keep_alive`, `HEARTBEAT_INTERVAL`, and `HEARTBEAT_JSON` for SSE keep-alive configuration.
- `ToSse` impl for `CliOutputEvent` to support CLI streaming.

### Changed
- Routed `EventRouter::route_agui` and `route_a2a` to mirror events onto `CONTEXT_BROADCASTER` for the unified context stream.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to Rust 2024 edition.

## [0.1.3] - 2026-02-03

### Removed
- **Breaking:** `WebhookUserEventPublisher` — migrate by switching to the cloud activity API in `systemprompt-cloud`.
- Unused dependencies `hmac`, `sha2`, `hex`, `chrono`, `reqwest`, and `systemprompt-traits`.

## [0.1.0] - 2026-02-02

### Changed
- Aligned to the workspace 0.1.0 stable release.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation for view-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- **Breaking:** Centralized module loaders removed from `systemprompt-loader` in favor of distributed schema registration — migrate by registering schemas through the `Extension` trait on the owning domain crate.
- Each domain crate now owns its SQL schemas via the `Extension` trait.

### Fixed
- `include_str!` paths that pointed outside the crate directory.
- Standalone compilation when the crate is fetched from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
