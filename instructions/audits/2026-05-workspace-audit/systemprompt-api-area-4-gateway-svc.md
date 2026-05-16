# systemprompt-api — Area 4: gateway service

Scope: `crates/entry/api/src/services/gateway/` (entry binary crate; `anyhow` permitted, per-item `///` banned).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only depends on domain (`systemprompt-ai`), infra (`database`, `config`), shared (`models`, `identifiers`) — downward only. |
| 2 | Error model | clean | `anyhow` at dispatch boundary; typed `thiserror` errors (`PolicyDenied`, `QuotaExceeded`, `InboundParseError`) where consumed structurally. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`; `unwrap_or_else` used throughout for fallible JSON/response builds. |
| 4 | Raw SQL | clean | All DB access via `systemprompt-ai` repositories; no `sqlx::query` in scope. |
| 5 | File size | remediated | `stream_tap.rs` (332)→`stream_tap/{mod,accumulator}.rs`; `audit.rs` (323)→`audit/{mod,message_text,payload}.rs`; `service.rs` (314)→`service/{mod,finalize}.rs`. All <300. |
| 6 | Function size | clean | `dispatch` ~130 lines but is a linear pipeline; sub-steps already extracted into `quota`, `finalize`, `policy`. No cohesive further split without artificial seams. |
| 7 | Async traits | remediated | Added `// Why:` justification on `#[async_trait] SafetyScanner` and `OutboundAdapter` — both held as trait objects, must stay dyn-compatible. |
| 8 | Typed identifiers | clean | Typed IDs throughout (`AiRequestId`, `UserId`, `TenantId`, etc.); `.into()` calls are JSON map/value construction, not ID call sites. |
| 9 | Comment standard | remediated | Stripped per-item `///` from `service.rs` (`DispatchInputs`), `pricing.rs` (`resolve`), `canonical.rs` (`derived_gateway_conversation_id`); `//!` heads kept/added on split modules. |
| 10 | No legacy | clean | No shims/dual paths/stubs; removed now-unused `CanonicalContent` import after extraction. |
| 11 | Naming | clean | `GatewayService`, `PolicyResolver`, `HeuristicScanner`, `*Adapter`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests` in scope. |
| 13 | Local duplication | remediated | Extracted duplicated `take_summary` (EOF + Drop snapshot logic) in `stream_tap`; shared accumulation logic isolated in `accumulator.rs`. |
| 14 | CHANGELOG | clean | Not edited (observations only). |

Verification: `SQLX_OFFLINE=true cargo clippy -p systemprompt-api --all-targets --all-features -- -D warnings` and `cargo doc -p systemprompt-api --no-deps` both clean.
