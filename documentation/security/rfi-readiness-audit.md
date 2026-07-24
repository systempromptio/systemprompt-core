# RFI Readiness Audit

Snapshot of the codebase's RFI / enterprise-security review posture. Every item below reflects the state of `main` at the audit date; re-run the verification column to refresh.

**Audit date:** 2026-05-27
**Snapshot:** `main`, workspace version 0.12.0.

## 1. Documentation Artefacts

| Artefact | Location | Status |
|----------|----------|--------|
| Vulnerability disclosure policy | `SECURITY.md` | published |
| Public evaluation pack entry point | `documentation/README.md` | published |
| STRIDE threat model | `documentation/security/threat-model.md` | published |
| Production deployment runbook | `documentation/guides/deploy-production.md` | published |
| Compliance control matrix (HIPAA / SOC 2 / ISO 27001) | `documentation/security/compliance-control-matrix.md` | published |
| Stability contract | `documentation/security/stability-contract.md` | published |
| Compatibility matrix (providers, protocols, runtime) | `documentation/reference/compatibility.md` | published |
| Architecture (layered crates) | `README.md` + `CLAUDE.md` (repository root) | published |
| Change history | `CHANGELOG.md` | active, per-release entries |
| Licence | `LICENSE` — BSL-1.1 with four-year conversion to Apache 2.0 | published |

## 2. Supply Chain

| Check | Tool | Status | Evidence |
|-------|------|--------|----------|
| RustSec advisory scan | `cargo deny check advisories` | clean (1 documented ignore) | `deny.toml` |
| Licence compliance | `cargo deny check licenses` | clean | `deny.toml` |
| Registry source lock | `cargo deny check sources` | clean (crates.io-only) | `deny.toml` |
| Duplicate / banned crates | `cargo deny check bans` | clean | `deny.toml` |
| GitHub Dependabot | GHSA feed | 1 LOW remaining (documented) | GitHub Security tab |

`deny.toml` ignores exactly one advisory (`RUSTSEC-2023-0071`); no other RustSec advisory is suppressed.

### Documented Accepted Risk

**RUSTSEC-2023-0071 / GHSA-9c48-w39g-hm26 — Marvin Attack in the `rsa` crate (LOW).**
Pulled transitively via `jsonwebtoken` for RSA-family JWT verification. No upstream fix is available — the `rsa` crate does not yet offer a constant-time implementation.

The JWT plane is RS256-only. First-party tokens are signed RS256 (`crates/infra/security/src/jwt/mod.rs:59`, `crates/infra/security/src/keys/authority.rs:1`), and verification rejects any algorithm other than RS256 (`crates/infra/security/src/auth/validation.rs:92`, `crates/infra/security/src/auth/hook_token.rs:82`). There is no ES256/ES384/EdDSA acceptance path anywhere in the codebase, so the accepted risk does not rest on algorithm choice.

Mitigations in place:

