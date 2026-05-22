# Compliance Control Matrix

This document maps controls from the HIPAA Security Rule, SOC 2 Trust Services Criteria, and ISO/IEC 27001 Annex A to architectural features and code paths in systemprompt.io.

## Framing: who owns what

systemprompt.io is **source-available infrastructure**, not a managed service. The binary runs inside the customer's environment, processes data that never leaves the customer's network, and persists to a database the customer owns. Consequences:

1. **The customer's compliance programme is the boundary of record.** Their SOC 2, HIPAA, and ISO 27001 audits cover the operating environment. systemprompt is a component that supports those programmes.
2. **systemprompt is not a HIPAA Business Associate.** Because the vendor (systemprompt.io) does not create, receive, maintain, or transmit PHI on behalf of the customer, no Business Associate Agreement is required under 45 CFR §160.103. The binary runs in the customer's compliance boundary; the customer remains the Covered Entity or Business Associate for the data flowing through it. A commercial licence agreement governs software use; a BAA is neither required nor meaningful for this deployment model.
3. **The marketing site's claim "architecture supports SOC 2 / HIPAA / ISO 27001"** means systemprompt provides the controls, evidence, and configurability needed for a customer to include it within a successful audit of those standards — not that systemprompt itself holds certifications.
4. **What systemprompt attests to directly:** the architectural features, code paths, and operational documentation in this repository. Everything below is verifiable by reading the code.

## 1. HIPAA Security Rule — 45 CFR §164.308, §164.310, §164.312

### §164.312 Technical Safeguards (the relevant part for software)

| Standard | Requirement | Systemprompt support | Evidence |
|----------|-------------|----------------------|----------|
| §164.312(a)(1) Access control | Unique user identification | Every request is authenticated; identity propagated as a typed user ID through every layer | `crates/shared/identifiers/src/lib.rs` (typed `UserId`), `crates/infra/security/` (JWT verification) |
| §164.312(a)(1) Access control | Emergency access procedure | Operational; deployment guide describes break-glass role provisioning | [../deployment-reference-architecture.md §6](../deployment-reference-architecture.md) |
| §164.312(a)(1) Access control | Automatic logoff | Session / token TTL enforced; configurable per IdP | `crates/domain/oauth/` token expiry |
| §164.312(a)(2) Encryption and decryption | Encryption of ePHI at rest and in transit | TLS 1.2+ enforced at entry. Prompt and response content not persisted by default. For secrets-at-rest (provider API keys, JWT signing key): the binary loads secrets from a profile-referenced file or environment; the expected deployment pattern is that the customer uses their existing envelope-encryption infrastructure (HashiCorp Vault, AWS/GCP/Azure KMS, sops + age) to protect the secrets file — the master key never enters the binary. DB-level encryption at rest is customer-managed (RDS/AKS storage encryption, dm-crypt, etc.) | `crates/infra/config/src/bootstrap/secrets/`, `crates/shared/models/src/secrets.rs`, [../deployment-reference-architecture.md §2](../deployment-reference-architecture.md) |
| §164.312(b) Audit controls | Record and examine activity | Every governed request produces a structured log or analytics event with identity, endpoint, outcome, timestamp | `crates/infra/logging/schema/log.sql`, `crates/infra/logging/schema/analytics.sql` |
| §164.312(c) Integrity | ePHI not altered or destroyed improperly | Append-only discipline is an operator-provisioned control: the systemprompt DB role is granted `INSERT, SELECT` (not `UPDATE, DELETE`) on the audit/log tables. **The grant itself is not shipped in the schema migrations** — the operator applies it per the deployment guide. No schema-level immutability triggers are shipped; recommended hardening DDL (a BEFORE UPDATE/DELETE trigger) is published in the deployment guide for customers whose programme requires defense-in-depth | [../deployment-reference-architecture.md §4](../deployment-reference-architecture.md), [threat-model.md §4.2](threat-model.md) |
| §164.312(d) Person or entity authentication | Verify identity of user | OAuth2/OIDC with PKCE; JWT signature and issuer validation; rejects `alg: none` and any algorithm other than RS256 | `crates/infra/security/`, `crates/domain/oauth/` |
| §164.312(e)(1) Transmission security | Integrity + encryption in transit | TLS at entry; outbound provider requests over HTTPS; no plaintext listener | `crates/entry/api/` |

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
| CC6.1 Logical access controls over protected information | OAuth2/OIDC at entry; handler-boundary RBAC; tenant scoping at the repository layer where applicable | `crates/infra/security/`, `crates/domain/users/`, tests in `crates/tests/` |
| CC6.2 Registration and authorisation | Managed by the customer IdP; systemprompt consumes claims | N/A (customer-owned) |
| CC6.3 Access removed on termination | Customer IdP revocation propagates on next token refresh | Token TTL configurable |
| CC6.6 Protects against unauthorised external access | TLS only; audited ingress; no inbound management channel to the binary | `crates/entry/api/` |
| CC6.7 Transmission of information | TLS 1.2+; customer-supplied trust store for outbound | Reverse-proxy config + provider adapter HTTPS |
| CC6.8 Prevents unauthorised or malicious software | Single binary, no dynamic code loading; extensions are compile-time registered via `inventory` | `crates/shared/extension/src/lib.rs` |

