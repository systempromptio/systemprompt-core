# Changelog

## [Unreleased]

### Breaking

- **Breaking:** `SessionStore::update_activity` is removed. `increment_request_count` already stamps `last_activity_at` and `duration_seconds`, and the request middleware, its only production caller, now makes just that one call.

## [0.55.0] - 2026-09-17

### Breaking

- **Breaking:** `McpServerInfo.port` is `Option<u16>` (`None` for an `external` server).

## [0.54.0] - 2026-09-16

### Removed

- **Breaking:** `ManagedRevisionOwnership` / `DynManagedRevisionOwnership` are removed.

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `LogService` and `ContentProvider` are native `async fn` traits (no `#[async_trait]`, no `dyn`); `LogService::{find_by_id, delete}` take `&LogId`; `ContentProvider` category filters are `Option<CategoryId>`.
- **Breaking:** `RepositoryError::Database { message }` replaces `Database(Box<dyn Error>)`; `RepositoryError::database(err)` takes any `Display`.
- **Breaking:** `AgentRegistryProvider::agent_exists` / `McpRegistryProvider::server_exists` return `Result<bool, RegistryError>`; only `NotFound` maps to `false`.
- **Breaking:** `ValidationError` (validation_report) is `ValidationIssue`; `validation::ValidationError` is `MetadataValidationError`; `extension_error::ApiError` is `ExtensionApiError`.
- **Breaking:** `AnalyticsProvider::find_reusable_session` returns `Option<SessionId>`; `AgentJwtClaims.subject` is a `UserId`; `AiGeneratedFile.id` is a `FileId` (the separate `id()` accessor is gone); `InsertAiFileParams`, `CreateSessionInput` and `ContextWithStats` are built through `new` / `with_*` (fields are no longer constructed by literal).
- **Breaking:** `OptionalStartupEventExt` is removed; `StartupEventExt` is implemented for `Option<&StartupEventSender>` and exposes `sender()` / `emit()` defaults, so the same method names work on both.

### Added

- `tool_executions::{ToolExecutionLookup, DynToolExecutionLookup}` — the cross-domain read seam over the MCP tool-execution ledger, implemented by the mcp domain and injected into agent.
- `SessionProvider`, `SessionStore` (session persistence, usage counters and behavioural data, implemented by the users domain), `AnalyticsEventStore` (implemented by logging), `ContentCatalogStats` (implemented by content) and `OwnerReassignment` / `ReassignedRows` (implemented by agent, ai and mcp for cross-domain user merges), each with its `Dyn*` alias. `SessionStore::find_reusable_fingerprint_session` answers a typed `SessionId`.
- `AiRequestTrace` (`sample`, `find_usage`, `list_usage`) with `TraceSampleFilter`, `TraceSampleMode`, `TraceSample`, `TraceMessage`, `TraceRequestUsage` (`is_settled` covers a completed row or one whose accounting failed after the spend was recorded), `TraceRequestStatus` and `DynAiRequestTrace`: the read seam over the AI request trace for domains that do not own it.
- **Breaking:** `AiSessionProvider::find_live_session` reports a session's owner only while it is neither revoked nor expired. Migrate by implementing it on every `AiSessionProvider`.
- `ManagedRevisionOwnership` / `DynManagedRevisionOwnership`: owner-scoped lookup of the resource a managed revision belongs to.

### Removed

- The unimplemented seams `Module`, `ApiModule`, `ModuleRegistry`, `register_module!`, `Service`, `AsyncService`, `traits::scheduler` (`JobTrigger`, `SchedulerLifecycle`, `JobInfo`, `JobStatus`, `SchedulerError`), the `traits::Result` alias, `LogEventPublisher` / `UserEventPublisher` / `AnalyticsEventPublisher`, `LogEventLevel` / `LogEventData`, and the `web` feature (with its `axum` and `inventory` dependencies).

## [0.52.0] - 2026-09-14

### Added

- `managed_resources`: the `ManagedSkillResolver` trait (`DynManagedSkillResolver`) with `SkillResolution::{NotManaged, Published(ResolvedManagedSkill), Withheld(WithheldReason)}`, `WithheldReason::{NeverAdopted, Withdrawn}` and `ManagedSkillResolverError::{Integrity, Unavailable}` — the seam through which agents resolve managed skills without a marketplace dependency.

## [0.44.0] - 2026-09-02

### Changed

- Trait signatures that took a long tail of positional parameters take a parameter struct instead, which is what `clippy::too_many_arguments` was flagging across the implementations.

## [0.38.0] - 2026-08-25

### Changed

- **Breaking:** `MessagingInbound.claims: FederatedIdentityClaims` is now `sender: SenderIdentity`. Call `sender.claims()` for the previous value; match `Linked`/`Unlinked` to tell a platform-verified sender from a first-touch one.

## [0.29.0] - 2026-08-05

### Added

- `SessionUsageCounters` (and `DynSessionUsageCounters`): session-scoped task/message counter increments, implemented by the analytics session repository and injected into domain workflows at composition roots. Failures are logged by callers, never propagated.

## [0.28.0] - 2026-07-31

### Breaking

- **Breaking:** the `OAuthRequirement` returned by `McpRegistryProvider` and `AgentRegistryProvider` gains `ema: bool`. An implementation building it by literal needs the extra initialiser; `false` preserves today's behaviour, and agents do not participate in Enterprise-Managed Authorization.

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `StartupEvent::McpServerReady.tools` and the `tools` argument of `StartupEventExt::mcp_ready` are `Option<usize>`. `None` means the server's tool list was never enumerated, which an OAuth-gated MCP server's reachability-only validation cannot do; it was previously indistinguishable from a measured zero. Migrate by passing `Some(count)` where a count was measured and `None` where it was not.

## [0.25.0] - 2026-07-27

### Breaking

- **Breaking:** `StartupEvent::SchedulerReady` carries `scheduled` and `available` in place of `job_count`, and `StartupEventExt::scheduler_ready` takes both. The single count was the inventory total — the jobs compiled into the binary, not the jobs a deployment will run. Migrate by passing the configured count first and the discovered count second.

## [0.22.0] - 2026-07-20

### Breaking

- **Breaking:** `AnalyticsProvider::extract_analytics` takes an `ExtractSignals<'_>` bundle (request URI plus resolved caller IP) instead of parsing hop headers. Migrate by constructing `ExtractSignals` with the caller IP resolved at the HTTP boundary.
- **Breaking:** `SessionAnalytics` is the single definition of a request's session signals — the parallel struct in `systemprompt-analytics` is gone. Its `referer`, `accept_language`, `page_url`, `screen_width`, `screen_height`, and `timezone` fields are removed (each was a write-only alias or always `None`), and it gains `is_bot`, `is_ai_crawler`, and `skip_tracking`, decided once by the provider that owns the keyword tables. Migrate by reading `preferred_locale` / `referrer_url` / `entry_url` and treating the verdicts as fields rather than calling `is_bot()` / `is_ai_crawler()`.

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
