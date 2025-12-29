# Infrastructure Layer Checklist

**Layer:** `crates/infra/`

---

## Boundary Rules

- [ ] No domain crate imports (`systemprompt-users`, `systemprompt-ai`, etc.)
- [ ] No app layer imports (`systemprompt-scheduler`)
- [ ] No entry layer imports (`systemprompt-api`, `systemprompt-tui`)
- [ ] Only `shared/` crate dependencies allowed
- [ ] No domain-specific repositories
- [ ] No business logic

## Required

- [ ] README.md exists
- [ ] status.md exists

## Statelessness

- [ ] No domain entities stored
- [ ] No business state maintained
- [ ] Utilities are reusable across domains

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
