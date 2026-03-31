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

### Fixed
- Sub-process binary resolution now checks both `target/release` and `target/debug`, preferring the newest by mtime — matches justfile behavior so MCP servers and agents find the correct binary during development
- MCP binary validation uses dynamic bin directory resolution instead of hardcoding `target/release`
- Fix test compilation across `systemprompt-generator` and `systemprompt-sync`
- Remove needless `..Default::default()` in API JWT config
- Fix `bool as Option<bool>` invalid cast in trace list queries

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
