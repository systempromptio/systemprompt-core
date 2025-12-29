## Instructions

Review this module as though you were Steve Klabnik implmenting world class idiomatic rust. Complete ALL steps in order:

### Step 1: Boundary Plan Verification

1. Find the boundary plan at `/var/www/html/systemprompt-core/plan/bd-{crate_name}.md`
2. Read the boundary plan and identify all listed violations
3. Verify each violation has been fixed:
   - Check `Cargo.toml` for removed dependencies
   - Grep for forbidden imports (e.g., `use systemprompt_core_{module}`)
   - Confirm traits are used instead of concrete types
4. If violations remain, fix them before proceeding

### Step 2: Build Verification

Run these commands and fix any errors:

```bash
cargo clippy -p {package_name} -- -D warnings
cargo build -p {package_name}
cargo test -p {package_name} --no-run
```

### Step 3: Checklist Compliance

Apply `/var/www/html/systemprompt-core/instructions/rust-checklist.md` in full:

- **Section 1-5:** Forbidden constructs, limits, patterns, naming, logging
- **Section 6-9:** Architecture (redundancy, file structure, domain, boundaries)
- **Section 10-11:** Antipatterns, architecture simplicity
- **Section 12-15:** Taxonomy, boundary violations, dependency direction, circular deps

For each failing check, fix the code before documenting.

### Step 4: Update status.md

Create or update `status.md` in the module root with:

- Complete results table (all 116 checks)
- Pass/Fail status with file:line evidence for failures
- Summary counts by category
- Verdict: APPROVED or REJECTED
- Timestamp of review

### Step 5: Update README.md

Update `README.md` with a factual map of the crate:

- List all directories with brief purpose
- List key files with brief explanation
- No marketing language, just facts
- Keep concise (no lengthy descriptions)

---

## Completion Criteria

The module review is complete when:

1. ✅ All boundary plan violations are fixed
2. ✅ `cargo clippy -p {package} -- -D warnings` passes
3. ✅ `cargo build -p {package}` succeeds
4. ✅ All 116 checklist rules evaluated
5. ✅ `status.md` created with full results
6. ✅ `README.md` updated with crate map
7. ✅ Verdict is APPROVED (or REJECTED with documented actions)
