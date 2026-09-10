# Compliance Control Matrix

This document maps controls from the HIPAA Security Rule, SOC 2 Trust Services Criteria, and ISO/IEC 27001 Annex A to architectural features and code paths in systemprompt.io.

## Deployment scope

These mappings identify software features relevant to selected controls. They do not
establish compliance, certification or the applicability of an agreement to a deployment.
The operator defines the assessment boundary, configures infrastructure and selects
upstream providers. Requests to external providers and integrations can leave that boundary.

Certification, third-party assessment and contractual status require current vendor
evidence. The repository provides implementation references and operational guidance.

## 1. HIPAA Security Rule — 45 CFR §164.308, §164.310, §164.312

### §164.312 Technical Safeguards (the relevant part for software)

| Standard | Requirement | Systemprompt support | Evidence |
|----------|-------------|----------------------|----------|
| §164.312(a)(1) Access control | Unique user identification | Protected routes validate caller identity and propagate a typed user ID; explicitly public routes allow anonymous access | `crates/shared/identifiers/src/lib.rs` (typed `UserId`), `crates/infra/security/` (JWT verification) |
| §164.312(a)(1) Access control | Emergency access procedure | Operational; deployment guide describes break-glass role provisioning | [../guides/deploy-production.md §6](../guides/deploy-production.md) |
| §164.312(a)(1) Access control | Automatic logoff | Session / token TTL enforced; configurable per IdP | `crates/domain/oauth/` token expiry |
| §164.312(a)(2) Encryption and decryption | Encryption of ePHI at rest and in transit | TLS termination is operator-managed. Gateway audit storage can contain prompt and response content. For secrets-at-rest (provider API keys, JWT signing key): the binary loads secrets from a profile-referenced file or environment; the expected deployment pattern is that the customer uses their existing envelope-encryption infrastructure (HashiCorp Vault, AWS/GCP/Azure KMS, sops + age) to protect the secrets file — the master key never enters the binary. DB-level encryption at rest is customer-managed (RDS/AKS storage encryption, dm-crypt, etc.) | `crates/infra/config/src/bootstrap/secrets/`, `crates/shared/models/src/secrets.rs`, [../guides/deploy-production.md §2](../guides/deploy-production.md) |
| §164.312(b) Audit controls | Record and examine activity | Every governed request produces a structured log or analytics event with identity, endpoint, outcome, timestamp | `crates/infra/logging/schema/log.sql`, `crates/infra/logging/schema/analytics.sql` |
| §164.312(c) Integrity | ePHI not altered or destroyed improperly | Append-only discipline is an operator-provisioned control: the systemprompt DB role is granted `INSERT, SELECT` (not `UPDATE, DELETE`) on the audit/log tables. **The grant itself is not shipped in the schema migrations** — the operator applies it per the deployment guide. No schema-level immutability triggers are shipped; recommended hardening DDL (a BEFORE UPDATE/DELETE trigger) is published in the deployment guide for customers whose programme requires defense-in-depth | [../guides/deploy-production.md §4](../guides/deploy-production.md), [threat-model.md §4.2](threat-model.md) |
| §164.312(d) Person or entity authentication | Verify identity of user | OAuth2/OIDC with PKCE mandated server-side (`S256` only; `plain` rejected); JWT signature and issuer validation; rejects `alg: none` and any algorithm other than RS256; a `kid` is mandatory and unknown keys fail closed. **Audience is always validated** — the policy's audience list is applied unconditionally and a policy declaring no audiences is rejected as an error, so a permissive "any audience" configuration cannot be expressed. Per-surface isolation is enforced through typed audience values and per-MCP-server audience checks | `crates/infra/security/src/jwt/validate.rs:65-88`, `crates/infra/security/src/auth/hook_token.rs:79`, `crates/domain/mcp/src/middleware/rbac.rs:153`, `crates/domain/oauth/` |
| §164.312(e)(1) Transmission security | Integrity + encryption in transit | Operator-configured TLS at ingress and HTTPS provider endpoints; the API itself binds an HTTP listener | `crates/entry/api/` |

### §164.308 Administrative Safeguards (customer-owned, supported by systemprompt)

