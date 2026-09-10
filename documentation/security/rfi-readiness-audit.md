# Security review reference

Technical evidence for reviewing a self-hosted systemprompt deployment. Implementation
references describe the source in this checkout. Workflow definitions describe configured
checks; they do not establish that a particular commit passed those checks.

## 0. Assessment scope

The repository provides first-party technical documentation. It does not contain a SOC 2
report, ISO 27001 or HITRUST certificate, or commissioned penetration-test report. Confirm
current external assessment status and scope with the vendor during procurement.

The operator controls deployment infrastructure, database access, network policy and
upstream integrations. Include these components in the deployment's security assessment.
The [control matrix](compliance-control-matrix.md) maps software capabilities to selected
control requirements; it is not a certification or an assessment result.

## 1. Documentation artefacts

| Artefact | Reference |
|----------|-----------|
| Vulnerability reporting | [Security policy](../../SECURITY.md) |
| Architecture and boundaries | [Architecture](../concepts/architecture.md), [threat model](threat-model.md) |
| Deployment controls | [Production deployment](../guides/deploy-production.md) |
| Control mappings | [Compliance control matrix](compliance-control-matrix.md) |
| Compatibility and version policy | [Compatibility](../reference/compatibility.md), [stability contract](stability-contract.md) |
| Release changes | [Changelog](../../CHANGELOG.md) |
| Software licence | [LICENSE](../../LICENSE) |

## 2. Supply chain

[Supply Chain](../../.github/workflows/supply-chain.yml) runs `just deny` on pushes and
pull requests targeting `main` or `next`, on a daily schedule, and by manual dispatch.
The recipe checks the repository workspaces using the shared [deny.toml](../../deny.toml).
Checks cover advisories, licences, dependency bans and registry sources. Yanked packages
are configured as warnings.

### 2.1 Advisory exceptions

`deny.toml` is the authoritative exception register. It records each advisory identifier
and its acceptance rationale. An ignored advisory remains an exception to the scan; a
successful scan does not establish that dependencies contain no vulnerabilities.

The register includes `RUSTSEC-2023-0071` for `rsa`, `RUSTSEC-2026-0173` for
`proc-macro-error2`, and exceptions for dependencies in the bridge's Linux windowing
resolution graph: `RUSTSEC-2026-0194`, `RUSTSEC-2026-0195`, `RUSTSEC-2026-0192`,
`RUSTSEC-2024-0370`, `RUSTSEC-2024-0412`, `RUSTSEC-2024-0413`, `RUSTSEC-2024-0415`,
`RUSTSEC-2024-0416`, `RUSTSEC-2024-0418`, `RUSTSEC-2024-0419`, `RUSTSEC-2024-0420`
and `RUSTSEC-2024-0429`.

Review dependency reachability for the target platform and feature set using its lockfile
and Cargo dependency graph. Build-time dependencies also require supply-chain review.

### 2.2 Scan coverage

The configured workflow runs `cargo deny` and a separate `cargo audit` job through
`just audit`. Assess advisory coverage against the scanner versions, lockfiles and policies.
The repository does not configure automatic SBOM publication. Generate an SBOM from the
lockfile and build configuration used for the deployed artifact when required.

## 3. Continuous integration

| Workflow | Configured checks |
|----------|-------------------|
| [CI](../../.github/workflows/ci.yml) | Formatting, locked build, SQLx cache verification and sharded tests with PostgreSQL |
| [Quality](../../.github/workflows/quality.yml) | Clippy, Rustdoc, source-contract checks, MSRV and platform-specific bridge checks |
| [Supply Chain](../../.github/workflows/supply-chain.yml) | Shared dependency policy across workspaces |
| [Coverage](../../.github/workflows/coverage.yml) | Instrumented tests, coverage artifacts, configured floor and comparison with a baseline |
| [Bridge coverage](../../.github/workflows/coverage-bridge.yml) | Platform-specific bridge coverage |
| [Exercise suites](../../.github/workflows/exercise-suites.yml) | Fuzz smoke tests and benchmark/load-test compilation |
| [Bridge release signing](../../.github/workflows/release-sign.yml) | Release builds, checksums and Sigstore signing |

Use the run associated with the evaluated commit for pass/fail results. Coverage percentages
require a successful measurement for that commit and its declared instrumentation scope.

### 3.1 Change management

