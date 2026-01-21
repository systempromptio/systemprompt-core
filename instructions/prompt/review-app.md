# Application Layer Review

>We are publishing this crate onto crates.io for the first time. Do a final ultimate review of this this module as though you were Steve Klabnik implementing world-class idiomatic Rust.

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

## Output

Generate `{crate_path}/status.md` using `/instructions/prompt/output.md` template.
Update the readme with the exact file structure, module explanation and overview
Identify all violations

**Verdict:** COMPLIANT if zero violations. NON-COMPLIANT otherwise.