1. The exploitable Marvin surface is RSA *private-key decryption* (PKCS#1 v1.5 timing oracle). The `rsa` code path exercised here is RS256 signature *verification* (public-key) and key *generation* — not decryption — so the platform does not exercise the decryption-oracle surface the advisory describes.
2. JWT verification is CPU-bounded and authenticated; it is not exposed as an unauthenticated high-throughput endpoint, limiting exploitability of the timing side-channel.
3. Tracked — the ignore is removed as soon as a fixed `rsa` release is available.

Full justification is recorded inline in `deny.toml` under `[advisories].ignore`.

## 3. Continuous Integration

| Workflow | File | Triggers | Status |
|----------|------|----------|--------|
| CI (fmt, clippy, build, sqlx offline check) | `.github/workflows/ci.yml` | push on main, PR | present |
| Supply Chain (cargo-deny: advisories, licenses, bans, sources) | `.github/workflows/supply-chain.yml` | push, PR, daily cron | present |
| Quality (cargo-deny + cargo-audit) | `.github/workflows/quality.yml` | push, PR, cron | present |
| Coverage (instrumented tests via RUSTFLAGS → LCOV + JSON + summary) | `.github/workflows/coverage.yml` | push on main, weekly cron, manual | present |
| Bridge release sign & publish (cosign keyless) | `.github/workflows/release-sign.yml` | `bridge-v*` tags, manual | present |

Bridge release artefacts are signed with Sigstore `cosign` (keyless) via `release-sign.yml` on `bridge-v*` tags. SBOM generation and CodeQL static analysis are **not yet authored** — no `.github/workflows/sbom.yml` or CodeQL configuration exists — and signing of the core platform release is not yet wired. See §6 (Known Gaps).

### Local Verification Performed

| Check | Command | Result |
|-------|---------|--------|
| Format | `cargo +nightly fmt --all -- --check` | clean |
| Lint (CI strict) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | zero warnings |
| Offline build | `SQLX_OFFLINE=true cargo build --workspace --locked` | clean |
| Advisory + licence + bans + sources | `cargo deny check` | clean (1 documented ignore) |
| Full test workspace | `cargo test --manifest-path crates/tests/Cargo.toml --workspace` | passing |

The `crates/tests/` workspace declares **52 workspace members** (unit, integration, contract, concurrency, property, fuzz, loadtest, and bench crates plus shared test-utility crates). Re-run the suite to capture the current passing test total before citing a headline figure.

## 4. Code and Architecture

| Property | Evidence |
|----------|----------|
| Zero `unsafe` blocks outside crypto primitives | workspace-wide search |
| 100% request-path SQL queries use compile-time verified macros (`sqlx::query!` / `query_as!` / `query_scalar!`) | `crates/infra/database/` + all repository crates |
| Typed identifiers (no raw `String` IDs) | `crates/shared/identifiers/` |
| Layer discipline enforced (shared → infra → domain → app → entry) | `crates/` layout; no circular deps in `cargo tree` |
| Memory-safe language | Rust edition 2024 |
| Single binary deployment | `crates/entry/api` + `crates/entry/cli` |
| Postgres-only persistence | `crates/infra/database/` |
| Secrets-at-rest via customer envelope encryption (KMS / Vault / sops); binary holds plaintext in process memory only | `crates/infra/config/src/bootstrap/secrets/`, `crates/shared/models/src/secrets.rs`, deployment guide §2 |
| OAuth2 with PKCE S256 (plain method rejected), JWT issuer verification, RS256-only acceptance with the active `kid` resolved against the in-process `TokenAuthority` and the JWKS sets fetched for every entry in `profile.security.trusted_issuers`. HS256 and `alg: none` are rejected. `validate_aud` is currently `false` — per-surface audience isolation is a tracked open item. | `crates/infra/security/src/auth/validation.rs`, `crates/infra/security/src/keys/authority.rs`, `crates/domain/oauth/src/repository/oauth/auth_code.rs` |
| Structured audit / log pipeline; append-only enforced by an operator-provisioned DB role grant (`INSERT, SELECT` only), not shipped in migrations; optional schema-level trigger documented | `crates/infra/logging/schema/log.sql`, `crates/infra/logging/schema/analytics.sql`, deployment guide §4.1.1 |
| Extension framework (compile-time registered via `inventory`) | `crates/shared/extension/` |

## 4b. Test Coverage and Quality Gates

The test and coverage investment is largely behind the `crates/tests/` separate workspace. Summary for reviewers:

- The `crates/tests/` workspace declares **52 workspace members** across eight categories: `unit/`, `integration/`, `contract/`, `concurrency/`, `property/` (proptest), `fuzz/` (four targets: `a2a_request`, `config_loading`, `identifier_validation`, `jsonrpc_parse`), `loadtest/`, `bench/`, plus shared test-utility crates. Re-run `cargo test --manifest-path crates/tests/Cargo.toml --workspace` to capture the current passing-test total before citing a headline figure.
- **Coverage tooling is operational.** A dedicated `crates/tests/.cargo/config.toml` overrides the root cranelift / sccache settings with the LLVM backend, which `cargo-llvm-cov` and `-Cinstrument-coverage` require. `just coverage`, `just coverage-html`, and `just coverage-clean` are the supported entry points.
- **CI integration is live.** `.github/workflows/coverage.yml` runs on a nightly cron plus manual dispatch (the instrumented build is too heavy for every push). It applies all extension schemas to a CI Postgres service, runs the full test workspace with `RUSTFLAGS='-C instrument-coverage'`, merges profdata, emits text/JSON/LCOV artefacts, uploads to Codecov, and enforces a ratchet that fails on any >0.5pt aggregate line-coverage drop. A per-run GitHub step summary makes the number visible without downloading anything.

### Per-crate coverage (dated snapshot — re-measure before citing)

> The figures below are the **2026-07-03 instrumented measurement** produced by the `Coverage` CI workflow (`.github/workflows/coverage.yml`, run against `main`) and mirrored locally by `just coverage`. `bin/bridge` (the desktop helper binary) is excluded from this denominator; it carries its own coverage workflow. Re-run the workflow to refresh before citing in a live RFI.

The headline figure is approximately **79.5% line coverage** across the production crates (128,881 lines), with **every production crate at or above 70%**. Security-critical surfaces sit highest:

| Crate | Lines | Coverage | Relevance |
|-------|------:|---------:|-----------|
| `domain/teams` | 334 | 99.7% | Teams messaging adapter |
| `shared/traits` | 790 | 99.5% | core interfaces |
| `shared/client` | 344 | 98.8% | HTTP client |
| `domain/slack` | 153 | 96.7% | Slack messaging adapter |
| `shared/provider-contracts` | 717 | 94.6% | provider trait definitions |
| `domain/analytics` | 3,464 | 94.1% | metrics / behavioural detection |
| `domain/templates` | 457 | 93.4% | template registry |
| `shared/extension` | 1,151 | 93.0% | extension framework |
| `shared/identifiers` | 1,145 | 92.5% | typed IDs (UserId, TaskId, etc.) |
| `infra/security` | 2,199 | 91.8% | JWT validation, auth extraction, manifest signing |
| `domain/marketplace` | 1,134 | 91.6% | marketplace / ABAC floor |
| `domain/content` | 2,395 | 90.7% | content management |
| `domain/users` | 1,279 | 90.2% | user management |
| `infra/logging` | 3,851 | 90.0% | structured logging |
| `infra/loader` | 822 | 86.6% | file / module discovery |
| `shared/template-provider` | 86 | 86.0% | template traits |
| `domain/files` | 1,347 | 85.7% | file storage |
| `app/scheduler` | 2,071 | 85.7% | job scheduling |
| `shared/models` | 12,159 | 85.4% | core data types |
| `domain/ai` | 5,968 | 84.7% | provider adapters |
| `infra/events` | 403 | 83.9% | audit / event pipeline |
| `domain/oauth` | 4,118 | 83.3% | OAuth2 / OIDC / PKCE |
| `infra/config` | 1,470 | 82.4% | secrets / profile bootstrap |
| `infra/database` | 3,466 | 80.6% | sqlx wrapper |
| `entry/api` | 16,891 | 80.2% | HTTP handlers |
| `infra/cloud` | 3,059 | 77.2% | cloud API / tenants |
| `domain/mcp` | 7,390 | 76.3% | MCP servers |
| `app/runtime` | 1,275 | 76.2% | AppContext wiring |
| `domain/agent` | 12,567 | 75.2% | A2A protocol (largest crate in the hot path) |
| `app/generator` | 2,145 | 71.4% | static site builder |
| `entry/cli` | 32,133 | 70.4% | CLI commands (largest crate; e2e / subprocess surface) |

Security-critical surfaces (`infra/security`, `shared/identifiers`, `infra/config`, `infra/events`) sit at 82–92%. The two largest crates — `entry/cli` (32k lines) and `domain/agent` / `entry/api` — which historically dominated the uncovered surface, now sit at 70–80% following the mid-2026 coverage campaign. CI enforces a ratchet that fails the run on any >0.5pt aggregate line-coverage regression.

## 5. Pre-answered Enterprise Security Questionnaire

Full pre-answers live in [compliance-control-matrix.md §4](compliance-control-matrix.md). Headline answers:

- **Certifications**: systemprompt.io as a vendor holds no SOC 2 / ISO 27001 / HITRUST certifications. The product is source-available infrastructure that the customer deploys inside their existing compliance boundary. Control-level support mappings are provided.
- **BAA**: Not applicable — systemprompt as a vendor does not create, receive, maintain, or transmit PHI. The binary runs in the customer's environment.
- **Data location**: Customer's own Postgres, under customer control. The vendor never sees customer data.
- **Encryption**: TLS 1.2+ in transit (enforced at entry). Secrets-at-rest via the customer's envelope-encryption infrastructure (KMS / Vault / sops) — the binary receives plaintext only after the customer's tooling opens the envelope, so the master key never enters the binary. DB-level encryption at rest is customer-managed.
- **SSO**: OIDC through the customer's IdP.
- **SBOM**: Generated on demand from the committed `Cargo.lock` (see §7). A CI-attached SBOM workflow is not yet authored.
- **Release integrity**: Bridge binaries are signed with Sigstore `cosign` (keyless) via `release-sign.yml` on `bridge-v*` tags. Signing of the core platform release is not yet wired (see §6).
- **Business continuity**: Source-available under BSL-1.1 with automatic conversion to Apache 2.0 four years after each version's publication. The customer keeps indefinite usage rights under the licence.

## 6. Known Gaps (Honest List)

These are artefacts an enterprise reviewer might ask for that are **not** yet in place. None are blocking for an RFI response; all are addressable under a commercial engagement timeline.

| Gap | Why it matters | Plan |
|-----|----------------|------|
| SBOM CI workflow (`sbom.yml`) | A CI-attached CycloneDX SBOM per release is a common procurement requirement | Author the workflow; until then the SBOM is generated on demand from the committed `Cargo.lock` |
| Core-platform release signing | `release-sign.yml` signs the **bridge** binary (cosign keyless) but the core platform release artefacts are not yet signed | Extend the signing workflow to core release artefacts before the first signed enterprise release |
| CodeQL static analysis | Automated security scanning signal | Enable GitHub default-setup CodeQL or author a workflow, then cite once it has run |
| Third-party penetration test report | Large healthcare buyers frequently require one | Commission before first enterprise deployment, or invite the customer to run their own |
| SOC 2 Type I / II attestation for systemprompt.io Ltd | Useful but not required for the self-hosted model | Revisit when customer count and team size justify the audit cost |
| Cyber liability + E&O insurance certificate | Typical procurement checkbox | Quote and bind before contract signature |
| Formal incident-response playbook (beyond SECURITY.md) | Full IR runbook for customer-facing incidents | Draft alongside the first paid customer |
| Public CI badges on README | Visible signal of a maintained project | Add once workflows have produced a stable history |

## 7. Verification

To reproduce the supply-chain and build checks from a fresh clone:

```bash
git clone https://github.com/systempromptio/systemprompt-core
cd systemprompt-core
cargo +nightly fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
SQLX_OFFLINE=true cargo build --workspace --locked
cargo deny check
```

A CycloneDX SBOM can be generated on demand from the committed `Cargo.lock` with `cargo cyclonedx`; this is a manual step and is not produced by CI until an SBOM workflow is authored.

All commands should complete without errors on a clean checkout.

## 8. Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial audit following an enterprise RFI inbound. 37 Dependabot advisories resolved to 1 LOW documented ignore; public evaluation pack shipped. |
| 2026-05-22 | Fidelity pass against `main` (0.11.1). Corrected the RUSTSEC-2023-0071 rationale to the real RS256-only mitigations (no ES/EdDSA path exists; the exploitable surface is RSA decryption, not the signature-verification path exercised here). Corrected the test-workspace figure to 52 workspace members and flagged the per-crate coverage table as a dated snapshot to re-measure. Repointed the secrets-bootstrap citation to `crates/infra/config/src/bootstrap/secrets/`. Noted `validate_aud=false` as a tracked open item and that the audit-table grant is operator-provisioned. |
| 2026-05-22 | `release-sign.yml` now exists and signs the bridge binary (Sigstore `cosign` keyless, `bridge-v*` tags) — recorded it in the CI table and reframed the gap as core-platform release signing. SBOM (`sbom.yml`) and CodeQL remain not authored. |
| 2026-05-27 | Re-pinned snapshot to 0.12.0. Authz surface refactored: `JwtClaims.department` / `AuthzRequest.department` replaced by an `attributes: BTreeMap<String, serde_json::Value>` bag (token issuers namespace their own keys); `AuthzContext` enum replaced with `{ kind, payload }`; `RuleType::Department` removed and migration `008_drop_department_acl.sql` narrows `access_control_rules.rule_type` to `('role','user')`; core RBAC resolver promoted to a first-class `RuleBasedHook` so every decision flows through the `AuthzDecisionHook` pipeline. All pre-0.12 JWTs are incompatible — rotate signing keys or wait out existing token lifetimes before upgrading. |
