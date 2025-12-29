# Shared Layer Checklist

**Layer:** `crates/shared/`

---

## Boundary Rules

- [ ] No `sqlx` dependency
- [ ] No `tokio` runtime (types only)
- [ ] No `reqwest` / HTTP clients
- [ ] No `std::fs` / file system
- [ ] No `systemprompt-*` imports (except shared/)
- [ ] No `async fn` definitions
- [ ] No mutable statics
- [ ] No singletons

## Required

- [ ] README.md exists
- [ ] status.md exists

## Type Quality

- [ ] All IDs use typed wrappers from `systemprompt_identifiers`
- [ ] No `String` for domain identifiers
- [ ] No `Option<T>` masking required fields
- [ ] Builders for types with 3+ fields
- [ ] `#[serde(transparent)]` on ID types

## Idiomatic Rust

- [ ] Iterator chains over imperative loops
- [ ] `?` operator for error propagation
- [ ] No unnecessary `.clone()`
- [ ] `impl Into<T>` / `AsRef<T>` for flexible APIs
- [ ] Derive ordering: `Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`

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
