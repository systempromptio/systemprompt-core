# Changelog

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

### Changed
- Refreshed offline `.sqlx/` query cache for the 0.11.0 workspace: every analytics query is re-verified against the post-tenancy-strip schema.

## [0.9.2] - 2026-05-14

### Changed
- Normalize changelog formatting and entry style.

## [0.1.21] - 2026-04-02

### Changed
- Expose `models` module publicly for external consumers.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade crate to the Rust 2024 edition.

### Fixed
- Rewrite content analytics queries to join `engagement_events` with `user_sessions` and filter bots via `is_bot` and `is_behavioral_bot` flags.
- Cast `avg_time_on_page` to `float8` for type safety.
- Cap `time_on_page_ms` at 1,800,000 ms to exclude outliers.

## [0.1.10] - 2026-02-08

### Added
- Add `event_type` column and accompanying migration to `engagement_events`.
- Add `content_id` column and index to `engagement_events`.
- Resolve content IDs from slugs during engagement tracking.
- Add `EngagementOptionalMetrics` with `serde(flatten)` for optional fields.
- Provide a default event-type helper for backwards-compatible deserialization.

### Changed
- Split `CreateEngagementEventInput` into required and optional field groups.
- Include `event_type` and `content_id` in engagement repository queries.

## [0.1.2] - 2026-02-03

### Changed
- Switch cost queries to `cost_microdollars` (`BIGINT`) for sub-cent precision.
- Regenerate the SQLx offline query cache.

## [0.1.0] - 2026-02-02

### Changed
- Align crate version with the workspace 0.1.0 stable release.

## [0.0.13] - 2026-01-27

### Changed
- Use `is_none_or` in place of `map_or` in bot detection.

## [0.0.11] - 2026-01-26

### Added
- Fan out engagement metrics on `PageExit` analytics events via `fan_out_engagement`.

### Fixed
- Resolve clippy warnings in repository modules.

## [0.0.3] - 2026-01-22

### Added
- Add migration system infrastructure.

### Fixed
- Validate schemas defined as SQL `VIEW`s.

## [0.0.2] - 2026-01-22

### Changed
- Adopt the distributed schema registration pattern with each domain crate owning its SQL via the `Extension` trait.
- Remove centralized module loaders from `systemprompt-loader`.

### Fixed
- Correct `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
