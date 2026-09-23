# Changelog

## [Unreleased]

### Breaking

- **Breaking:** the `reporting` module is removed (`spawn`, `process_pending`, `initialize`, `rebuild`, `status`, `RebuildOutcome`, `ReportingStatus`, `SnapshotWakeup`), and with it `AppContext::{feedback_facts_repository, feedback_snapshots_repository, snapshot_wakeup}` and the matching `DataPlane`/`Subsystems` fields. There is no reporting worker and no baseline rebuild at boot.
- **Breaking:** `managed::capture_authoring_input`, `managed::inventory::prepare_baselines` and `GitSourceOrchestrator::verify` are removed; they had no caller once the managed HTTP admin routes went. `GitSourceOrchestrator::synchronize` and `OrchestrationError` stay; `OrchestrationError::Source` now reads "Source operation failed".

## [0.59.1] - 2026-09-23

### Fixed

- The reporting drain names the cause of a poisoned fact. It logged the error
  and returned only a count and an outbox id, so in a context without a tracing
  subscriber the failure read as `1 reporting fact(s) left pending` and nothing
  more. The first poisoned fact's cause is now part of the returned error.

## [0.59.0] - 2026-09-22

### Breaking

- **Breaking:** `AppContext::build()` no longer builds the analytics reporting baseline; `reporting::spawn` (the server's reporting task) builds it in the background and then drains, so a large upgraded database boots and serves while the baseline is paged in. `reporting::initialize` returns `RebuildOutcome` (`Rebuilt`, `AlreadyInitialized`, `InProgressElsewhere`) and, like `rebuild`, runs the phased rebuild synchronously for tests and the CLI.

### Added

- `AppContext::session_store()` beside `session_usage()`: the gateway needs `SessionStore::increment_ai_usage`, which the narrowed `SessionUsageCounters` view does not expose. The underlying owner already returns `DynSessionStore`, so no second repository is constructed.

### Changed

- Reporting retention is a profile default rather than an operator's memory: the `retention:` block carries every window and `database_cleanup` enforces all of them in batches under a per-run time budget.

## [0.55.0] - 2026-09-17

### Breaking

- **Breaking:** `AppContext` owns the process's `AiService` and `ArtifactIngest`: `ai_service()` / `ai_service_arc()` return `Option<Arc<AiService>>` (assembled at boot from the services catalog, `None` with a warning when no default provider is usable) and `artifact_ingest()` / `artifact_ingest_arc()` return the one tool-result ingest path (`context::services`). Every extension router receives `Extension<Option<Arc<AiService>>>`, `Extension<Arc<ArtifactIngest>>` and `Extension<ServicesRefresh>` beside the governance engine.
- **Breaking:** `ActiveServicesRoot.base` is propagated through the loader bootstrap; the composed root binds the profile services path it is layered on.

### Added

- `trace`: `AiRequestListItem` / `AiRequestDetail` and the tool-call read model carry `finish_reason`.
- `startup_validation::mcp_validator` checks each server by type: an `external` server must declare none of `binary` / `package` / `port`, an internal one must declare `binary` and `port`; port uniqueness is checked over internal servers only.

## [0.54.0] - 2026-09-16

### Removed

- **Breaking:** `optimization` is `managed`: `managed::{OrchestrationError, capture_authoring_input, git_sources::GitSourceOrchestrator, inventory}`. `SkillOptimizationOrchestrator`, `EvaluationEvidence`, `SourceAcceptance`, `holdout`, `OptimizationError::{Evaluation, Bundle, Json}` and `AppContext::evaluation_repositories` / `DataPlane::evaluation_repositories` are removed.

### Added

- **Breaking:** `services_reconcile::reconcile_fetched_services` returns `ReconcileOutcome::{Projected, NothingPending}` instead of `()`, so a caller can report whether an authz projection actually ran.
- `AiRequestListItem::client_attestation`; `AiRequestDetail::{client_kind, client_attestation, client_evidence}` with the new `AiRequestClientEvidence` read model.

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `Subsystems.event_bridge` holds an `EventBridgeHandle` instead of a `JoinHandle`; shutdown cancels and joins it.
- **Breaking:** `HoldoutConfirmationTarget::id` is `EvalHoldoutProposalId`. Migrate by constructing it with `EvalHoldoutProposalId::try_new`.
- **Breaking:** `TraceQueryService::list_audit_messages` / `list_audit_tool_calls` take an `AuditPage` (`AuditPage::ALL` for the previous behaviour).
- **Breaking:** `SkillOptimizationOrchestrator::new(managed, evaluations)` drops the separate `RevisionRepository` argument and reads revisions from `EvaluationRepositories::revisions`. Migrate by removing the third argument.

### Added

- `Subsystems.publish_guard` / `AppContext::publish_guard()`: the inventory publish memo shared by the scheduled refresh and the manual route, previously a process static. `Subsystems.snapshot_wakeup` / `AppContext::snapshot_wakeup()` (`reporting::SnapshotWakeup`): the `feedback_snapshots` listener relay, spawned on first subscription and joined by `shutdown()`.
- `RuntimeError::Evaluation`; the composition root builds `EvaluationSeams` from the AI request repository, the users session provider and the managed repository and constructs the evaluation repositories on the write pool.
- `Plugins::marketplace_cache` / `AppContext::marketplace_cache()`: the owned marketplace catalogue and bundle cache, built once per context.
- `AppContext::schema_install()` exposes the `SchemaInstallReport` of the boot's schema installation (`Subsystems.schema_install`), so `/health/detail` can surface declared foreign keys an established database could not create.
- `trace` module: `TraceQueryService`, `AiTraceService`, their result types and `TraceError` (moved from `systemprompt_logging::trace`); the crate now carries its own `.sqlx` cache.
- `AppContext::governance()` / `governance_arc()`; `Subsystems.governance` carries the engine built once from the profile's services root.
- `OptimizationError::Bundle(RevisionBundleError)` for a candidate bundle that fails verification.
- `reporting` module: `spawn` (the owned projection worker: 256 deliveries per pass, once per second, a full pass reschedules immediately), `initialize` (installs owner capture contracts and builds the baseline when none exists), `rebuild`, `process_pending` and `status`; the builder initialises reporting at boot.
- `optimization` module: cross-domain source verification and evaluation attestation (`candidate`, `capture`, `diagnostics`, `holdout`, `inventory`, `iteration`) and `GitSourceOrchestrator` (`git_sources`), the application-owned credential resolution for Git import, sync and verification.
- `AppContext::feedback_facts_repository()` and `feedback_snapshots_repository()`; the repository accessors live in `context::repositories`; `AppContext::analytics_repositories()` is built with the users session store, the logging event store and the content catalog stats.
- `trace::RequestCursor` and `AiRequestFilter::{with_until, with_before}` for keyset paging of request logs; `TraceQueryService::{count_audit_messages, count_audit_tool_calls}` and `AuditPage` on the two audit list queries.
- `optimization::EvaluationEvidence`: the facts an evaluation attestation commits to, with `digest()` hashing them as canonical JSON (RFC 8785) so field order never changes the attested digest.
- `trace::RequestCursor` (`FromStr`; a malformed value is a `RequestCursorError`) and `AiRequestFilter::{with_until, with_before}` for keyset paging of request logs; `TraceQueryService::{count_audit_messages, count_audit_tool_calls}` and `AuditPage` on the two audit list queries.

### Changed

- `AppContext` and `DatabaseContext` connect through `Database::connect`; the profile `database_type` string is no longer consulted.
- The optimization iteration compares the campaign's typed `CampaignStatus`.
- The composition root builds `A2ARepositories` with `A2aDependencies` (managed-skill resolver and the mcp `ToolUsageRepository` as the tool-execution lookup) and adopts the legacy context through `ensure_legacy_context`.

### Fixed

- A configured-inventory load failure, an invalid dependency verification request and an unavailable secrets store carry their cause in the `OptimizationError::Source` message and are logged.
- The reporting projection worker claims through `OutboxConsumer` instead of a `DurableOutbox` stamped with a fabricated instance id.
- `AppContext` construction fails when the governance audit sink cannot obtain the write pool instead of silently installing a null audit sink.

- Boot no longer fails with `Context … not found for user` after the system admin changes: the legacy context (`ContextId::legacy()`) is re-homed onto the current admin through `ensure_system_context` instead of the user-scoped `ensure_context`.
- Core initialisation resolves the secrets store once and fails when it is unavailable, instead of resolving every services-bundle source credential to `None`.
- `reporting::rebuild` / `initialize` retry a transaction Postgres aborted as a deadlock or serialization failure (SQLSTATE 40P01 / 40001) up to four times with backoff; the rebuild holds `SHARE` locks on every source table and a concurrent writer could otherwise abort it.


## [0.52.0] - 2026-09-14

### Added

- `AppContext::managed_repository()` and `AppContext::evaluation_repositories()` expose the managed-resource and experiment repository bundles built once in the data plane; the A2A repository bundle is composed with the managed skill resolver so agents resolve managed skills through the shared `ManagedSkillResolver` seam.

## [0.51.0] - 2026-09-11

### Added

- `discover_models` (`builder::core_layer::discover_vertex_models`) augments the provider registry at boot from each upstream's live listing, given the secrets and a per-listing budget; which providers are discoverable is decided by the loader's catalog sources from the credential their secret parses into. The pass is fail-open: a provider no source recognises or a missing or unusable credential leaves the YAML catalog exactly as authored. It is exposed so the CLI runner can install the registry through it for `infra services serve|start`.

## [0.50.0] - 2026-09-10

### Added

- `ShutdownRequest` on `AppContext`, with `AppContext::request_restart`, lets a request handler ask the process to shut down cleanly so a supervisor brings it back. It is how an admin services refresh rolls onto a new composition.
- `services_reconcile` projects a freshly fetched composition into the authz tables once at boot, one pass per configured source in profile order, each carrying that source's name and an `IngestScope` built from what its manifest claims. An instance that swapped in a new composition and failed to reconcile is refused a boot rather than serving the old grants against the new catalog.

### Changed

- The core builder resolves the services root through `ServicesSourceBootstrap` before deriving `AppPaths`, so every path follows the composed tree when bundle sources are configured.

## [0.48.0] - 2026-09-08

### Changed

- `AppContextBuilder` initialises the global `GovernanceEngine` while it builds the context, so a deployment whose governance rules do not load fails at boot rather than on the first decision. `RuntimeError` gained a transparent `Governance` variant carrying `GovernanceEngineError`.

## [0.45.0] - 2026-09-03

### Removed

- `create_request_span`. Its only caller built the span and dropped it without entering it, so the fields it recorded never reached a log line. Migrate by opening the span at the HTTP boundary with `systemprompt_api`'s context middleware.

## [0.44.0] - 2026-09-02

### Added

- `AppContext` resolves and carries the replica's `InstanceId`, and hands it to the MCP service registry, the event outbox and the scheduler so each of them can scope its rows to the node that owns them.
- A `FileStorage` accessor on the context, built once from the `storage:` profile section and injected wherever uploads and generated files are written.

### Changed

- The provider catalog and gateway routes are read from the services tree instead of the profile, so the context no longer reaches into `profile.providers` to build the registry.

## [0.41.0] - 2026-08-28

### Changed

- The authz bootstrap is handed the parent chain built from the services config, so a rule granted on a plugin cascades to the skills, agents and MCP servers that plugin owns. When the services config cannot be loaded the builder warns and resolves without the cascade rather than failing to start, which keeps a config fault from taking the process down but does mean a plugin-level grant stops reaching its children until it is fixed.

## [0.29.0] - 2026-08-05

### Added

- `AppContext::session_usage()` exposes the analytics session repository as a `DynSessionUsageCounters` for wiring domain repositories that bump per-session counters without a direct analytics dependency.

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `AppContext::load_geoip_database` returns `Result<Option<GeoIpReader>, RuntimeError>`. An explicitly configured `paths.geoip_database` that cannot be opened fails startup with `RuntimeError::GeoIpUnreadable` instead of degrading to a warning that left every session's country NULL; "not configured" keeps the warning-and-`None` behaviour.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.

### Changed

- Content-analytics assembly is extracted from `AppContextBuilder::build` into a focused helper; no public API or behavioural change.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

### Changed

- Database pool settings from the profile's `database.pool` block are validated and applied when the application context is built.

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
- Replica identity on `AppContext`, exposed to the API server (Prometheus `served_by` middleware) and the scheduler (advisory-lock keys).
- Stream-concurrency config: a global semaphore caps in-flight A2A SSE streams, bounding file-descriptor use under fan-out.

## [0.10.2] - 2026-05-15

### Changed

- Adapt to `ExtensionRegistry::discover()` returning `Result`: `AppContextBuilder::build` and startup extension validation now propagate or report `LoaderError` instead of consuming an infallible value.

## [0.9.2] - 2026-05-14

### Added
- `RuntimeError` and `RuntimeResult` for typed error handling across the runtime surface.
- `context_loaders` module exposing `load_geoip_database` and `load_content_config` helpers.
- `context_traits` module for context-facing trait surfaces.
- `with_marketplace_filter` on `AppContextBuilder` for marketplace ACL injection.

### Changed
- Move `AppContextBuilder` into its own `builder.rs` module.
- Split context resource loading out of `context.rs` into `context_loaders.rs`.

### Removed
- `installation` module and its `install_module` / `install_module_with_db` entry points; install flows now live in `systemprompt-database`.

## [0.1.21] - 2026-04-01

### Changed
- Move `AppContext` construction logic from `new_internal` into `AppContextBuilder::build`.
- Add `AppContextParts` to keep `AppContext::from_parts` within the argument-count limit.
- Initialize logging immediately after database pool creation so subsequent tracing events are persisted.
- Remove the redundant `init_logging` call from `serve.rs`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.

## [0.1.10] - 2026-02-08

### Added
- `AppContext::content_routing` accessor returning `Option<Arc<dyn ContentRouting>>`.
- `RouteClassifier` integration with content routing for URL classification.

## [0.1.0] - 2026-02-02

### Changed
- First stable release at the unified workspace version.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.4] - 2026-01-23

### Added
- Export `FilesConfigValidator` from the startup validation module.

### Fixed
- Schema validation now handles VIEW-based schemas correctly.
- Wire in the migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait.

### Removed
- Centralized module loaders from `systemprompt-loader`.

### Fixed
- Correct `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
