# Domain Layer Review

> Review this module as though you were Steve Klabnik implementing world-class idiomatic Rust.

---

## Input

- **Folder:** `{crate_path}`
- **Checklist:** `/instructions/rust/domain.md`
- **Standards:** `/instructions/rust/rust.md`

---

## Steps

1. Verify `module.yaml` exists and has required fields
2. Verify `src/repository/` and `src/services/` directories exist
3. Read all `.rs` files in `{crate_path}/src/`
4. Read `Cargo.toml`
5. Execute each checklist item from `/instructions/rust/domain.md`
6. For each violation, record: `file:line` + violation type
7. Generate `status.md` using output template

---

## Validation Commands

```bash
# Structure checks
test -f {crate_path}/module.yaml
test -d {crate_path}/src/repository
test -d {crate_path}/src/services

# Boundary checks
grep -E "systemprompt-(users|ai|agent|oauth|files|mcp)" {crate_path}/Cargo.toml | grep -v "^#"

# Repository pattern (no runtime SQL)
grep -rn "sqlx::query[^!]" {crate_path}/src/

# SQL in services (forbidden)
grep -rn "sqlx::" {crate_path}/src/services/

# Code quality
cargo clippy -p {crate_name} -- -D warnings
cargo fmt -p {crate_name} -- --check
```

---

## Output

Generate `{crate_path}/status.md` using `/instructions/prompt/output.md` template.

**Verdict:** COMPLIANT if zero violations. NON-COMPLIANT otherwise.
