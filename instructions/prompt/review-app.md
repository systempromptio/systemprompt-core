# Application Layer Review

> Review this module as though you were Steve Klabnik implementing world-class idiomatic Rust.

---

## Input

- **Folder:** `{crate_path}`
- **Checklist:** `/instructions/rust/app.md`
- **Standards:** `/instructions/rust/rust.md`

---

## Steps

1. Read all `.rs` files in `{crate_path}/src/`
2. Read `Cargo.toml`
3. Execute each checklist item from `/instructions/rust/app.md`
4. Verify orchestration pattern (services only, no direct repos)
5. For each violation, record: `file:line` + violation type
6. Generate `status.md` using output template

---

## Validation Commands

```bash
# Boundary checks
grep -E "systemprompt-(api|tui)" {crate_path}/Cargo.toml

# No domain SQL (except job tracking)
grep -rn "sqlx::query" {crate_path}/src/ | grep -v "job"

# SystemSpan usage
grep -rn "SystemSpan::new" {crate_path}/src/

# Code quality
cargo clippy -p {crate_name} -- -D warnings
cargo fmt -p {crate_name} -- --check
```

---

## Output

Generate `{crate_path}/status.md` using `/instructions/prompt/output.md` template.

**Verdict:** COMPLIANT if zero violations. NON-COMPLIANT otherwise.