Development targets `next`; `main` is the release branch. `just gate [REF]` dispatches
release checks. `just promote [SHA]` prepares the selected commit on `promote` and opens a
release pull request. Check the repository's current GitHub rulesets separately when
assessing branch protection; hosted settings are not established by source files.

### 3.2 Local verification

```bash
just format-check
SQLX_OFFLINE=true cargo build --workspace --locked
just lint-comments
just deny
```

Tests run through `just test-shard <group>` or `just test-all-shards` and require the
configured PostgreSQL test database. `scripts/test-shard.sh --list` lists available groups.
`just doc-check` builds documentation for the root, bridge and test workspaces; test
workspace SQLx macros require its database configuration.

## 4. Implementation controls

| Control | Implementation evidence | Scope |
|---------|-------------------------|-------|
| JWT validation | `crates/infra/security/src/jwt/validate.rs` | First-party RS256 validation requires `kid`, a nonempty audience policy and time-claim validation; issuer checks depend on the selected policy |
| OAuth authorization code protection | `crates/domain/oauth/` | PKCE S256, exact redirect URI validation, expiring single-use codes and refresh-token revocation |
| Authorization | `crates/infra/security/src/authz/` | Route policies and resource decisions; authenticated identity alone does not grant access |
| Database access | `crates/infra/database/`, domain repositories | PostgreSQL, parameterized queries and compile-time query checks; bootstrap DDL has separate rules |
| Extension registration | `crates/shared/extension/` | Compile/link-time registration through `inventory`; extensions execute as trusted in-process code |
| Audit persistence | `crates/infra/logging/`, `crates/domain/ai/` | Structured decisions and request records; database permissions and retention are deployment controls |
| Transport protection | `crates/entry/api/src/services/server/startup.rs` | The API binds an HTTP TCP listener; production TLS termination and ingress restrictions are operator-managed |
| Shutdown | `crates/entry/api/src/services/server/shutdown.rs` | Signal-driven connection draining, child cleanup and forced-exit deadlines |

## 5. Procurement information

- **Data storage:** configured PostgreSQL and file storage. Provider requests and enabled
  integrations can transmit data to configured external services.
- **Encryption:** configure TLS at ingress and protect database volumes, backups and secrets
  with deployment-managed encryption and access controls.
- **Identity:** OAuth2/OIDC and WebAuthn; review issuer, session and revocation configuration.
- **Audit content:** request records can include prompts, responses, tool definitions and
  wire payloads. Establish access and retention rules for the enabled paths.
- **Release integrity:** the bridge signing workflow publishes checksums and Sigstore
  verification material. Verify the artifact and workflow identity used for deployment.
- **Commercial evidence:** request current certification, assessment, insurance and agreement
  documents from the vendor. Repository content does not establish contractual coverage.

## 6. Limitations and deployment requirements

| Area | Current boundary | Deployment requirement |
|------|------------------|------------------------|
| Tenant isolation | Scoped transactions consult extension-registered providers; without one, they use an ordinary transaction. Core migrations do not supply tenant RLS policies | Implement and test scope providers, scoped query coverage and RLS policies, or use separate deployments |
| Outbound URL validation | Parse-time URL validation checks schemes and literal addresses; the shared guarded client additionally validates resolved addresses and redirects | Restrict egress and review each client's redirect and destination policy |
| Gateway quotas | Ordinary gateway cost buckets account for completed requests; concurrent in-flight requests can exceed a ceiling. Evaluation reservations use a separate admission path | Size ordinary quotas for concurrent usage; review evaluation reservation and reconciliation rules separately |
| Audit integrity | The governance-decision UPDATE trigger rejects row updates while enabled; DELETE remains grant-controlled, and owners can disable the trigger | Apply table-specific least privilege and external retention; preserve required updates to mutable request records |
| PostgreSQL compatibility | Deployment documentation specifies PostgreSQL 18+; CI service definitions use PostgreSQL 16 | Validate the deployment's PostgreSQL version before rollout |
| Supply-chain automation | Workflow files use mutable Action tags; no dependency-update, CodeQL or SBOM workflow is committed | Assess build provenance and dependency controls for the deployment |
| External assurance | Third-party assessment artifacts are not included in this repository | Obtain current evidence with an explicit scope and date |

## 7. Verification

Evaluate the exact source revision, dependency lockfiles, feature set, target platform and
configuration being deployed. Record the commands and results used for an assessment.
Source inspection, scanner output, hosted repository settings and third-party assessments
are separate evidence sources and should be identified as such.
