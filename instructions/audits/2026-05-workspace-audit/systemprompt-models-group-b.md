# Audit — systemprompt-models Group B

Scope: `crates/shared/models/src/{profile,ai,a2a,execution}/`

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | No upward/cross-layer deps; only `systemprompt_identifiers` and external crates imported. |
| 2 | Error model | clean | `thiserror` enums in `profile/error.rs`, `context/context_error.rs`; no `anyhow` in public signatures. |
| 3 | No panics / prints | clean | Only `expect` is the permitted compile-time-constant `Regex::new` carve-out in `profile/mod.rs`. No `println!`/`dbg!`. |
| 4 | Raw SQL | clean | No `sqlx::query` anywhere in scope. |
| 5 | File size | clean | Largest is `profile/gateway.rs` at 245 lines; all under 300. |
| 6 | Function size | clean | `ai/tools/tool_call.rs::from_json_row` (87) and `execution/context/propagation.rs::from_headers` (114) exceed 75 lines, but are flat linear field-by-field row/header parsers with no cohesive extractable sub-steps; splitting would create forbidden `*_helpers.rs` padding. Left as-is per the guidance. |
| 7 | Async traits | remediated | `AiProvider` (`ai/provider_trait.rs`) uses `#[async_trait]`; added a `///` documenting it is required because the trait is consumed only as `DynAiProvider` (`Arc<dyn AiProvider>`). |
| 8 | Typed identifiers | clean | No raw `String` entity-ID struct fields; A2A `String` fields are the documented protocol carve-out; remaining `String` fields are model/provider/tool names, not typed IDs. |
| 9 | Comment standard | remediated | Replaced four generic placeholder `//!` heads (`profile/mod.rs`, `ai/mod.rs`, `execution/mod.rs`, `a2a/mod.rs`) with substantive module docs; added a `//!` head to `ai/tools/mod.rs`. Existing `///` in `gateway.rs` encodes genuine WHY (slug/hash invariants). |
| 10 | No legacy | clean | No backwards-compat shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | No `*Manager` types; descriptors/configs/builders named appropriately. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No extract-worthy repeated logic; per-field row parsing is inherently repetitive but not a duplication smell. |
| 14 | CHANGELOG accuracy | clean | Not edited (owned by another agent); changes here are doc-comment only, no behavioural delta. |
