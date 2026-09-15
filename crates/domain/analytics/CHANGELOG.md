# Changelog

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `AnalyticsRepositories::new(db, sessions: DynSessionStore, event_sink: DynAnalyticsEventStore, content: DynContentCatalogStats)` — the users, logging and content owners are injected; `SessionRepository` delegates session persistence to the users store and behavioural event/content reads to their owners. Migrate by passing the owners from the composition root (`AppContext::analytics_repositories()`).
- **Breaking:** session lifecycle moves to `systemprompt_users` (`SessionRepository`, `UsersAiSessionProvider`, the session mutations, geo and fingerprint queries) and `SessionCleanupService` is removed; `AnalyticsService` extracts request signals only. Event ingestion runs through the logging-owned `AnalyticsEventStore`; the authoritative public-page count comes from `ContentCatalogStats`.
- **Breaking:** reporting repositories (agents, tools, requests, costs, conversations, traffic, content, core stats, overview, CLI sessions) read the analytics-owned `analytics_report_*` projections instead of the source tables; reports are eventually consistent and a report against an uninitialised baseline is refused with a rebuild instruction.
- **Breaking:** `FeedbackSnapshotsRepository::new(pool, facts: FeedbackFactsRepository)` takes the facts repository (`AppContext::feedback_snapshots_repository()`).
- **Breaking:** `SnapshotRangeRequest::operation_id`, `SnapshotRangeJob::operation_id` and `SnapshotJobLease::operation_id` are `AnalyticsSnapshotJobId`; every worker argument and `worker_id` field is `AnalyticsWorkerId`; `SnapshotRangeJob::state` is `SnapshotJobState`. Migrate by constructing the typed ids with `generate()`/`new()` and matching on the enum.
- **Breaking:** `RequestAnalyticsRepository::list_requests(start, end, &RequestListFilter)` replaces the `(limit, model)` arguments; `ConversationAnalyticsRepository::{list_agent_contexts, list_gateway_sessions}` take a trailing `user: Option<&str>`; `ConversationListRow` and `GatewaySessionListRow` gain `user_id`.

### Added

- `projection` module: versioned reporting contracts (`ReportingSource`, `SourceDefinition`, `ReportingRow`) and the `ReportingProjector` that applies `reporting.row` facts from the durable outbox in the delivering transaction, ignoring revisions already in the baseline and older or duplicate entity revisions; `begin_rebuild` / `finish_rebuild` replace projection rows under the projector advisory lock and shared source-table locks; `status(pool, consumer)` reports initialisation, generation, pending count and oldest pending age.
- `feedback` module: `FeedbackFactsRepository` and `FactsProcessingService` — durable normalised invocation, request, assessment and resource-association facts with source-qualified deduplication keys and monotonic revisions; corrections replace facts and tombstones retain ordering; leased processing with owner checkpoints (`drain`, owned cancellable `run`); downstream delta claiming (`claim_deltas`, `delta_batch`, `lock_delta_lease` / `complete_delta_batch`); idempotent backfill pages. Migrations 005 (`analytics_fact_*`, `analytics_normalized_facts`) and 006 (`analytics_ingestion_producers`).
- `snapshots` module: fenced daily aggregate snapshots, bounded custom-range jobs (`request_range`, `claim_range`, `complete_range`, `range_job`), snapshot reads and health, and retention that erases evidence only behind committed producer, fact and snapshot barriers. Migration 007 (`analytics_snapshot_*`).
- `resource_metrics`: each request and assessed conversation counts once within a cohort; related conversation spend is non-additive across cohorts.
- Privacy coordination: `lock_user_deletion`, `next_cutoff_revision` and the `evidence_cutoff` on `analytics_projection_state`; the SQL functions installed by migrations 008–010 (`prepare_reporting_privacy`, `begin_user_privacy` counterparts) make a user deletion or merge wait for pending committed evidence and deliver it atomically before identity is removed.
- `models::reporting` row types for the CLI report commands.
- `FeedbackSnapshotsRepository::fail_range` records a lease-fenced terminal failure with its diagnostic.
- `CostAnalyticsRepository::get_breakdown_by_user` (spend, requests, tokens and distinct conversations per user); `RequestListFilter` (`user`, `offset`); `ConversationAnalyticsRepository::list_gateway_sessions` beside `list_agent_contexts`, both filterable by user.

### Changed

- Every table the extension creates is declared by its own schema file (`analytics_report_*`, `analytics_fact_*`, `analytics_snapshot_*`, `analytics_projection_*`, `ingestion_producers`), so `infra db doctor` reports none as undeclared; `reporting_privacy.sql` installs the privacy functions declaratively.
- Stream cursors are canonical digit-only strings; a padded or signed cursor is refused.
- The rebuild snapshot cursor and the retention lock live in the `projection` module; snapshot delta batches and retention compaction are split into named helpers.
- `FingerprintRepository::find_reusable_session` answers a typed `SessionId`; the fingerprint engagement count is a compile-time checked query.

### Fixed

- `complete_range` routes an assembly or serialisation error through `fail_range`, so a deterministic failure is no longer re-claimed on every run.
- The reporting projector's retention check uses the compile-time query macro.

## [0.48.0] - 2026-09-08

### Changed

- No functional change; the crate's `why` comments were re-cut by the comment-standards pass to state the hidden constraint and nothing else.

## [0.47.0] - 2026-09-06

### Breaking

