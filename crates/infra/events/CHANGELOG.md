# Changelog

## [0.44.0] - 2026-09-02

### Fixed

- The cross-replica relay no longer redelivers a node's own events to it. `event_outbox` rows carry `origin_instance_id` and the bridge skips its own, so subscribers on the emitting node were receiving every event twice on any deployment with more than one replica.

## [0.29.0] - 2026-08-04

### Added

- `systemprompt_events::is_listening` reports whether the cross-replica relay currently holds a listener. It reads `true` in a deployment that never starts a bridge, so the absence of a relay is not mistaken for a broken one, and flips to `false` only once a bridge has failed to establish `LISTEN` and has not recovered.
- A listener that fails with SQLSTATE `25006` (`read_only_sql_transaction`) is named for what it is: the pool points at a read-only standby, and `LISTEN`/`NOTIFY` requires the primary. The error says which knob fixes it (`database_write_url`) rather than surfacing the raw driver message, which reads as a transient connection fault.

### Changed

- Listener reconnection backs off exponentially from 1s to a 60s cap and resets on success, instead of retrying every 5s forever. A relay pointed at a standby never recovers on its own, and the fixed interval turned a misconfiguration into a permanent once-per-5s error stream against the database.
- Opening the listener is one fallible step rather than two: `PgListener::connect_with` and `LISTEN` shared a retry path but reported through separate branches that could drift.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.

### Changed

- The outbox-row identifier is typed through the repository lookup surface.

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
