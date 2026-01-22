# systemprompt.io Clippy Linting Standards

**Reference:** This document extends [rust.md](../rust/rust.md). All code must meet world-class idiomatic Rust standards.

---

## Enforcement

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

Zero tolerance for warnings. All clippy lints are enforced at the workspace level.

---

## Workspace Lint Configuration

All crates inherit from `[workspace.lints.clippy]` in root `Cargo.toml`. No per-crate overrides except test crates.

### Lint Groups

| Group | Level | Purpose |
|-------|-------|---------|
| `all` | deny | Core correctness lints |
| `pedantic` | deny | Stricter code quality |
| `nursery` | warn | Experimental quality lints |
| `cargo` | warn | Cargo manifest quality |
| `perf` | warn | Performance suggestions |
| `suspicious` | deny | Potentially incorrect code |

### Denied Lints (Errors)

These lints cause compilation to fail:

| Lint | Rationale |
|------|-----------|
| `unwrap_used` | Use `?`, `ok_or_else()`, or `expect()` with context |
| `panic` | Return `Result`, never panic in library code |
| `unimplemented` | Implement or return `Result::Err` |
| `todo` | Complete implementation before merge |
| `too_many_arguments` | Refactor into builder pattern or config struct |
| `dbg_macro` | Remove debug macros before merge |
| `exit` | Only allowed in CLI entry points with justification |
| `rc_mutex` | Use `Arc<Mutex<T>>` or redesign |

### Warned Lints (Must Address)

| Lint | Action Required |
|------|-----------------|
| `cognitive_complexity` | Refactor function, extract helpers |
| `too_many_lines` | Split into smaller functions (limit: 75 lines) |
| `type_complexity` | Create type alias |
| `expect_used` | Prefer `?` operator, use `expect` only with clear message |
| `inefficient_to_string` | Use `to_owned()` or avoid allocation |
| `unnecessary_wraps` | Remove unnecessary `Result`/`Option` wrapper |
| `unused_async` | Remove `async` or add async operation |
| `if_not_else` | Invert condition for clarity |
| `redundant_else` | Remove unnecessary `else` after early return |
| `manual_let_else` | Use `let ... else { }` pattern |
| `match_bool` | Use `if`/`else` instead |
| `option_if_let_else` | Use combinators: `map_or`, `map_or_else` |
| `needless_pass_by_value` | Take `&T` instead of `T` |
| `items_after_statements` | Move items before statements |
| `semicolon_if_nothing_returned` | Add semicolon for clarity |
| `or_fun_call` | Use `unwrap_or_else` instead of `unwrap_or` with fn call |
| `redundant_clone` | Remove unnecessary `.clone()` |
| `unnecessary_to_owned` | Avoid `.to_owned()` when reference suffices |
| `implicit_clone` | Make cloning explicit |
| `large_futures` | Box large futures |
| `match_wild_err_arm` | Handle specific error variants |
| `print_stdout` | Use `tracing` for output |
| `print_stderr` | Use `tracing` for errors |
| `empty_structs_with_brackets` | Remove `{}` from unit structs |
| `rest_pat_in_fully_bound_structs` | Bind all fields explicitly |
| `clone_on_ref_ptr` | Use `Arc::clone(&x)` for clarity |
| `separated_literal_suffix` | Use `100_u32` not `100u32` |
| `try_err` | Use `?` operator instead of `return Err` |

### Allowed Lints (Workspace Exceptions)

These are allowed at workspace level due to practical constraints:

| Lint | Reason |
|------|--------|
| `cargo_common_metadata` | Not publishing to crates.io |
| `multiple_crate_versions` | Transitive dependency conflicts |
| `return_self_not_must_use` | Builder pattern returns |
| `must_use_candidate` | Too many false positives |
| `trivially_copy_pass_by_ref` | Consistency over micro-optimization |
| `cast_possible_truncation` | TUI coordinate math requires casts |
| `cast_sign_loss` | TUI coordinate math requires casts |
| `cast_precision_loss` | Progress bar calculations |
| `cast_possible_wrap` | TUI coordinate math |
| `format_push_string` | String building patterns |
| `uninlined_format_args` | Readability preference |
| `result_unit_err` | Legacy error patterns |
| `missing_docs_in_private_items` | No rustdoc requirement |
| `module_name_repetitions` | Domain naming clarity |
| `missing_errors_doc` | No rustdoc requirement |
| `missing_panics_doc` | No rustdoc requirement |
| `derive_partial_eq_without_eq` | Generated code compatibility |

---

## Inline Allow Policy

**Default: FORBIDDEN**

Inline `#[allow(clippy::...)]` attributes are prohibited except for cases documented in [exceptions.md](./exceptions.md).

### Process for New Exceptions

1. Verify the lint cannot be fixed through refactoring
2. Document the technical constraint requiring the exception
3. Add to `exceptions.md` with file path and justification
4. Include explanatory comment on the allow attribute

### Exception Categories

Only these categories may qualify for exceptions:

| Category | Example |
|----------|---------|
| **Serde/Serialization** | Empty structs for JSON `{}` |
| **FFI/External Constraints** | Generic closures in async contexts |
| **Fundamental Type Design** | Capability flags requiring many bools |
| **CLI Entry Points** | Intentional `exit()` for fatal errors |

---

## Test Crates

Test crates (`crates/tests/**`) may have relaxed lints:

```toml
[lints.clippy]
expect_used = "allow"
unwrap_used = "allow"
panic = "allow"
```

This does NOT apply to:
- Unit tests in library crates
- Integration tests in `src/` directories
- Any production code

---

## Rust Lints

| Lint | Level |
|------|-------|
| `unsafe_code` | forbid |
| `missing_debug_implementations` | warn |
| `missing_copy_implementations` | warn |
| `trivial_casts` | warn |
| `trivial_numeric_casts` | warn |
| `unused_import_braces` | warn |
| `unused_qualifications` | warn |
| `variant_size_differences` | warn |

---

## Pre-Commit Checklist

Before every commit:

1. `cargo clippy --workspace -- -D warnings` passes
2. `cargo fmt --all` applied
3. No new inline `#[allow(...)]` without exception documentation
4. All `cognitive_complexity` warnings addressed or justified
5. No `todo!()`, `unimplemented!()`, or `panic!()` in library code