| Standard | Customer responsibility | Systemprompt support |
|----------|-------------------------|----------------------|
| §164.308(a)(1) Security management | Risk analysis, risk management | Threat model, deployment guide, and compatibility matrix inform the customer's analysis |
| §164.308(a)(3) Workforce security | Authorisation and clearance | RBAC enforced at handler boundary; scopes drawn from IdP claims |
| §164.308(a)(5) Security awareness | Training | Not applicable to the binary |
| §164.308(a)(6) Security incident procedures | Incident response | SECURITY.md defines coordinated disclosure; the audit event stream supports customer forensics |
| §164.308(a)(7) Contingency plan | Backup, DR, emergency mode | Deployment guide §4 (backup), §5 (DR), §9 (rollback) |

### §164.310 Physical Safeguards

Entirely customer-owned. Physical security of the host infrastructure is outside systemprompt's trust boundary.

## 2. SOC 2 Trust Services Criteria

Common Criteria mappings. Mirrors the 2017 TSC revision (effective through current audit cycles).

### CC6 — Logical and Physical Access Controls

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC6.1 Logical access controls over protected information | OAuth2/OIDC at entry; a compile-enforced authorization gate (`with_auth(policy)` — omitting the policy is a build error) that fails closed for unauthenticated requests; a deny-overrides rule resolver; and per-`user_id` filtering in repository queries. **Row-level tenant isolation is not provided** — the database scoping layer is an opt-in seam with no registered provider and no RLS policies shipped, so deployments needing hard tenant separation must register a scope provider and author policies, or run one instance per tenant | `crates/infra/security/src/authz/`, `crates/infra/database/src/scope/mod.rs:11-16`, `crates/domain/users/`, tests in `crates/tests/` |
| CC6.2 Registration and authorisation | Managed by the customer IdP; systemprompt consumes claims | N/A (customer-owned) |
| CC6.3 Access removed on termination | Customer IdP revocation propagates on next token refresh | Token TTL configurable |
| CC6.6 Protects against unauthorised external access | Operator-configured TLS and ingress restrictions; authenticated administrative API routes | `crates/entry/api/` |
| CC6.7 Transmission of information | TLS 1.2+; customer-supplied trust store for outbound | Reverse-proxy config + provider adapter HTTPS |
| CC6.8 Prevents unauthorised or malicious software | Single binary, no dynamic code loading; extensions are compile-time registered via `inventory` | `crates/shared/extension/src/lib.rs` |

### CC7 — System Operations

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC7.1 Detection of anomalies | Structured metrics + audit event stream to the customer SIEM | [../guides/deploy-production.md §7](../guides/deploy-production.md) |
| CC7.2 Monitors system capacity | Prometheus metrics; recommended alerts documented | deployment guide §7.1 |
| CC7.3 Evaluates security events | Customer SIEM responsibility; systemprompt provides the feed | — |
| CC7.4 Incident response | SECURITY.md disclosure + customer incident response process | SECURITY.md |
| CC7.5 Recovery from incidents | Backup + DR runbook | deployment guide §4–5 |

### CC8 — Change Management

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC8.1 Authorises, designs, develops, tests, approves, implements, and documents changes | All development lands on `next`. The release line, `main`, is protected by a ruleset that requires a pull request and grants **no bypass to anyone** — a direct push is refused for maintainers and repository admins alike. Promotion is a deliberate two-step: `just gate` dispatches CI, Quality and Supply Chain against a pinned SHA and waits for all three; `just promote` then freezes that exact commit on a `promote` ref and opens the PR onto `main`, so nothing merged in the interim can ride along ungated. Every push to `next` runs the full gate set: fmt, build, sqlx offline verification, sharded test groups, clippy, rustdoc, source-gate linters, an MSRV check, and `cargo deny`. Release tags are verified against CHANGELOG entries by `just check-release-tag`. | `.github/workflows/{ci,quality,supply-chain}.yml`, `justfile` (`gate`, `promote`, `check-release-tag`), CHANGELOG.md, [stability-contract.md](stability-contract.md) |

### CC9 — Risk Mitigation

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC9.1 Identifies, selects, and develops risk mitigation activities | Threat model; continuous dependency scanning with `cargo deny` (advisories, licences, banned crates, registry sources) across all seven workspaces, blocking on pull requests and on a daily schedule. Advisories assessed and accepted are recorded with written justification in `deny.toml` and disclosed in §5 below | [threat-model.md](threat-model.md), `.github/workflows/supply-chain.yml`, `deny.toml` |
| CC9.2 Vendor and business partner risk management | Customer's responsibility. A CycloneDX SBOM is generated on demand via `cargo cyclonedx`; automated per-release SBOM publication is **not configured** (no `sbom.yml` workflow exists) | `deny.toml`; SBOM generation is currently a manual step |

