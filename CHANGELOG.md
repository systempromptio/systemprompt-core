# Changelog

## [0.1.19] - 2026-03-31

### Added
- `CloudEnterpriseLicenseInfo` struct for domain-based enterprise licensing
- `enterprise` field on `UserMeResponse` (optional, backward-compatible)
- `EnterpriseLicenseInfo` type alias
- Structured streaming with `StreamChunk` enum for typed AI provider responses with token usage tracking
- Pricing-based cost calculation for streaming responses
- Authenticated `/api/v1/health/detail` endpoint with full system diagnostics (split from public health check)
- Email validation module (`validation.rs`) with shared `is_valid_email` helper
- ConnectInfo fallback for IP extraction in bot detector and IP ban middleware
- `geolocation` feature flag for optional GeoIP/MaxMind dependency in analytics and runtime

### Changed
- Simplify public `/health` endpoint to a lightweight DB-only check (fast for load balancers)
- Replace `tokio::process::Command("df")` disk usage with synchronous `libc::statvfs` syscall
- Make `CliService` conditionally compiled behind `cli` feature flag in logging crate
- Reduce default tokio features in workspace (remove `fs`, `process`, `signal` from default set)
- Replace blocking `std::sync::Mutex` with `tokio::sync::Mutex` in Gemini AI provider to prevent tokio worker thread stalls
- Agent sub-processes now start with a clean environment (`env_clear`) instead of inheriting all parent secrets
- Filter system traces and unknown status from trace list by default

### Security
- Fix OAuth redirect URI bypass: full URLs can no longer match relative URI registrations
- Fix WebAuthn user ID spoofing: completion handler now verifies authenticated user identity via auth token instead of trusting query parameter
- Remove wildcard CORS headers from WebAuthn completion endpoint
- Enforce 120-second expiry on WebAuthn registration and authentication challenges
- Add Shannon entropy validation for PKCE code challenges
- Block internal/private IP addresses in OAuth resource URI validation
- Use constant-time comparison (`subtle` crate) for sync token authentication
- Block symlinks and hardlinks in tarball extraction with canonical path validation
- Unify authorization code error messages to prevent enumeration attacks

