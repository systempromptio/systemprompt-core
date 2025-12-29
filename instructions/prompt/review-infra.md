# Infrastructure Layer Review

> Review this module as though you were Steve Klabnik implementing world-class idiomatic Rust.

---

## Input

- **Folder:** `{crate_path}`
- **Checklist:** `/instructions/rust/infra.md`
- **Standards:** `/instructions/rust/rust.md`

---

## Steps

1. Read all `.rs` files in `{crate_path}/src/`
2. Read `Cargo.toml`
3. Execute each checklist item from `/instructions/rust/infra.md`
4. For each violation, record: `file:line` + violation type
5. Generate `status.md` using output template

---

## Validation Commands

```bash
# Boundary checks
grep -E "systemprompt-(users|ai|agent|oauth|files|mcp)" {crate_path}/Cargo.toml
grep -E "systemprompt-(api|tui|scheduler)" {crate_path}/Cargo.toml

# Code quality
cargo clippy -p {crate_name} -- -D warnings
cargo fmt -p {crate_name} -- --check
```

---

## Output

Generate `{crate_path}/status.md` using `/instructions/prompt/output.md` template.

**Verdict:** COMPLIANT if zero violations. NON-COMPLIANT otherwise.
