# systemprompt-traits Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave A2 public-API compliance sweep)
**Verdict:** CLEAN

---

## Summary (post-sweep)

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in PUBLIC signatures | 0 |
| `anyhow::` references in `From<anyhow::Error>` adapters | 8 (acceptable — escape hatches, not in trait signatures) |

**Total scored violations:** 0

---

## Wave A2 Public-API Compliance Sweep

Branch: `compliance-wA2-traits`

### Anyhow before/after
- Before: 20 `anyhow` references, including the public `ContextPropagation::from_headers -> anyhow::Result<Self>` and `FileStorage` trait methods using `anyhow::Result`.
- After: 0 `anyhow` references in public trait/method/return-type signatures. Remaining `anyhow` references (8) are confined to `impl From<anyhow::Error> for *Error` adapter blocks plus two `Other(#[from] anyhow::Error)` enum variants in `RepositoryError` / `DomainConfigError`. These are documented escape hatches for legacy `anyhow`-using callers; they do not appear in trait method or return-type signatures.

### Typed errors introduced
- `FileStorageError` / `FileStorageResult` (replaces `anyhow::Result` in `FileStorage` trait).
- `ContextPropagationError` / `ContextPropagationResult` (replaces `anyhow::Result` in `ContextPropagation::from_headers`).
- `crates/shared/models/src/execution/context/propagation.rs` — `RequestContext` impl rewritten to return the new typed error.
- `crates/tests/common/mocks/src/file_storage.rs` — `MockFileStorage` rewritten to use `FileStorageError`.

### Rustdoc coverage before/after
- Before: 0 `///` items, no module-level `//!` docs on the bulk of `pub mod` declarations, no crate-level docs.
- After: every `pub mod` carries `//!`, every `pub` item carries `///`, crate root has full `//!` overview (layering, errors, async traits, feature-flag matrix). `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` is clean.

### Trait `async fn` modernization
Every async trait in this crate is consumed as `Arc<dyn TraitName>` via the public `Dyn*` aliases, so native `async fn` (which is not yet `dyn`-compatible without performance penalties or work-arounds) cannot be used.

The following traits keep `#[async_trait]` and are documented with the rationale ("`#[async_trait]` is required because the trait is consumed as `Arc<dyn …>` via [`Dyn…`]"):
`AnalyticsProvider`, `FingerprintProvider`, `UserProvider`, `RoleProvider`,
`ProcessCleanupProvider`, `FileUploadProvider`, `SessionAnalyticsProvider`,
`AiFilePersistenceProvider`, `AiSessionProvider`, `ContextProvider`,
`AgentRegistryProvider`, `McpRegistryProvider`, `JobTrigger`,
`SchedulerLifecycle`, `FileStorage`, `LogService`, `Service`, `AsyncService`,
`Module`, `ApiModule` (web), `ContentProvider`.

### Associated `Error` types vs concrete typed errors
Two traits intentionally keep an associated `Error` type so implementers
can pick the most precise error for their backend:
- `LogService` — `type Error: std::error::Error + Send + Sync;` because the
  trait is generic over storage backends with very different failure shapes
  (DB pool errors vs. file IO).
- `ContentProvider` — `type Error` for the same reason; content stores
  vary widely between domains.

Every other public trait uses a concrete crate-defined `thiserror` enum
because the failure modes are well-understood and uniform across
implementations. The `ExtensionError` cross-cutting trait formalises the
contract every concrete error must satisfy (`code`, `status`,
`is_retryable`, `user_message`, MCP/API rendering).

### File splits
- `ai_providers.rs` (326 lines) split into `ai_providers/{mod,error,image,files,sessions}.rs` to keep every file ≤ 300 lines.

### Self-verification gate
| Command | Result |
|---------|--------|
| `cargo fmt -p systemprompt-traits` | PASS |
| `cargo build -p systemprompt-traits --all-features` | PASS |
| `cargo clippy -p systemprompt-traits --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-traits --no-deps --all-features` | PASS |
| `just check-bans` (filtered to crate path) | PASS (no hits) |
| `cargo build --workspace` | PASS |
| `cargo check --manifest-path crates/tests/Cargo.toml --workspace` | PASS |

### Cargo metadata
`[package.metadata.docs.rs] all-features = true` added so docs.rs builds
the `web` feature surface.

---

## File Statistics (post-sweep)

| Metric | Value |
|--------|-------|
| Total .rs files | 33 |
| Files over 300 lines | 0 |
| Largest file | `crates/shared/traits/src/startup_events/events.rs` (257 lines) |

---

## Verdict

**CLEAN**

Every Wave 1 + Wave A2 compliance criterion is satisfied: zero `anyhow`
in public signatures, full rustdoc coverage, every trait documents the
rationale for `#[async_trait]` use, every public error is `thiserror`,
all files ≤ 300 lines, no `let _ =` / `.ok()` / banned constructs.