- **Breaking:** `RequestStatsRow` gains `reasoning_tokens`, `cache_read_tokens` and `cache_creation_tokens`; `CostSummaryRow` gains the same three. Struct literals need the new fields; the platform, per-user and request aggregations sum them from the existing `ai_requests` columns. The three figures were already stored per request and were dropped on the way into every rollup, so reasoning and cache spend could be read for one request and for no window.
## [0.44.0] - 2026-09-02

### Fixed

- Analytics lookups that feed an authorization decision read the primary, not a read replica; a moment after a write the replica still answered with the previous row.

## [0.42.0] - 2026-08-31

### Added

- `ProfileUsageService` assembles the bridge profile-usage report in the domain that owns the queries. The API route had been building it inline and running its own SQL from a handler body.
- Recent-context rows carry `context_name`.

## [0.31.0] - 2026-08-18

### Changed

- Platform and per-user cost aggregations exclude `ai_requests` rows flagged `synthetic`, so seeded demo traffic no longer inflates spend dashboards.

## [0.29.0] - 2026-08-04

### Breaking

- **Breaking:** the `models::cli` module is renamed to `models::reporting`. Migrate by updating module-qualified imports; the glob re-exports at `models::*` are unchanged.

### Changed

- Missing GeoIP-reader status logs once per process without a per-request IP. The diagnostic states that location fields remain empty.
- The two `geoip_skip_reason = "private_or_local_ip"` lines drop from `debug` to `trace`. They fire on every request on a loopback or LAN deployment, and a proxy misconfiguration resolving clients to private addresses is still diagnosable at `trace`.

## [0.27.0] - 2026-07-29

### Added

- `SessionRepository::backfill_session_geo` batch-updates `country`/`region`/`city` on sessions with an IP but no geo data (private IPs skipped, keyset-paginated, idempotent), and `count_sessions_missing_geo` reports the candidate count. Backs the new `backfill_session_geo` scheduler job — `country` was previously write-once at INSERT, so enabling GeoIP appeared to do nothing for existing rows.
- GeoIP lookups log a structured `geoip_skip_reason` (`no_reader` vs `private_or_local_ip`), so a missing database is distinguishable from a proxy misconfiguration resolving clients to private addresses.

## [0.25.0] - 2026-07-27

### Breaking

- **Breaking:** `RequestListRow::provider` and `RequestListRow::model` are `Option<String>`. A request rejected before routing has neither. Migrate by matching on the option at any site that reads either field.

### Changed

- The per-provider and per-model aggregates in `CostAnalyticsRepository`, `AnalyticsQueryRepository`, and `RequestAnalyticsRepository` exclude rows whose `provider` or `model` is `NULL`, so a rejected request no longer forms a group of its own.

## [0.24.0] - 2026-07-26

### Fixed

- The extension declares `funnel_steps`, whose DDL moves to its own `schema/funnel_steps.sql`. The installed schema is unchanged.

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** `BotRow.request_count` is renamed to `session_count` — it always counted sessions.

### Changed

- Every traffic, overview, and session query selects from the canonical `v_clean_traffic` / `v_engaged_traffic` / `v_bot_sessions` views instead of restating flag combinations, so the human/bot predicate is defined once and all four bot flags (`is_bot`, `is_ai_crawler`, `is_scanner`, `is_behavioral_bot`) are excluded uniformly.
- The bots breakdown reports a three-way partition — human, ghost (no landing page or zero requests), and bot sessions — whose per-type counts sum to the bot total.
- The bot user-agent keyword tables cover `mkgp-data-pipeline` (Common Crawl) and `quic-go`.

## [0.22.0] - 2026-07-20

### Breaking

- **Breaking:** `SessionAnalytics` is constructed via the `SessionAnalytics::builder(headers).with_uri(..).with_geoip(..).with_caller_ip(..)` builder; the `from_headers`, `from_headers_with_geoip`, `from_headers_with_geoip_and_socket`, `from_headers_and_uri`, and `from_request` constructors are removed. Migrate to the builder, supplying the caller IP explicitly.
- **Breaking:** `maxminddb` moves from 0.29 to 0.30, changing the `maxminddb::Reader` type behind the public `GeoIpReader` alias. Migrate by moving dependent crates to `maxminddb` 0.30.
- **Breaking:** `SessionAnalytics` is re-exported from `systemprompt-traits` rather than defined here, and its builder is constructed as `SessionAnalyticsBuilder::new(headers)`. `SessionAnalytics::builder`, the `is_bot()` / `is_ai_crawler()` / `is_bot_ip()` / `is_datacenter_ip()` / `is_high_risk_country()` / `is_spam_referrer()` / `should_skip_tracking()` predicates, `AnalyticsService::{is_bot, compute_fingerprint}`, and `CreateAnalyticsSessionInput` are removed. Migrate to `SessionAnalyticsBuilder::new`, the `is_bot` / `is_ai_crawler` / `skip_tracking` fields, `SessionAnalytics::compute_fingerprint`, and `systemprompt_traits::CreateSessionInput`.

### Fixed

- The extractor no longer parses the client IP from `X-Forwarded-For` / `X-Real-IP`, so a spoofed hop header can no longer set `ip_address` or the derived GeoIP fields.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.20.0] - 2026-07-15

### Changed

- Conversation analytics (context counts, listings, activity trends, platform totals) exclude `kind = 'cli_session'` bookkeeping rows, so dashboards count only real conversations.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.

### Changed

- The provider-usage cost aggregate widens to `bigint` to avoid overflow on large totals, and the provider-usage query moves to compile-time-verified sqlx macros.

### Removed

- The feature-ambiguous geoip lookup wrapper is dropped.

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
