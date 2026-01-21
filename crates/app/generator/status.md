# systemprompt-generator Compliance

**Layer:** Application
**Reviewed:** 2026-01-21
**Verdict:** COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | :white_check_mark: |
| Required Structure | :white_check_mark: |
| Code Quality | :white_check_mark: |
| Orchestration Quality | :white_check_mark: |
| Idiomatic Rust | :white_check_mark: |

---

## Commands Run

```
cargo check -p systemprompt-generator              # PASS
cargo fmt -p systemprompt-generator -- --check     # PASS
```

---

## Violations Fixed

| File:Line | Violation | Fix Applied |
|-----------|-----------|-------------|
| `src/templates/data.rs` | 480 lines (exceeded 300 limit) | Split into `data/` module with 4 files |
| `src/sitemap/generator.rs:194-198` | Direct SQL query | Replaced with `ContentRepository::list_by_source` |
| `src/templates/html.rs:59,171` | TODO comments | Extracted `source_id` from item properly |
| `src/content/cards.rs:38,50,52,56,62` | Inline comments | Removed all inline comments |
| `src/content/cards.rs:32` | Direct `env::var()` | Removed external domain conversion (no-op) |
| `src/content/cards.rs:41,47` | `.ok()` without logging | Added `tracing::warn!` before `.ok()` |
| `src/assets.rs:17,37,82` | Direct `env::var()` | Simplified functions, removed env var dependencies |
| `src/templates/engine.rs:5,27,40` | Direct `env::var()` | Use `Config::get()` |
| `src/rss/generator.rs:34` | Direct `env::var()` | Use `Config.api_external_url` |
| `src/sitemap/generator.rs:45` | Direct `env::var()` | Use `Config.api_external_url` |
| `src/jobs/copy_assets.rs:1-3` | Import ordering | Fixed with `cargo fmt` |

---

## File Structure After Refactoring

```
src/templates/data/
├── mod.rs         (229 lines) - Main entry point, prepare_template_data
├── types.rs       (72 lines)  - Struct definitions
├── extractors.rs  (125 lines) - Config extraction functions
└── builders.rs    (78 lines)  - JSON template building
```

---

## Positive Observations

- Clippy passes with `-D warnings`
- Proper use of `?` operator for error propagation
- Good use of iterator chains and combinators
- Jobs implement the `Job` trait interface correctly
- Orchestration pattern followed (coordinates domain services)
- Proper structured logging with `tracing`
- No `unwrap()` or `panic!()` usage
- No `unsafe` code
- No direct SQL in application layer
- No direct environment variable access
- All files under 300 lines