## 3. ISO/IEC 27001:2022 — Annex A (selected)

| Control | Description | Systemprompt support |
|---------|-------------|----------------------|
| A.5.7 Threat intelligence | Monitor advisory feeds | `cargo deny` against the RustSec advisory DB on every push, every PR, and a daily schedule; patch SLA in SECURITY.md |
| A.5.23 Information security for cloud services | Policy for use of cloud | Self-hosted deployment model means the customer retains control |
| A.8.2 Privileged access rights | Restrict and manage | Handler-boundary RBAC; DB role least-privilege (operator-provisioned) |
| A.8.3 Information access restriction | Access per policy | Per-`user_id` filtering in repository queries plus the deny-overrides authz resolver. Not row-level security — see CC6.1 |
| A.8.5 Secure authentication | MFA, strong auth | OAuth2/OIDC with PKCE; MFA is IdP-side |
| A.8.8 Management of technical vulnerabilities | Patch management | SECURITY.md triage + fix SLAs |
| A.8.9 Configuration management | Manage securely | Profile-based config, version-controlled, signed manifests for the MCP allowlist |
| A.8.12 Data leakage prevention | Detect and prevent | Secret scanning and log redaction on supported paths; gateway request payloads require access and retention controls |
| A.8.15 Logging | Produce, protect, analyse logs | Structured JSON audit stream; governance-decision UPDATE trigger; operator-managed deletion grants, privileged access and external retention; SIEM integration |
| A.8.16 Monitoring activities | Monitor for anomalies | Prometheus metrics, documented alert thresholds |
| A.8.23 Web filtering | Control outbound content | Per-provider `base_url` config supports an egress proxy |
| A.8.24 Use of cryptography | Policy + controls | Configure TLS 1.2+ at the deployment ingress. JWT verification via `jsonwebtoken::Validation::new(Algorithm::RS256)`, with any non-RS256 algorithm rejected (`crates/infra/security/src/jwt/validate.rs:70`); the active `kid` is resolved against the in-process `TokenAuthority` cache and the public set published at `/.well-known/jwks.json`. HS256 and `alg: none` are rejected; multi-issuer trust is configured via `profile.security.trusted_issuers`. PKCE `S256` enforced for the OAuth2 code flow (plain rejected, constant-time compare). MCP manifest signatures via Ed25519. OAuth refresh-token ids and authorisation codes are stored as HMAC-SHA-256 digests under the deployment `oauth_at_rest_pepper` (`crates/shared/models/src/secrets.rs:29`). Other secrets-at-rest are expected via customer envelope encryption (Vault / KMS / sops) — the binary does not perform its own symmetric at-rest encryption |
| A.8.25 Secure development lifecycle | Apply secure SDLC | Compile-time SQL verification, fmt/clippy/tests in CI, threat model maintained |
| A.8.26 Application security requirements | Identify and apply | This document + threat model |
| A.8.28 Secure coding | Apply principles | Workspace lints deny unrestricted unsafe code, unwrap and expect usage. Platform FFI uses scoped exceptions with safety obligations. Review the enabled target and the source-contract checks configured in CI |
| A.8.31 Separation of environments | Dev / test / prod | Profile-based config allows per-environment overrides |
| A.8.32 Change management | Controlled changes | CI + CHANGELOG + stability contract |

## 4. Standard Security Questionnaire Answers

Pre-answers to the questions an enterprise security questionnaire (CAIQ, SIG, SIG Lite, VSAQ) asks most often.

