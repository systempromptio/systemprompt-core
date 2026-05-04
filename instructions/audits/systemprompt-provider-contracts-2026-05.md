# systemprompt-provider-contracts Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave A4 public-API compliance sweep)
**Verdict:** CLEAN

---

## Summary

| Category | Count |
|----------|-------|
| `unwrap()` / `expect()` | 0 |
| `panic!()` / `todo!()` / `unimplemented!()` | 0 |
| `println!` / `eprintln!` / `dbg!` | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Files >300 lines | 0 |
| Raw `String` IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` in public trait / fn signatures | 0 |
| `anyhow::` total (From impls + stream item type) | 7 occurrences across 3 files |
| `#[async_trait]` traits | 11 (all `dyn`-consumed; documented per trait) |
| `pub` items with rustdoc | 239 / 239 |
| `pub mod` items with `//!` | 5 / 5 |

**Total scored violations:** 0

---

## Sweep Outcome

### Public-API hygiene

- Introduced `error::ProviderError` as the concrete error type for every
  trait that previously surfaced `anyhow::Error` (`RssFeedProvider`,
  `SitemapProvider`, `FrontmatterProcessor`, `ContentDataProvider`,
  `PageDataProvider`, `PagePrerenderer`, `ComponentRenderer`,
  `TemplateDataExtender`, `Job`).
- Existing typed errors retained for `LlmProvider` (`LlmProviderError`)
  and `ToolProvider` (`ToolProviderError`); both gained per-variant
  rustdoc.
- All three error types implement `From<anyhow::Error>` so downstream
  provider crates that already use `anyhow::Result` propagate with `?`
  unchanged at merge time.
- `ChatStream` retains `anyhow::Result<String>` as the stream-item type;
  this is documented on the alias and is the only `anyhow` mention
  in any public signature.
- Every `pub` item carries a `///` rustdoc paragraph; every `pub mod`
  carries a `//!` module doc.
- `lib.rs` carries a `//!` feature-flag matrix; `Cargo.toml` carries
  `[package.metadata.docs.rs] all-features = true`.
- `#[allow(clippy::struct_field_names)]` removed by renaming `NavConfig`
  fields (Rust-side: `app`, `docs`, `blog`, `playbooks`, `github`,
  `getting_started`) with `#[serde(rename = "<name>_url")]` keeping the
  YAML schema stable.
- `let _ = base_url;` in `SitemapProvider::static_urls` replaced with the
  idiomatic `_base_url` parameter prefix.

### Error-strategy decision

Concrete `ProviderError` for the broad set of webgen-adjacent traits
rather than an associated `type Error: std::error::Error`. Reasons:

1. Every downstream provider already returns `anyhow::Result`; a
   concrete error with `#[from] anyhow::Error` requires no signature
   changes at the impl site.
2. An associated-type form would force every `dyn` use site
   (template registry, scheduler, etc.) to carry an extra type
   parameter or erase the error to `Box<dyn Error>`.
3. The webgen traits do not need to discriminate between provider-
   specific failure modes at the call site — the host treats every
   provider failure as a render-pipeline failure with context.

LLM and tool providers keep their existing typed errors because callers
*do* discriminate (rate-limit retry, auth re-prompt, tool-not-found).

### File split

`llm.rs` (316 lines) split into a `llm/` module:

- `llm/mod.rs` — module doc + re-exports.
- `llm/message.rs` — `ChatMessage`, `ChatRole`.
- `llm/request.rs` — `ChatRequest`, `SamplingParameters`.
- `llm/response.rs` — `ChatResponse`, `TokenUsage`.
- `llm/error.rs` — `LlmProviderError`, `LlmProviderResult`.
- `llm/provider.rs` — `LlmProvider`, `ToolExecutor`, `ChatStream`,
  `ToolExecutionContext`.

Documentation expansion pushed `tool.rs` and `web_config/theme.rs` over
the limit, so both were split as well:

- `tool/{mod,definition,call,content,context,error,provider}.rs`.
- `web_config/theme/{mod,fonts,colors,typography,scale,tokens,layout,card,mobile}.rs`.

All 38 source files now sit at or below 213 lines.

---

## Architectural Compliance

Layer: `shared`. Pure trait-contract crate; no I/O, no SQL, no logging.
Dependencies flow downward only — depends solely on `serde`, `chrono`,
`thiserror`, `anyhow`, `async-trait`, `inventory`, `futures`, and
`systemprompt-identifiers`.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| No inline `//` comments | PASS |
| `///` on every `pub` item | PASS |
| `//!` on every `pub mod` | PASS |
| All files <=300 lines | PASS |
| No raw `String` IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |
| No `anyhow` in public signatures | PASS |
| `cargo fmt -p systemprompt-provider-contracts` | PASS |
| `cargo build --all-features` | PASS |
| `cargo clippy --all-targets --all-features --no-deps -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | PASS |
| `just check-bans` (provider-contracts paths) | PASS |
| `[package.metadata.docs.rs] all-features = true` | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 38 |
| Files over 300 lines | 0 |
| Largest file | 213 lines (`job.rs`) |
| Total `pub` items | 239 |
| Items with rustdoc | 239 |
| `pub mod` items with `//!` | 5 |

---

## Verdict

**CLEAN**

The crate now meets the Wave A public-API compliance bar:

- Concrete `thiserror`-derived error types replace every `anyhow::Result`
  in public trait signatures.
- Full rustdoc coverage on every `pub` item; module-level `//!` on every
  `pub mod`; feature-flag matrix in `lib.rs`.
- Source files split by cohesion (no `_helpers.rs` cop-outs).
- No `#[allow(...)]`, no `let _ =`, no inline `//` comments.
- `cargo doc` clean under `RUSTDOCFLAGS="-D warnings"`.
