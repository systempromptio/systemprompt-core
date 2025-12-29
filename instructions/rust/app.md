# Application Layer Checklist

**Layer:** `crates/app/`

---

## Boundary Rules

- [ ] No entry layer imports (`systemprompt-api`, `systemprompt-tui`)
- [ ] No direct SQL for domain data (only job tracking allowed)
- [ ] No direct repository access (use services)
- [ ] Business logic delegated to domain services

## Required

- [ ] README.md exists
- [ ] status.md exists

## Orchestration Quality

- [ ] Coordinates domain services only
- [ ] No data transformation logic
- [ ] No validation logic
- [ ] Pure workflow execution

## Scheduler (if applicable)

- [ ] Jobs implement trait interface
- [ ] Job execution delegated to domain
- [ ] Status tracked via repository
- [ ] Failures logged and continued
- [ ] `SystemSpan::new("scheduler")` for logging

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