| Question | Answer |
|----------|--------|
| Are you SOC 2 certified? | No SOC 2 report is included in this repository. Obtain current vendor assessment evidence; §2 maps relevant software controls. |
| Are you ISO 27001 certified? | No ISO 27001 certificate is included in this repository. Confirm current status with the vendor; §3 maps selected controls. |
| Are you HITRUST certified? | No HITRUST certificate is included in this repository. Confirm current status and assessment scope with the vendor. |
| Do you sign BAAs? | Confirm applicable agreement requirements and available terms for the deployment with the vendor. |
| Where is customer data stored? | In configured PostgreSQL and file storage. Provider calls and enabled integrations transmit data to their configured destinations. |
| Do you encrypt data at rest? | The binary itself does not perform symmetric at-rest encryption of secrets; the deployment model expects the customer to use their existing envelope-encryption infrastructure (Vault / AWS KMS / GCP KMS / Azure Key Vault / sops) to protect the secrets file on disk. This keeps master-key management inside the customer's HSM/KMS rather than in a vendor-supplied binary. Customer data in Postgres is encrypted via customer-configured storage encryption (RDS / Cloud SQL / dm-crypt / TDE). Deployment guide §2 documents the supported patterns. |
| Do you encrypt data in transit? | The API binds an HTTP listener. Configure TLS at the reverse proxy and HTTPS for external provider endpoints. |
| What authentication methods do you support? | OAuth2 / OIDC with PKCE, plus WebAuthn. Customer-supplied IdP. |
| Do you support SSO? | Yes — OIDC-based SSO through the customer's IdP. |
| Do you support audit logging? | Yes. Every governed request produces a structured audit event with full decision trace. |
| How do you handle vulnerabilities? | SECURITY.md defines reporting, SLAs, and coordinated disclosure. `cargo deny` runs on every push, every pull request, and daily, with merge requirements determined by repository settings. Advisories we have assessed and accepted are published with justifications in §5.1. |
| Do you run penetration tests? | No commissioned third-party report is included in this repository. Obtain current assessment scope, date and results from the vendor. Test the deployed configuration and its integration boundaries. |
| Do you publish an SBOM? | Not currently attached to releases. A CycloneDX SBOM can be generated on demand from the committed `Cargo.lock` with `cargo cyclonedx`, and we will produce one for you on request. Automated per-release publication is tracked in [rfi-readiness-audit.md §6](rfi-readiness-audit.md). |
| Are releases signed? | `systemprompt-bridge` binaries are signed with Sigstore `cosign` (keyless, OIDC-bound to this repository and workflow) via `.github/workflows/release-sign.yml` on `bridge-v*` tags, with a Rekor transparency-log entry and a published `cosign verify-blob` command. The core platform ships as source and as crates.io packages rather than as binaries we distribute; organisations that repackage it for internal distribution sign the resulting artefact packs under their own key and provenance, under their deployment policy. |
| What is your business continuity plan? | Source is distributed under the repository licence. Deployment continuity requires operator-managed backups, restore tests, dependency availability and applicable usage rights. |
| Do you have cyber liability insurance? | Request current insurance documentation from the vendor. |

## 5. Evidence Catalog

| Evidence type | Location |
|---------------|----------|
| Source code | This repository (`crates/`) |
| Architecture reference | `crates/`-level READMEs; repository root `README.md` |
| Security policy and disclosure | `SECURITY.md` |
| Threat model | [threat-model.md](threat-model.md) |
| Deployment and operations | [../guides/deploy-production.md](../guides/deploy-production.md) |
| Stability and compatibility | [stability-contract.md](stability-contract.md), [../reference/compatibility.md](../reference/compatibility.md) |
| Change history | `CHANGELOG.md` |
| Supply-chain continuous verification | `.github/workflows/supply-chain.yml`, `deny.toml` |
| Continuous integration and gating | `.github/workflows/{ci,quality,coverage,coverage-bridge,exercise-suites,release-sign}.yml` |
| Licence | `LICENSE` (BUSL-1.1 → Apache 2.0 four-year conversion) |

Bridge release artefacts are signed with `cosign` keyless (`.github/workflows/release-sign.yml`, `bridge-v*` tags). The core platform ships as source and as crates.io packages and is not signed by us; organisations that repackage it for internal distribution sign the resulting artefact packs under their own key and provenance. Per-release SBOM publication (CycloneDX attachment) is not configured; no `sbom.yml` workflow is committed.

### 5.1 Advisory exceptions

[deny.toml](../../deny.toml) is the authoritative register of advisory exceptions and
acceptance rationales. [Security review reference §2](rfi-readiness-audit.md#2-supply-chain)
lists their scope. Review target- and feature-specific dependency reachability when
assessing a release. A passing scan with exceptions is not a vulnerability-free assertion.
