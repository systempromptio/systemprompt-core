# Entry Layer Checklist

**Layer:** `crates/entry/`

---

## Required

- [ ] `main.rs` or public lib API exists
- [ ] README.md exists
- [ ] status.md exists

## Handler Quality (API)

- [ ] Handlers: extract → delegate → respond only
- [ ] No business logic in handlers
- [ ] No direct SQL
- [ ] No direct repository access
- [ ] Request context extracted for tracing
- [ ] Proper error conversion to HTTP status
- [ ] Typed request/response models

## Command Quality (TUI)

- [ ] Commands delegate to services
- [ ] No business logic in commands
- [ ] Clap derive macros used
- [ ] `SystemSpan::new("cli")` for logging
- [ ] User-friendly error messages

## Wiring Quality

- [ ] All dependencies constructed in AppServices
- [ ] Repositories wrapped in Arc
- [ ] Services receive dependencies via constructor
- [ ] No global state in handlers

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
