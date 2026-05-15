# systemprompt-analytics Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave C2)
**Verdict:** CLEAN

---

## Summary (post Wave C2)

| Category | Before | After |
|----------|-------:|------:|
| `unwrap()` / `expect()` | 0 | 0 |
| `panic!()` / `todo!()` / `unimplemented!()` | 0 | 0 |
| `println!` / `eprintln!` / `dbg!` | 0 | 0 |
| `let _ =` discards | 0 | 0 |
| `.ok()` discards | 6 | 6 (carve-out, see below) |
| Inline `//` comments | 0 | 0 |
| Doc `///` comments on pub items | 0 | added throughout |
| `//!` module headers on pub mods | 0 | 1 per pub mod |
| Files >300 lines | 3 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` outside allowlist | 34 (false-positive) | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` attributes | 3 | 0 |
| `anyhow::` references in src | 37 | 1 (`Other(#[from] anyhow::Error)` adapter only) |
| `async_trait` references | 6 | 6 (kept; native async traits introduced where dyn-safe) |

---

## sqlx audit decision table

The Wave 1 audit reported 34 raw `sqlx::query(...)` calls. All were
**false-positives** from a regex that matched `sqlx::query!` (the macro form)
as well. The `just lint-sqlx` and `grep -E 'sqlx::query[^_!a-zA-Z]'` scans
both report **zero** unverified call sites in `crates/domain/analytics/src/`.
No allowlist additions were required.

| Path                                                    | Decision  | Reason |
|---------------------------------------------------------|-----------|--------|
| `repository/events.rs` (×2)                             | KEEP      | Already `sqlx::query!` macro (compile-time verified) |
| `repository/funnel/mutations.rs` (×7)                   | KEEP      | Already `sqlx::query!` macro |
| `repository/funnel/stats.rs`                            | KEEP      | Already `sqlx::query!` macro |
| `repository/session/behavioral.rs` (×3)                 | KEEP      | Already `sqlx::query!` macro |
| `repository/session/mutations.rs` (×13)                 | KEEP      | Already `sqlx::query!` macro |
| `repository/session/queries.rs`                         | KEEP      | Already `sqlx::query!` macro |
| `repository/session/behavioral_queries.rs`              | KEEP      | New file, all `sqlx::query!`/`query_scalar!`/`query_as!` |
| `repository/conversations.rs`                           | KEEP      | Already `sqlx::query!` macro |
| `repository/fingerprint/mutations.rs` (×5)              | KEEP      | Already `sqlx::query!` macro |
| `repository/engagement.rs`                              | KEEP      | Already `sqlx::query!` macro |
| `repository/queries.rs::AnalyticsQueryRepository`       | KEEP      | Uses `db_pool.fetch_all` against the `DatabaseProvider` trait — the supported dynamic-SQL path; not a `sqlx::query` call |

---

## Public-API typed error boundary

`AnalyticsError` (in `src/error.rs`) is the canonical error type. New
variants added in Wave C2:

* `Repository(#[from] systemprompt_database::RepositoryError)` — propagates
  database-layer errors.
* `Serialization(#[from] serde_json::Error)` — for JSON-decoded analytics
  payloads.
* `MissingField(String)` — replaces former `anyhow!("Missing X")` macros in
  `repository/queries.rs`.
* `InvalidArgument(String)` — caller misuse.
* `Io(#[from] std::io::Error)` — filesystem / GeoIP DB.
* `Other(#[from] anyhow::Error)` — escape hatch for upstream dynamic errors;
  exists so internal helpers can absorb stray `anyhow::Error` without forcing
  per-site mapping.

`pub type Result<T> = std::result::Result<T, AnalyticsError>;` is re-exported
from the crate root as both `Result` and `AnalyticsResult` (the historical
public name).

---

## File-split outcomes

| Original                                                              | Lines | Split into |
|-----------------------------------------------------------------------|------:|-----------|
| `services/extractor.rs`                                               | 308 | `services/extractor/mod.rs` (263) + `services/extractor/geoip.rs` (94) |
| `repository/session/queries.rs`                                       | 345 | `repository/session/queries.rs` (156) + `repository/session/behavioral_queries.rs` (218) |
| `services/behavioral_detector/checks.rs`                              | 401 | `services/behavioral_detector/checks.rs` (208) + `services/behavioral_detector/fingerprint_checks.rs` (138) + `services/behavioral_detector/helpers.rs` (84) |

After Wave C2 no file in `crates/domain/analytics/src/` exceeds 300 lines.

---

## `.ok()` carve-outs (extractor.rs)

All six `.ok()` calls live in `services/extractor/mod.rs` and are
`HeaderValue::to_str().ok()` — converting an HTTP header value to a `&str`.
A non-ASCII header is not actionable for the analytics pipeline and must
**not** abort session creation; treating the value as absent is the correct
fallback. Documented in the module-level `//!` header at the top of
`extractor/mod.rs`.

---

## `#[allow(...)]` removals

| Original                                              | Fix |
|-------------------------------------------------------|------|
| `models/mod.rs::#[allow(unused_imports)]` over `pub use cli::*;` | Removed; the glob is genuinely used. |
| `models/cli/request.rs::CostSummaryRow::#[allow(clippy::struct_field_names)]` | Renamed `total_requests`/`total_cost`/`total_tokens` → `requests`/`cost`/`tokens`; SQL aliases and CLI consumer (`entry/cli/.../costs/summary.rs`) updated. |
| `repository/tools/list_queries.rs::list_tools_with_filter::#[allow(clippy::too_many_arguments)]` | Refactored to take `&ToolListParams<'_>, pattern: &str`. |

---

## Cross-cut shims

`AnalyticsError` does not implement `From<AnalyticsError>` for the upstream
`ProviderError`. Three call sites in `crates/app/scheduler/src/jobs/`
(`cleanup_inactive_sessions.rs`, `behavioral_analysis.rs`) propagate via
`.map_err(anyhow::Error::new)?` — `ProviderError: From<anyhow::Error>`
already, so the route is single-hop.

`scheduler::jobs::behavioral_analysis::log_flag_result` now takes
`&systemprompt_analytics::AnalyticsResult<()>` (it logs the result of an
analytics-typed call). `log_ban_result` continues to take
`&Result<(), anyhow::Error>` because it logs a `users::BannedIpRepository`
result, which still uses `anyhow`.

---

## Verification gate

| Gate                                                              | Status |
|-------------------------------------------------------------------|--------|
| `cargo fmt`                                                       | PASS |
| `cargo build --workspace`                                         | PASS |
| `cargo build -p systemprompt-analytics`                           | PASS |
| `cargo clippy --workspace --all-targets --all-features -D warnings` | PASS |
| `cargo clippy -p systemprompt-analytics --all-features --all-targets -D warnings` | PASS |
| `RUSTDOCFLAGS=-D warnings cargo doc -p systemprompt-analytics --all-features --no-deps` | PASS |
| `just lint-sqlx`                                                  | PASS |
| `cargo build` for `systemprompt-analytics-tests`                  | PASS |

`just check-bans-crate systemprompt-analytics` reports six pre-existing
`sqlx::query()` test-helper inserts in `crates/tests/unit/domain/analytics/src/repository/costs.rs`. These are out-of-scope for the source crate
sweep; `crates/domain/analytics/src/` itself has no violations.

---

## Verdict

**CLEAN**