### Refactored
- **CLI architecture remediation**: eliminate all `unwrap_or_default()`, `unsafe`, unlogged `.ok()`, and `println!()` violations across 8 CLI domains (admin, analytics, cloud, core, infrastructure, plugins, web, build)
- Split 14 oversized CLI files (>300 lines) into focused submodules — zero files now exceed the 300-line limit
- Extract magic numbers to named constants across analytics and infrastructure commands
- Refactor long functions (>75 lines) in analytics agents/show, sessions/live, and tools/show
- Replace `unsafe { std::env::set_var() }` in cloud profile/sync with safe `ProfileBootstrap::init_from_path()` config propagation
- Replace raw `std::env::var()` calls in cloud commands with Config-based alternatives
- **Struct consolidation**: rename duplicate `ToolModelConfig` (all-optional) to `ToolModelOverride`, resolve `Settings` collision into `ServicesSettings`/`DeploymentSettings`, deduplicate `RenderingHints` (CLI now imports from models crate)
- Convert `ToolContext` ID fields from raw `String` to typed identifiers (`SessionId`, `TraceId`, `AiToolCallId`)
- Convert image generation model ID fields from raw `String` to typed identifiers (`UserId`, `SessionId`, `TraceId`, `McpExecutionId`)
- **Eliminate inline SQL from CLI**: move 10 inline queries from `logs/show.rs`, `logs/export.rs`, and `logs/summary.rs` to `TraceQueryService` with dedicated query modules (`log_lookup_queries.rs`, `log_summary_queries.rs`)
- **Typed IDs for trace models**: replace 6 remaining `String` ID fields with typed identifiers (`LogId`, `AiRequestId`, `ExecutionStepId`) across `LogSearchItem`, `AiRequestListItem`, `AiRequestDetail`, `AuditLookupResult`, `ExecutionStep`, `AiRequestInfo`
- **DRY identifier definitions**: consolidate hand-written identifier structs into `define_id!()` macro invocations, removing ~2,500 lines of duplicated boilerplate across 14 identifier modules
- Consolidate shared utilities and per-crate `.sqlx/` caches for publish workflow
- Config cleanup: encapsulate visibility, remove dead code across config and logging crates
- **Code quality sweep across all layers** (139 files): remove clippy suppressions, fix forbidden constructs, eliminate silent error patterns
  - Remove `#[allow(clippy::*)]` suppressions by fixing underlying issues: `cognitive_complexity` (split functions), `too_many_arguments` (parameter structs), `struct_excessive_bools` (bitflags/enums), `print_stdout` (CliService::output/std::io::Write), `expect_used` (proper error propagation), `unnecessary_wraps`, `struct_field_names`, `empty_structs_with_brackets`, `option_option` (CategoryIdUpdate enum), `enum_variant_names`
  - Replace `CommandDescriptor` 6-bool struct with u8 bitflags pattern and const accessor methods
  - Introduce parameter structs: `TenantSessionParams`, `NonStreamingRequest`, `SessionStoreParams`, `ToolCallParams`, `TrackingParams`, `ReconcileSuccessParams`, `BuildContextParams`, `AuthCodeValidationParams`
  - Remove anyhow bridges in `AiError` and `AgentError`: replace `DatabaseError(#[from] anyhow::Error)` with `DatabaseError(String)`
  - Replace `println!`/`eprintln!` with `std::io::Write` across infra/logging CLI display and startup validation
  - Fix all `unwrap_or_default()` in CLI and domain code with explicit error handling
  - Fix silent error patterns: convert `let _ =` and `.ok()` to `tracing::warn!` or proper propagation across agent, mcp, scheduler, runtime, and API layers
  - Replace `Vec<EndpointRateLimit>` for rate limit config (eliminate struct_field_names)
  - Split `ProviderCapabilities` into `SchemaComposition` + `SchemaFeatures` sub-structs
  - Replace `process::exit()` with proper error propagation in CLI bootstrap
- Extract trace/logging queries into dedicated modules (`log_search_queries.rs`, `request_queries.rs`, `tool_queries.rs`, etc.)
- Remove dead `show_helpers.rs` and unused agent lib.rs clippy allow-list
- **Module visibility hardening**: convert `pub mod` to `pub(crate) mod` for internal modules across 7 domain crates (agent, ai, analytics, users, oauth, content, mcp) — reduces public API surface while preserving re-exports
- **Rename `models::ContentError` to `ContentValidationError`**: resolve naming collision with the operational `error::ContentError` in the content crate
- Fix `McpCspDomains` field references (`connect_domains` -> `connect`, `resource_domains` -> `resources`, etc.)
- Fix `BuildContextParams` call sites to use struct construction instead of positional args
- Fix `Arc<AnalyticsService>` to `Arc<dyn AnalyticsProvider>` coercion in session middleware
- Fix `CloudPaths` API consumers after `get_cloud_paths()` return type change
- Remove unused `_pool` parameter from `CleanupRepository::new_with_write_pool`

### Fixed
- Sub-process binary resolution now checks both `target/release` and `target/debug`, preferring the newest by mtime — matches justfile behavior so MCP servers and agents find the correct binary during development
- MCP binary validation uses dynamic bin directory resolution instead of hardcoding `target/release`
- Fix test compilation across `systemprompt-generator` and `systemprompt-sync`
- Remove needless `..Default::default()` in API JWT config
- Fix `bool as Option<bool>` invalid cast in trace list queries
- Populate AI trace summary fields (`total_cost`, `total_tokens`, `total_latency`) that were previously always zero

## [0.1.18] - 2026-03-05

### Changed
- Upgrade Rust edition from 2021 to 2024
- Reorder imports across all crates to comply with Rust 2024 edition formatting rules
- Change `unsafe_code` workspace lint from `forbid` to `deny`
- Parallelize prerender pipeline: concurrent source processing, item rendering, and content enrichment
- Replace regex-based TOC heading ID injection with string search (removes `regex` dependency from generator)

### Removed
- Remove TUI OAuth client seed data and configuration
- Remove TUI testing plan
