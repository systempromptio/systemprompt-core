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
cargo clippy -p systemprompt-generator --no-deps -- -D warnings  # PASS
```

---

## Violations Fixed (This Review)

| File:Line | Violation | Fix Applied |
|-----------|-----------|-------------|
| `src/content/cards.rs:30` | `convert_external_url_to_local` not const | Added `const fn` |
| `src/sitemap/generator.rs:211` | Unnecessary `Result` wrapper | Return `SitemapUrl` directly |
| `src/content/cards.rs:36` | `map_err` instead of `inspect_err` | Use idiomatic `inspect_err` |
| `src/templates/paper.rs:27` | `map_err` instead of `inspect_err` | Use idiomatic `inspect_err` |
| `src/templates/html.rs:241` | `.ok()` without logging | Added `inspect_err` with logging |

---

## Previous Violations Fixed

| File:Line | Violation | Fix Applied |
|-----------|-----------|-------------|
| `src/templates/data.rs` | 480 lines (exceeded 300 limit) | Split into `data/` module with 4 files |
| `src/sitemap/generator.rs:194-198` | Direct SQL query | Replaced with `ContentRepository::list_by_source` |
| `src/templates/html.rs:59,171` | TODO comments | Extracted `source_id` from item properly |
| `src/content/cards.rs:38,50,52,56,62` | Inline comments | Removed all inline comments |
| `src/content/cards.rs:32` | Direct `env::var()` | Removed external domain conversion (no-op) |
| `src/assets.rs:17,37,82` | Direct `env::var()` | Simplified functions, removed env var dependencies |
| `src/templates/engine.rs:5,27,40` | Direct `env::var()` | Use `Config::get()` |
| `src/rss/generator.rs:34` | Direct `env::var()` | Use `Config.api_external_url` |
| `src/sitemap/generator.rs:45` | Direct `env::var()` | Use `Config.api_external_url` |
| `src/jobs/copy_assets.rs:1-3` | Import ordering | Fixed with `cargo fmt` |

---

## File Line Counts

All files under 300 lines limit:

| File | Lines |
|------|-------|
| `src/templates/html.rs` | 278 |
| `src/prerender/content.rs` | 266 |
| `src/templates/paper.rs` | 246 |
| `src/templates/navigation.rs` | 246 |
| `src/prerender/parent.rs` | 243 |
| `src/templates/data/mod.rs` | 230 |
| `src/sitemap/generator.rs` | 226 |
| `src/jobs/publish_content.rs` | 212 |
| `src/build/steps.rs` | 198 |
| `src/prerender/index.rs` | 187 |
| `src/prerender/context.rs` | 185 |
| `src/build/validation.rs` | 177 |
| `src/build/orchestrator.rs` | 139 |
| `src/content/cards.rs` | 135 |
| `src/templates/data/extractors.rs` | 126 |
| `src/jobs/copy_assets.rs` | 114 |
| `src/prerender/homepage.rs` | 103 |
| `src/prerender/fetch.rs` | 103 |
| `src/rss/generator.rs` | 102 |
| `src/templates/items.rs` | 87 |
| `src/rss/xml.rs` | 84 |
| `src/templates/data/builders.rs` | 79 |
| `src/templates/data/types.rs` | 73 |
| `src/sitemap/xml.rs` | 71 |
| `src/content/markdown.rs` | 56 |
| `src/assets.rs` | 54 |
| `src/templates/engine.rs` | 33 |
| `src/api.rs` | 32 |
| `src/lib.rs` | 24 |
| `src/prerender/engine.rs` | 19 |
| `src/templates/mod.rs` | 14 |
| `src/prerender/mod.rs` | 12 |
| `src/content/mod.rs` | 9 |
| `src/build/mod.rs` | 6 |
| `src/jobs/mod.rs` | 6 |
| `src/rss/mod.rs` | 6 |
| `src/sitemap/mod.rs` | 6 |

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
- No inline comments
- No TODO/FIXME comments
- Uses `inspect_err()` pattern for logging before `.ok()`
- Uses `const fn` where appropriate
