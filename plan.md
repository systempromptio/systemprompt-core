● User approved Claude's plan
  ⎿  Plan saved to: ~/.claude/plans/polymorphic-beaming-bonbon.md · /plan to edit
     DRY Violation Remediation Plan

     Context

     The codebase has accumulated significant technical debt through copy-paste patterns across crates. The existing define_id! macro system in
     identifiers/src/macros.rs proves the project already embraces macros to eliminate boilerplate -- but similar patterns elsewhere remain
     hand-duplicated. This plan identifies 7 buckets of DRY violations, proposes idiomatic Rust solutions for each, and sequences them for parallel
     agent execution.

     ---
     Bucket 1: Token Type Boilerplate (3 identical 68-line files)

     Problem: CloudAuthToken, JwtToken, SessionToken are character-for-character identical except the type name. Each is 68 lines.

     Files:
     - crates/shared/identifiers/src/auth/cloud_token.rs
     - crates/shared/identifiers/src/auth/jwt_token.rs
     - crates/shared/identifiers/src/auth/session_token.rs

     Solution: Add define_token! macro to crates/shared/identifiers/src/macros.rs that generates:
     - Struct with derives (Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize + sqlx/serde cfg attrs)
     - new(), as_str(), redacted() methods
     - Display impl (delegates to redacted())
     - From<String>, From<&str>, AsRef<str>, ToDbValue impls

     Each token file reduces from 68 lines to ~1 line: define_token!(CloudAuthToken);

     Saves: ~200 lines, eliminates 3-way duplication permanently

     ---
     Bucket 2: String Truncation Helpers (4+ implementations)

     Problem: 4 separate truncation functions across 4 crates:
     1. truncate_with_ellipsis() -- UTF-8 safe, with tests (crates/entry/cli/src/shared/text.rs)
     2. truncate_id() -- naive byte-level (crates/entry/cli/src/commands/infrastructure/logs/shared.rs:6)
     3. Private truncate() -- naive byte-level (crates/shared/models/src/ai/tool_result_formatter.rs:140)
     4. Private truncate() -- identical copy (crates/domain/ai/src/services/tooled/formatter.rs)

     Plus 25+ inline truncation patterns (&str[..N], .chars().take(N)) scattered across CLI commands.

     Solution: Add pub mod text to crates/shared/models/src/ with:
     pub fn truncate(text: &str, max_len: usize) -> String;       // UTF-8 safe, with "..."
     pub fn truncate_id(id: &str, max_len: usize) -> String;      // For display-truncated IDs

     Adopt the UTF-8-safe implementation from entry/cli/src/shared/text.rs (includes find_char_boundary and tests). All 4 existing implementations and
     inline patterns import from here.

     Files to modify:
     - crates/shared/models/src/text.rs (new)
     - crates/shared/models/src/lib.rs (add pub mod text)
     - crates/entry/cli/src/shared/text.rs (re-export from models)
     - crates/entry/cli/src/commands/infrastructure/logs/shared.rs (remove truncate_id, import)
     - crates/shared/models/src/ai/tool_result_formatter.rs (remove private truncate, import)
     - crates/domain/ai/src/services/tooled/formatter.rs (remove private truncate, import)

     ---
     Bucket 3: Date/Time Formatting (15+ duplicated patterns)

     Problem:
     - format_timestamp() duplicated identically in 2 files (analytics/shared/time.rs:130 and logs/shared.rs:14)
     - format_duration_ms() in 2 files with different signatures (i64 vs Option<i64>)
     - 7+ analytics commands repeat format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d"))
     - Inconsistent format strings: "%Y-%m-%d %H:%M" vs "%Y-%m-%d %H:%M:%S" vs "%Y-%m-%d %H:%M:%S UTC"

     Solution: Add pub mod time_format to crates/shared/models/src/ with:
     pub fn format_timestamp(dt: DateTime<Utc>) -> String;           // "%Y-%m-%d %H:%M:%S"
     pub fn format_date(dt: DateTime<Utc>) -> String;                // "%Y-%m-%d"
     pub fn format_date_range(start: DateTime<Utc>, end: DateTime<Utc>) -> String;
     pub fn format_duration_ms(ms: i64) -> String;                   // human-readable
     pub fn format_optional_duration_ms(ms: Option<i64>) -> String;  // wraps above

     Leave parse_* functions in analytics/shared/time.rs (CLI-specific, use anyhow, not duplicated).

     Files to modify:
     - crates/shared/models/src/time_format.rs (new)
     - crates/shared/models/src/lib.rs (add export)
     - crates/entry/cli/src/commands/analytics/shared/time.rs (remove format fns, import)
     - crates/entry/cli/src/commands/infrastructure/logs/shared.rs (remove format fns, import)
     - 7 analytics trend command files (replace inline date range formatting)

     ---
     Bucket 4: Builder Method Boilerplate (86+ with_* methods)

     Problem: 43+ types have hand-rolled builders, each with 2-8 with_* methods that all follow:
     pub fn with_field(mut self, value: T) -> Self {
         self.field = Some(value.into());
         self
     }

     Also 40+ filter types in logging/trace/models.rs follow the same pattern.

     Solution: Add builder_methods! macro to crates/shared/models/src/lib.rs:
     macro_rules! builder_methods {
         ($( $method:ident -> $field:ident : $ty:ty ),* $(,)?) => { ... }
     }

     Apply incrementally to highest-boilerplate types first. Each type saves 4-6 lines per method.

     Files to modify:
     - crates/shared/models/src/lib.rs (add macro)
     - crates/infra/logging/src/trace/models.rs (apply to 4 filter types)
     - Other builder types incrementally

     ---
     Bucket 5: CLI Command Execution Pattern (11+ files)

     Problem: 11+ command files in infrastructure/logs/ repeat identical 3-tier pattern:
     pub async fn execute(args, config) -> Result<CommandResult<T>> {
         let ctx = AppContext::new().await?;
         let pool = ctx.db_pool().pool_arc()?;
         execute_with_pool_inner(args, &pool, config).await
     }
     pub async fn execute_with_pool(args, db_ctx, config) -> Result<CommandResult<T>> {
         let pool = db_ctx.db_pool().pool_arc()?;
         execute_with_pool_inner(args, &pool, config).await
     }

     Solution: Add define_pool_command! macro to crates/entry/cli/src/shared/mod.rs that generates both wrapper functions given the args type, result
     type, and inner function path.

     Files to modify:
     - crates/entry/cli/src/shared/mod.rs (add macro)
     - 11+ command files in crates/entry/cli/src/commands/infrastructure/logs/

     ---
     Bucket 6: API Response Helper Duplication

     Problem: 4 separate modules define response helpers, but canonical types already exist in shared/models/src/api/responses.rs (ApiError,
     SingleResponse, CollectionResponse, etc.)

     Duplicates:
     - crates/entry/api/src/routes/mcp/registry.rs -- local error_response, collection_response
     - crates/entry/api/src/routes/content/links/types.rs -- inline internal_error

     Note: OAuth responses in oauth/responses.rs must keep OAuth 2.0 spec format -- leave those.

     Solution: Replace MCP and content ad-hoc helpers with canonical ApiError/SingleResponse/CollectionResponse from systemprompt_models.

     Files to modify:
     - crates/entry/api/src/routes/mcp/registry.rs
     - crates/entry/api/src/routes/content/links/types.rs
     - crates/entry/api/src/routes/content/links/handlers.rs

     ---
     Bucket 7: OAuth Repository Init Boilerplate (10+ handlers)

     Problem: 10+ OAuth handlers repeat identical repository initialization:
     let repository = match OAuthRepository::new(state.db_pool()) {
         Ok(r) => r,
         Err(e) => return init_error(e),
     };

     Solution: Create Axum FromRequestParts extractor in crates/entry/api/src/routes/oauth/extractors.rs.

     Files to modify:
     - crates/entry/api/src/routes/oauth/extractors.rs (new)
     - crates/entry/api/src/routes/oauth/mod.rs
     - 10+ handler files in routes/oauth/

     ---
     Execution Sequencing

     Phase 1 -- Shared Foundation (parallel agents)

     ┌─────────┬─────────────────────────────────────────────────┬──────┬───────────┐
     │  Agent  │                     Bucket                      │ Risk │ LOC Saved │
     ├─────────┼─────────────────────────────────────────────────┼──────┼───────────┤
     │ Agent 1 │ Bucket 1: define_token! macro                   │ Low  │ ~200      │
     ├─────────┼─────────────────────────────────────────────────┼──────┼───────────┤
     │ Agent 2 │ Bucket 2: Truncation utilities in shared/models │ Low  │ ~100      │
     ├─────────┼─────────────────────────────────────────────────┼──────┼───────────┤
     │ Agent 3 │ Bucket 3: Time formatting in shared/models      │ Low  │ ~80       │
     └─────────┴─────────────────────────────────────────────────┴──────┴───────────┘

     Phase 2 -- Macro Infrastructure (parallel agents)

     ┌─────────┬───────────────────────────────────────────────────────────────┬────────┬───────────┐
     │  Agent  │                            Bucket                             │  Risk  │ LOC Saved │
     ├─────────┼───────────────────────────────────────────────────────────────┼────────┼───────────┤
     │ Agent 4 │ Bucket 4: builder_methods! macro + apply to filter types      │ Low    │ ~150      │
     ├─────────┼───────────────────────────────────────────────────────────────┼────────┼───────────┤
     │ Agent 5 │ Bucket 5: define_pool_command! macro + apply to logs commands │ Medium │ ~130      │
     └─────────┴───────────────────────────────────────────────────────────────┴────────┴───────────┘

     Phase 3 -- API Layer Cleanup (parallel agents)

     ┌─────────┬───────────────────────────────────────────────────────────┬────────┬───────────┐
     │  Agent  │                          Bucket                           │  Risk  │ LOC Saved │
     ├─────────┼───────────────────────────────────────────────────────────┼────────┼───────────┤
     │ Agent 6 │ Bucket 6: API response consolidation (MCP + content only) │ Medium │ ~40       │
     ├─────────┼───────────────────────────────────────────────────────────┼────────┼───────────┤
     │ Agent 7 │ Bucket 7: OAuth extractor                                 │ Medium │ ~80       │
     └─────────┴───────────────────────────────────────────────────────────┴────────┴───────────┘

     Phases are independently shippable. Within each phase, agents work on isolated file sets with no conflicts.

     Verification

     After each phase:
     1. cargo check --workspace -- compilation
     2. cargo clippy --workspace -- lint
     3. cargo test --manifest-path crates/tests/Cargo.toml --workspace -- integration tests
     4. Verify no regressions in CLI output formatting or API response shapes

● Starting Phase 1 with 3 parallel agents working on Buckets 1, 2, and 3.