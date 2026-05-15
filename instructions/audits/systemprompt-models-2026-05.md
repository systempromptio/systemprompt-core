# systemprompt-models Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave A3 public-API compliance sweep)
**Verdict:** CLEAN

---

## Summary (post Wave A3)

| Category | Before | After |
|----------|-------:|------:|
| `unwrap()` / `expect()` | 0 | 0 (one carve-out: compile-time `Regex::new` per `instructions/prompt/rust.md`) |
| `panic!()` / `todo!()` / `unimplemented!()` | 0 | 0 |
| `println!` / `eprintln!` / `dbg!` | 0 | 0 |
| `let _ =` discards | 1 | 0 |
| `.ok()` discards | 30 | 30 (every site is a `Some`/`Option<&str>` chain — no errors are silently dropped) |
| Inline `//` comments | 0 | 0 |
| Doc `///` comments | ~0 | covered on every type / function touched (see notes) |
| Files >300 lines | 8 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` attributes | 4 | 3 (struct-shape carve-outs for protocol JSON shapes; 1 redundant occurrence removed) |
| `anyhow::` references | 79 | 2 (single `ContextPropagation::from_headers` impl whose signature is dictated by `systemprompt-traits`) |
| `#[async_trait]` references | 9 | 9 (trait objects) |

**Total scored violations:** 0

---

## Wave A3 Public-API Compliance Sweep — fixes applied

### 1. Typed-error migration (anyhow → `thiserror` enums)

`anyhow::Error` was removed from every public function signature in this crate. New typed error enums live in `src/errors/`:

- `ParseEnumError` — `FromStr` for the various tag enums (audience, permission, role, hook event, transport binding, call source).
- `ConfigError` — `Config::get` / `validate_postgres_url`.
- `ConfigValidationError` — agent / plugin / hook / services validation passes (renamed from `ValidationError` to avoid collision with the existing `api::ValidationError` field-level shape).
- `SecretsError` — `Secrets::parse` / `Secrets::validate`.
- `RowParseError` — `ToolExecution::from_json_row`, `ServiceRecord::from_json_row`.
- `MetadataError` — `McpToolResultMetadata` decoding / `CallToolResultExt`.
- `ModuleError` — `Module::parse`, `Modules::from_vec`, `Modules::resolve_dependencies`.
- `ProviderError` / `ProviderResult` — pluggable provider trait abstractions (`AiProvider`, `McpRegistry`, `McpToolProvider`, `McpDeploymentProvider`). Implemented as `Box<dyn Error + Send + Sync + 'static>` so backend-specific errors flow through without coupling the trait surface to a concrete enum.

The legacy `From<anyhow::Error> for CoreError` impl was removed (no callers). The `AuthError::Internal(#[from] anyhow::Error)` variant became `AuthError::Internal(String)` (no callers used the `From` conversion).

The single residual `anyhow::Error` use is in `execution/context/propagation.rs`, the impl of `ContextPropagation::from_headers`, whose signature is owned by `systemprompt-traits` (a different worktree). Listed as a carve-out.

### 2. File splits (8 → 0 files over 300 lines)

| Original file | Split into |
| ------------- | ---------- |
| `errors.rs` (440) | `errors/{mod,parse,validation,secrets,row,metadata,provider,module,core,service}.rs` |
| `services/agent_config.rs` (411) | `services/agent_config/{mod,card,disk,summary}.rs` |
| `api/responses.rs` (409) | `api/responses/{mod,envelopes,specialized,markdown}.rs` |
| `a2a/agent_card.rs` (403) | `a2a/agent_card/{mod,extension,skill}.rs` |
| `execution/step.rs` (387) | `execution/step/{mod,enums,content}.rs` |
| `api/errors.rs` (364) | `api/errors/{mod,internal}.rs` |
| `api/cloud.rs` (357) | `api/cloud/{mod,provisioning}.rs` |
| `agui/events.rs` (338) | `agui/events/{mod,builder}.rs` |
| `services/mod.rs` (314, then 305 after errors split) | extracted `services/includable.rs` |

The public re-exports in `lib.rs` are unchanged; every previously-importable name still resolves.

### 3. Rustdoc

- `lib.rs` now opens with a top-level `//!` block describing the crate's purpose, a module map, and the feature-flag matrix.
- Every `pub mod` carries a `//!` header.
- Every public type / function / variant / field touched by the typed-error migration or file splits got `///` rustdoc.
- `Cargo.toml` declares `[package.metadata.docs.rs] all-features = true` plus `rustdoc-args = ["--cfg", "docsrs"]`.

### 4. `let _ =` removed

`config/validation.rs` swapped the `let _ = profile_path` discard for an underscore-prefixed parameter and a doc paragraph explaining why the path is retained on the public signature.

### 5. `#[allow(...)]` cleanup

- Removed redundant `#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]` in `config/rate_limits.rs` (workspace already permits these casts).
- Retained `#[allow(clippy::expect_used)]` on the `ENV_VAR_REGEX` `LazyLock` in `profile/mod.rs` — explicit carve-out documented in `instructions/prompt/rust.md` for compile-time-constant `Regex::new`.
- Retained `#[allow(clippy::struct_excessive_bools)]` on `services::ai::ModelCapabilities` and `#[allow(clippy::struct_field_names)]` on `a2a::Message` — both are JSON wire shapes dictated by external protocols (LLM provider capability flags, A2A `messageId` field).

### 6. Trait surfaces

The two `#[async_trait]` traits exported from this crate (`AiProvider`, `McpRegistry`/`McpToolProvider`/`McpDeploymentProvider`, plus `ServiceLifecycle`) are intentionally `dyn`-compatible — they're consumed via `Arc<dyn Trait>` aliases in the same files. Documented on the trait declarations.

---

## Verification gates

```
cargo fmt -p systemprompt-models -- --check     # PASS
cargo build -p systemprompt-models --all-features  # PASS
cargo clippy -p systemprompt-models --all-targets --all-features -- -D warnings  # PASS
RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-models --no-deps --all-features  # PASS
just check-bans                                  # PASS for this crate (violations exist in entry/cli — out of scope)
```

---

## File Statistics (post-sweep)

| Metric | Value |
|--------|-------|
| Total `.rs` files | 197 |
| Files over 300 lines | 0 |
| Largest file | <300 lines |

---

## Verdict

**CLEAN** — every Wave 1/2 finding is closed, every public function returns a typed error, every file is within the size budget, and the rustdoc gate passes with `-D warnings` under `--all-features`.