### CC7 — System Operations

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC7.1 Detection of anomalies | Structured metrics + audit event stream to the customer SIEM | [../deployment-reference-architecture.md §7](../deployment-reference-architecture.md) |
| CC7.2 Monitors system capacity | Prometheus metrics; recommended alerts documented | deployment guide §7.1 |
| CC7.3 Evaluates security events | Customer SIEM responsibility; systemprompt provides the feed | — |
| CC7.4 Incident response | SECURITY.md disclosure + customer incident response process | SECURITY.md |
| CC7.5 Recovery from incidents | Backup + DR runbook | deployment guide §4–5 |

### CC8 — Change Management

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC8.1 Authorises, designs, develops, tests, approves, implements, and documents changes | Version-controlled repo; CI enforces fmt, clippy, tests; CHANGELOG maintained per release; stability contract defines compatibility guarantees | `.github/workflows/ci.yml`, CHANGELOG.md, [stability-contract.md](stability-contract.md) |

### CC9 — Risk Mitigation

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC9.1 Identifies, selects, and develops risk mitigation activities | Threat model; continuous dependency audit (cargo-audit, cargo-deny) blocking PRs | [threat-model.md](threat-model.md), `.github/workflows/supply-chain.yml`, `deny.toml` |
| CC9.2 Vendor and business partner risk management | Customer's responsibility. A CycloneDX SBOM is generated on demand via `cargo cyclonedx`; automated per-release SBOM publication is **planned, not yet wired** (no `sbom.yml` workflow exists) | `deny.toml`; SBOM generation is currently a manual step |

## 3. ISO/IEC 27001:2022 — Annex A (selected)

| Control | Description | Systemprompt support |
|---------|-------------|----------------------|
| A.5.7 Threat intelligence | Monitor advisory feeds | `cargo audit` on schedule (RustSec DB); patch SLA in SECURITY.md |
| A.5.23 Information security for cloud services | Policy for use of cloud | Self-hosted deployment model means the customer retains control |
| A.8.2 Privileged access rights | Restrict and manage | Handler-boundary RBAC; DB role least-privilege (operator-provisioned) |
| A.8.3 Information access restriction | Access per policy | Tenant scoping in repository queries where applicable |
| A.8.5 Secure authentication | MFA, strong auth | OAuth2/OIDC with PKCE; MFA is IdP-side |
| A.8.8 Management of technical vulnerabilities | Patch management | SECURITY.md triage + fix SLAs |
| A.8.9 Configuration management | Manage securely | Profile-based config, version-controlled, signed manifests for the MCP allowlist |
| A.8.12 Data leakage prevention | Detect and prevent | Secrets tagged and redacted in logs; prompt/response persistence off by default |
| A.8.15 Logging | Produce, protect, analyse logs | Structured JSON audit stream, append-only via operator-provisioned DB role least-privilege (optional schema-level trigger published for defense-in-depth), SIEM integration |
| A.8.16 Monitoring activities | Monitor for anomalies | Prometheus metrics, documented alert thresholds |
| A.8.23 Web filtering | Control outbound content | Per-provider `base_url` config supports an egress proxy |
| A.8.24 Use of cryptography | Policy + controls | TLS 1.2+ required at entry. JWT verification via `jsonwebtoken::Validation::new(Algorithm::RS256)`, with any non-RS256 algorithm rejected (`crates/infra/security/src/auth/validation.rs:92`); the active `kid` is resolved against the in-process `TokenAuthority` cache and the public set published at `/.well-known/jwks.json`. HS256 and `alg: none` are rejected; multi-issuer trust is configured via `profile.security.trusted_issuers`. PKCE `S256` enforced for the OAuth2 code flow (plain rejected, constant-time compare). MCP manifest signatures via Ed25519. OAuth refresh-token ids and authorisation codes are stored as HMAC-SHA-256 digests under the deployment `oauth_at_rest_pepper` (`crates/shared/models/src/secrets.rs:17`). Other secrets-at-rest are expected via customer envelope encryption (Vault / KMS / sops) — the binary does not perform its own symmetric at-rest encryption |
| A.8.25 Secure development lifecycle | Apply secure SDLC | Compile-time SQL verification, fmt/clippy/tests in CI, threat model maintained |
| A.8.26 Application security requirements | Identify and apply | This document + threat model |
| A.8.28 Secure coding | Apply principles | Rust memory safety; no unsafe blocks outside crypto primitives; coding standards enforced |
| A.8.31 Separation of environments | Dev / test / prod | Profile-based config allows per-environment overrides |
| A.8.32 Change management | Controlled changes | CI + CHANGELOG + stability contract |

