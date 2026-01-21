# systemprompt-generator Compliance

**Layer:** Application
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | :x: |
| Required Structure | :white_check_mark: |
| Code Quality | :x: |
| Orchestration Quality | :white_check_mark: |
| Idiomatic Rust | :x: |

---

## Violations

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/templates/data.rs` | 480 lines (exceeds 300 limit) | Code Quality |
| `src/sitemap/generator.rs:194-198` | Direct SQL query in application layer | Boundary Rules |
| `src/templates/html.rs:59` | TODO comment | Code Quality |
| `src/templates/html.rs:60` | Inline comment | Code Quality |
| `src/templates/html.rs:171` | TODO comment | Code Quality |
| `src/content/cards.rs:38` | Inline comment | Code Quality |
| `src/content/cards.rs:50` | Inline comment | Code Quality |
| `src/content/cards.rs:52` | Inline comment | Code Quality |
| `src/content/cards.rs:56` | Inline comment | Code Quality |
| `src/content/cards.rs:62` | Inline comment | Code Quality |
| `src/content/cards.rs:32` | Direct `env::var()` with `unwrap_or_else` fallback | Anti-Pattern |
| `src/content/cards.rs:41` | `.ok()` swallowing error without logging | Anti-Pattern |
| `src/content/cards.rs:47` | `.ok()?` swallowing error without logging | Anti-Pattern |
| `src/assets.rs:17` | Direct `std::env::var()` usage | Anti-Pattern |
| `src/assets.rs:37` | Direct `std::env::var()` usage | Anti-Pattern |
| `src/assets.rs:82` | Direct `std::env::var()` usage | Anti-Pattern |
| `src/templates/engine.rs:5` | Direct `std::env::var()` with `.ok()` | Anti-Pattern |
| `src/templates/engine.rs:27` | Direct `std::env::var()` usage | Anti-Pattern |
| `src/templates/engine.rs:40` | Direct `std::env::var()` usage | Anti-Pattern |
| `src/rss/generator.rs:34` | Direct `std::env::var()` with fallback | Anti-Pattern |
| `src/sitemap/generator.rs:45` | Direct `std::env::var()` with fallback | Anti-Pattern |
| `src/templates/paper.rs:32` | `.ok()` swallowing error without logging | Anti-Pattern |
| `src/templates/html.rs:241` | `.ok()` swallowing error without logging | Anti-Pattern |
| `src/jobs/copy_assets.rs:1-3` | Import ordering (fmt --check fails) | Code Quality |

---

## Commands Run

```
cargo clippy -p systemprompt-generator -- -D warnings  # PASS
cargo fmt -p systemprompt-generator -- --check          # FAIL
```

---

## Actions Required

1. **Split `templates/data.rs`** - Extract helper structs and functions into separate modules to reduce file length below 300 lines
2. **Move SQL to repository** - The raw SQL in `sitemap/generator.rs:194-198` should use a repository pattern via `ContentRepository` or a dedicated `SitemapRepository`
3. **Remove TODO comments** - Fix the source_name extraction issue in `templates/html.rs:59-60,171` or remove the TODOs
4. **Remove inline comments** - Delete all inline comments in `content/cards.rs` and `templates/html.rs`; code should be self-documenting
5. **Replace direct `env::var()` calls** - Use `Config::get()` or `AppPaths` for configuration access across all files:
   - `content/cards.rs:32`
   - `assets.rs:17,37,82`
   - `templates/engine.rs:5,27,40`
   - `rss/generator.rs:34`
   - `sitemap/generator.rs:45`
6. **Fix `.ok()` usages** - Add proper error logging before converting to Option:
   - `content/cards.rs:41,47`
   - `templates/paper.rs:32`
   - `templates/html.rs:241`
   - `templates/engine.rs:5-9`
7. **Fix import ordering** - Run `cargo fmt` on `jobs/copy_assets.rs`

---

## Positive Observations

- Clippy passes with `-D warnings`
- Proper use of `?` operator for error propagation throughout
- Good use of iterator chains and combinators
- Jobs implement the `Job` trait interface correctly
- Orchestration pattern is mostly followed (coordinates domain services)
- Proper structured logging with `tracing`
- No `unwrap()` or `panic!()` usage
- No `unsafe` code
- Well-organized module structure with clear separation of concerns
