# Coverage Baseline Report

> Generated: 2026-04-02 | Tooling: cargo-llvm-cov + llvm-cov report | Test workspace: crates/tests/

## Overall

| Metric | Value |
|--------|-------|
| Total production lines | 30,067 |
| Covered lines | 7,807 |
| **Line coverage** | **25.97%** |
| Regions covered | 25.40% |
| Functions covered | 25.14% |

## Per-Crate Coverage

### Domain Layer

| Crate | Lines | Covered | Coverage |
|-------|-------|---------|----------|
| domain/templates | 434 | 326 | **75.1%** |
| domain/oauth | 3,120 | 1,064 | 34.1% |
| domain/analytics | 3,122 | 769 | 24.6% |
| domain/users | 967 | 167 | 17.3% |

### Infrastructure Layer

| Crate | Lines | Covered | Coverage |
|-------|-------|---------|----------|
| infra/events | 121 | 121 | **100.0%** |
| infra/security | 530 | 513 | **96.8%** |
| infra/config | 510 | 442 | **86.7%** |
| infra/loader | 782 | 559 | 71.5% |
| infra/cloud | 2,049 | 470 | 22.9% |
| infra/logging | 3,730 | 341 | 9.1% |
| infra/database | 1,893 | 132 | 7.0% |

### Shared Layer

| Crate | Lines | Covered | Coverage |
|-------|-------|---------|----------|
| shared/client | 262 | 194 | 74.0% |
| shared/template-provider | 86 | 52 | 60.5% |
| shared/identifiers | 923 | 517 | 56.0% |
| shared/traits | 691 | 351 | 50.8% |
| shared/provider-contracts | 680 | 303 | 44.6% |
| shared/extension | 1,007 | 431 | 42.8% |
| shared/models | 9,160 | 1,055 | 11.5% |

### Not Yet Measured

These crates have no unit tests in the test workspace or are not instrumented:
- domain/agent, domain/ai, domain/content, domain/files, domain/mcp
- app/runtime, app/scheduler, app/generator, app/sync
- entry/api, entry/cli

## How to Run

```bash
just coverage        # Text summary
just coverage-html   # HTML report at coverage-report/html/index.html
just coverage-clean  # Remove coverage artifacts
```

## Architecture

The coverage system works around two constraints in `.cargo/config.toml`:

1. **Cranelift backend** (`codegen-backend = "cranelift"`) — used for fast dev builds but incompatible with LLVM's `-Cinstrument-coverage`
2. **sccache wrapper** (`RUSTC_WRAPPER = "sccache"`) — caches uninstrumented binaries, preventing profraw generation

Solution: `crates/tests/.cargo/config.toml` overrides both settings for the test workspace. The `just coverage` recipe runs from within `crates/tests/` so Cargo resolves the local config, keeping the root config untouched for normal development.
