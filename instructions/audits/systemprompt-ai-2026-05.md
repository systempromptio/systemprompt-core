# systemprompt-ai Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04 (Wave C3 sweep)
**Verdict:** CLEAN

---

## Summary

| Category | Before | After |
|----------|--------|-------|
| `unwrap()` / `expect()` | 0 | 0 |
| `panic!()` / `todo!()` / `unimplemented!()` | 0 | 0 |
| `println!` / `eprintln!` / `dbg!` | 0 | 0 |
| `let _ =` discards | 0 | 0 |
| `.ok()` discards | 6 | 6 (all annotated `// Why:`) |
| Inline `//` comments | 0 | 6 (`// Why:` carve-outs) |
| Doc `///` / `//!` comments | 0 | 60+ |
| Files >300 lines | 1 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |
| `anyhow::Result` in PUBLIC signatures | 121 | 0 |
| `anyhow::` references (total, internal carve-out) | 121 | 107 |
| `#[async_trait]` references | 18 | 18 (all on `dyn`-used traits) |

---

## Wave C3 Changes

### Typed-error sweep
- `crate::error::AiError` extended with `#[from]` impls for `LlmProviderError`,
  `reqwest::Error`, `std::io::Error`, `regex::Error`,
  `systemprompt_traits::ToolProviderError`,
  `systemprompt_config::SecretsBootstrapError`, plus a transparent
  `Internal(anyhow::Error)` carve-out for legacy helpers.
- Crate-level `Result<T>` alias re-routed to `Result<T, AiError>`.
- All public signatures across `crate::services::*` migrated from
  `anyhow::Result` to `crate::error::Result`.
- `provider_impl.rs` (the `AiProvider for AiService` bridge) continues to box
  errors into `ProviderResult` — unchanged from Wave A merge.
- Internal helpers (`stream_wrapper.rs`, `request_logging.rs`,
  `service.rs::store_error`) accept `&dyn std::fmt::Display` so they work with
  both `AiError` and `anyhow::Error` callers.

### Rustdoc
- `lib.rs` rewritten with crate-level `//!` overview, error-model section,
  feature-flag note, and `[package.metadata.docs.rs] all-features = true`.
- `error.rs` carries module-level `//!` plus `///` on every variant and
  variant field.
- All top-level pub modules (`services`, `repository`, `models`, plus their
  pub submodules — `core`, `core/ai_service`, `core/request_storage`,
  `providers`, `providers/anthropic`, `providers/openai`, `providers/gemini`,
  `tools`, `tooled`, `storage`, `structured_output`, `schema`, `config`)
  carry `//!` headers.
- `cargo doc -D warnings` passes clean (no broken intra-doc links).

### .ok() carve-outs
| Site | Justification |
|------|---------------|
| `services/core/request_storage/async_operations.rs:store_request_async` | best-effort audit-trail storage; logged before drop |
| `services/providers/gemini/streaming.rs:parse_stream_chunks` | partial SSE chunk; logged at debug, skip rather than terminate stream |
| `services/tools/adapter.rs` (4 sites) | optional `model_config` / `meta` metadata; logged warn, preserve rest of conversion |

### Sqlx
- `just lint-sqlx` ✅ clean — all 10 audit-flagged sites already used the
  verified `sqlx::query!()` macro; the audit's "raw sqlx" count was a
  false-positive from the regex matching the `query!` form.

### File splits
- `models/providers/anthropic.rs` was the only >300-line file in the prior
  audit; subsequent reorganisation already moved it under 300. No new files
  pushed over the limit by rustdoc additions (largest is now
  `services/providers/anthropic/generation.rs` at 297 lines).

### Async trait
- 18 `#[async_trait]` annotations remain. All are on traits used as `dyn` —
  `AiProvider`, `ImageProvider`, `ToolProvider` (foreign), and the gemini /
  openai / anthropic provider trait impls. Native `async fn in trait` is not
  yet usable here because the dispatch surface is dynamic.

---

## Verification

```
cargo fmt -p systemprompt-ai             ✅
cargo build -p systemprompt-ai           ✅
cargo clippy -p systemprompt-ai -D warn  ✅
RUSTDOCFLAGS=-D warnings cargo doc       ✅
just check-bans-crate crates/domain/ai   ✅
just lint-sqlx                           ✅
cargo build --workspace                  ✅
cargo clippy --workspace -D warnings     ✅
```

---

## Verdict

**CLEAN**