## 4. Standard Security Questionnaire Answers

Pre-answers to the questions an enterprise security questionnaire (CAIQ, SIG, SIG Lite, VSAQ) asks most often.

| Question | Answer |
|----------|--------|
| Are you SOC 2 certified? | Not at this time. The architecture is designed so the customer's SOC 2 programme covers the deployment. See §2 above. |
| Are you ISO 27001 certified? | Not at this time. See §3 above for control mappings. |
| Are you HITRUST certified? | Not at this time. HITRUST inherits HIPAA + ISO mappings from §1 and §3. |
| Do you sign BAAs? | A BAA is not applicable to this deployment model. See "Framing" above. |
| Where is customer data stored? | In the customer's Postgres instance, under the customer's control. systemprompt.io as a vendor does not receive or store customer data. |
| Do you encrypt data at rest? | The binary itself does not perform symmetric at-rest encryption of secrets; the deployment model expects the customer to use their existing envelope-encryption infrastructure (Vault / AWS KMS / GCP KMS / Azure Key Vault / sops) to protect the secrets file on disk. This keeps master-key management inside the customer's HSM/KMS rather than in a vendor-supplied binary. Customer data in Postgres is encrypted via customer-configured storage encryption (RDS / Cloud SQL / dm-crypt / TDE). Deployment guide §2 documents the supported patterns. |
| Do you encrypt data in transit? | TLS 1.2+ required at entry; all outbound provider calls over HTTPS. |
| What authentication methods do you support? | OAuth2 / OIDC with PKCE, plus WebAuthn. Customer-supplied IdP. |
| Do you support SSO? | Yes — OIDC-based SSO through the customer's IdP. |
| Do you support audit logging? | Yes. Every governed request produces a structured audit event with full decision trace. |
| How do you handle vulnerabilities? | SECURITY.md defines reporting, SLAs, and coordinated disclosure. Continuous dependency audit (cargo-deny, cargo-audit) runs in CI and blocks merges. |
| Do you run penetration tests? | Customer-commissioned penetration testing is supported under commercial agreement. |
| Do you publish an SBOM? | A CycloneDX SBOM is generated on demand via `cargo cyclonedx`. Automated per-release SBOM publication is planned, not yet wired. |
| Are releases signed? | Signed release artefacts (Sigstore cosign keyless) are planned, not yet wired; no signing workflow is committed today. |
| What is your business continuity plan? | Source-available under BSL-1.1 with conversion to Apache 2.0 four years after each version's publication. The customer retains indefinite usage rights under licence and can continue operating without vendor involvement. See [stability-contract.md](stability-contract.md). |
| Do you have cyber liability insurance? | Commercial insurance particulars available under NDA with qualified prospects. |

## 5. Evidence Catalog

| Evidence type | Location |
|---------------|----------|
| Source code | This repository (`crates/`) |
| Architecture narrative | `crates/`-level READMEs; repository root `README.md` |
| Security policy and disclosure | `SECURITY.md` |
| Threat model | [threat-model.md](threat-model.md) |
| Deployment and operations | [../deployment-reference-architecture.md](../deployment-reference-architecture.md) |
| Stability and compatibility | [stability-contract.md](stability-contract.md), [../reference/compatibility.md](../reference/compatibility.md) |
| Change history | `CHANGELOG.md` |
| Supply-chain continuous verification | `.github/workflows/supply-chain.yml`, `deny.toml` |
| Licence | `LICENSE` (BSL-1.1 → Apache 2.0 four-year conversion) |

Release-signing and per-release SBOM publication (`cosign` keyless signing, CycloneDX attachment) are planned but not yet wired; no `release-sign.yml` or `sbom.yml` workflow is committed.

## 6. Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
| 2026-05-22 | Removed evidence citations for non-existent `sbom.yml` and `release-sign.yml` workflows (marked SBOM/signing as planned, not yet wired). Repointed the audit-table schema citation to `crates/infra/logging/schema/{log,analytics}.sql`. Restated the A.8.24 cryptography control as RS256-only with the real at-rest mitigations; removed the audience-validation claim. Marked the §164.312(c) integrity grant as operator-provisioned. |
