# 07 — Coverage Measurement & CI Integration

**Grade: F**

No coverage measurement exists. The Cranelift codegen backend blocks `cargo-llvm-cov`, and no alternative is configured. There is no way to know what percentage of code is tested, whether coverage is improving or declining, or which modules have zero coverage.

**Unlocks:** All coverage-target reports (09-18) benefit from measurable baselines. Without measurement, improvement is guesswork.

---

## Current State

- `cargo-llvm-cov` 0.6.18 and `cargo-tarpaulin` 0.32.8 are both installed
- Neither can run because `.cargo/config.toml` sets `codegen-backend = "cranelift"` for the dev profile
- LLVM coverage instrumentation requires the LLVM backend — Cranelift does not support `-Cinstrument-coverage`
- No CI pipeline runs tests or coverage automatically
- No coverage badges, no per-PR delta reports, no baseline metric

### Why Cranelift Is Configured

Cranelift provides faster debug compilation (~30-40% faster incremental builds). This is a valid development ergonomic. The conflict is that coverage tools require LLVM instrumentation passes that Cranelift does not implement.

## Desired State

- Coverage reports generated on every PR via CI
- Per-crate line coverage visible in HTML reports
- Coverage delta shown on PRs ("+2.3% in systemprompt-agent")
- Baseline established for all 30 production crates
- No coverage minimum enforced initially — visibility first, thresholds later
- Development builds still use Cranelift for speed

## How to Get There

### Option A: Profile Override (Recommended)

Create a dedicated coverage profile that forces the LLVM backend:

```toml
# .cargo/config.toml — add this section
[profile.coverage]
inherits = "dev"
codegen-backend = "llvm"
```

Then run coverage with:

```bash
CARGO_PROFILE=coverage cargo llvm-cov \
  --manifest-path crates/tests/Cargo.toml \
  --workspace --html
```

If `cargo-llvm-cov` does not support profile selection natively, use an environment variable override:

```bash
RUSTFLAGS="-C instrument-coverage" cargo test \
  --manifest-path crates/tests/Cargo.toml \
  --workspace
```

### Option B: CI-Only Override

In CI, override the codegen backend via environment:

```yaml
# .github/workflows/coverage.yml
- name: Run coverage
  env:
    CARGO_PROFILE_DEV_CODEGEN_BACKEND: llvm
  run: |
    cargo llvm-cov --manifest-path crates/tests/Cargo.toml \
      --workspace --lcov --output-path lcov.info
```

This keeps Cranelift for local development and uses LLVM only in CI.

### Option C: Use Tarpaulin

`cargo-tarpaulin` uses ptrace-based instrumentation instead of compiler instrumentation, so it may work with Cranelift:

```bash
cargo tarpaulin --manifest-path crates/tests/Cargo.toml \
  --workspace --out html
```

Test this first — tarpaulin is less precise than llvm-cov but avoids the backend conflict entirely.

## Incremental Improvement Strategy

### Week 1
- Test all three options locally (A, B, C) and determine which works
- Generate first-ever coverage report
- Document the per-crate baseline numbers

### Week 2
- Add coverage job to CI (GitHub Actions or equivalent)
- Generate HTML report as CI artifact
- Add coverage badge to README

### Week 3
- Add per-PR coverage delta comments (via `cargo-llvm-cov --fail-under` or custom script)
- Establish baseline: record current per-crate coverage in a tracking document

### Month 2+
- After cleanup phases (reports 02-05) complete, set initial thresholds:
  - Security crates: 70% minimum
  - Domain crates: 40% minimum
  - Entry crates: 30% minimum
- Ratchet thresholds up quarterly based on progress

## Tracking

| Metric | Current | Week 1 Target | Month 1 Target |
|--------|---------|---------------|----------------|
| Coverage reports available | No | Yes (local) | Yes (CI) |
| Per-crate baselines documented | No | Yes | Yes |
| PR delta visible | No | No | Yes |
| Coverage thresholds enforced | No | No | No (month 2+) |
