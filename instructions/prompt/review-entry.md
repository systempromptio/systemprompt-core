# Entry Layer Review

> Review this module as though you were Steve Klabnik implementing world-class idiomatic Rust.

---

## Input

- **Folder:** `{crate_path}`
- **Checklist:** `/instructions/rust/entry.md`
- **Standards:** `/instructions/rust/rust.md`

---

## Steps

1. Verify `main.rs` or public lib exists
2. Read all `.rs` files in `{crate_path}/src/`
3. Read `Cargo.toml`
4. Execute each checklist item from `/instructions/rust/entry.md`
5. Verify handlers follow extract → delegate → respond pattern
6. For each violation, record: `file:line` + violation type
7. Generate `status.md` using output template

---

## Validation Commands

```bash
# Structure checks
test -f {crate_path}/src/main.rs || test -f {crate_path}/src/lib.rs

# No SQL in handlers (API)
grep -rn "sqlx::" {crate_path}/src/routes/

# No repository access in handlers
grep -rn "repository\." {crate_path}/src/routes/

# State extractor used
grep -rn "State<AppServices>" {crate_path}/src/

# Code quality
cargo clippy -p {crate_name} -- -D warnings
cargo fmt -p {crate_name} -- --check
```

---

## Output

Generate `{crate_path}/status.md` using `/instructions/prompt/output.md` template.

**Verdict:** COMPLIANT if zero violations. NON-COMPLIANT otherwise.
