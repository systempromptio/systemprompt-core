# Domain Layer Checklist

**Layer:** `crates/domain/`

---

## Boundary Rules

- [ ] No cross-domain imports (`systemprompt-users` from `systemprompt-ai`, etc.)
- [ ] No app layer imports
- [ ] No entry layer imports
- [ ] Only `shared/` and `infra/` dependencies allowed

## Required Structure

- [ ] `module.yaml` exists at crate root
- [ ] `module.yaml` name matches directory name
- [ ] `src/repository/` directory exists
- [ ] `src/services/` directory exists
- [ ] `src/error.rs` exists
- [ ] README.md exists
- [ ] status.md exists

## module.yaml

- [ ] `name` field present and matches directory
- [ ] `version` field present (valid SemVer)
- [ ] `display_name` field present
- [ ] `type` field present (`infrastructure` or `core`)

## Repository Quality

- [ ] All queries use SQLX macros (`query!`, `query_as!`, `query_scalar!`)
- [ ] No runtime query strings (`sqlx::query()` without `!`)
- [ ] No business logic in repositories
- [ ] Typed IDs used (not raw strings)
- [ ] Pool is `Arc<PgPool>`

## Service Quality

- [ ] Repositories injected via constructor
- [ ] Request context as first parameter for request-scoped ops
- [ ] `let _guard = req_ctx.span().enter();` before logging
- [ ] Structured logging: `info!(field = %value, "message")`
- [ ] Errors mapped to domain error types
- [ ] No direct SQL in services

## Idiomatic Rust

- [ ] Iterator chains over imperative loops
- [ ] `?` operator for error propagation
- [ ] No unnecessary `.clone()`
- [ ] `impl Into<T>` / `AsRef<T>` for flexible APIs
- [ ] Derive ordering: `Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`
- [ ] `thiserror` for domain error types

## Code Quality

- [ ] File length ≤ 300 lines
- [ ] Cognitive complexity ≤ 15
- [ ] Function parameters ≤ 5
- [ ] No `unsafe`
- [ ] No `unwrap()` / `panic!()`
- [ ] No inline comments (`//`)
- [ ] No TODO/FIXME
- [ ] `cargo clippy -p {crate} -- -D warnings` passes
- [ ] `cargo fmt -p {crate} -- --check` passes
