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
| §164.312(a)(1) Access control | Emergency access procedure | Operational; deployment guide describes break-glass role provisioning | [../guides/deploy-production.md §6](../guides/deploy-production.md) |
| §164.312(a)(1) Access control | Automatic logoff | Session / token TTL enforced; configurable per IdP | `crates/domain/oauth/` token expiry |
| §164.312(a)(2) Encryption and decryption | Encryption of ePHI at rest and in transit | TLS 1.2+ enforced at entry. Prompt and response content not persisted by default. For secrets-at-rest (provider API keys, JWT signing key): the binary loads secrets from a profile-referenced file or environment; the expected deployment pattern is that the customer uses their existing envelope-encryption infrastructure (HashiCorp Vault, AWS/GCP/Azure KMS, sops + age) to protect the secrets file — the master key never enters the binary. DB-level encryption at rest is customer-managed (RDS/AKS storage encryption, dm-crypt, etc.) | `crates/infra/config/src/bootstrap/secrets/`, `crates/shared/models/src/secrets.rs`, [../guides/deploy-production.md §2](../guides/deploy-production.md) |
| §164.312(b) Audit controls | Record and examine activity | Every governed request produces a structured log or analytics event with identity, endpoint, outcome, timestamp | `crates/infra/logging/schema/log.sql`, `crates/infra/logging/schema/analytics.sql` |
| §164.312(c) Integrity | ePHI not altered or destroyed improperly | Append-only discipline is an operator-provisioned control: the systemprompt DB role is granted `INSERT, SELECT` (not `UPDATE, DELETE`) on the audit/log tables. **The grant itself is not shipped in the schema migrations** — the operator applies it per the deployment guide. No schema-level immutability triggers are shipped; recommended hardening DDL (a BEFORE UPDATE/DELETE trigger) is published in the deployment guide for customers whose programme requires defense-in-depth | [../guides/deploy-production.md §4](../guides/deploy-production.md), [threat-model.md §4.2](threat-model.md) |
| §164.312(d) Person or entity authentication | Verify identity of user | OAuth2/OIDC with PKCE mandated server-side (`S256` only; `plain` rejected); JWT signature and issuer validation; rejects `alg: none` and any algorithm other than RS256; a `kid` is mandatory and unknown keys fail closed. **Audience is always validated** — the policy's audience list is applied unconditionally and a policy declaring no audiences is rejected as an error, so a permissive "any audience" configuration cannot be expressed. Per-surface isolation is enforced through typed audience values and per-MCP-server audience checks | `crates/infra/security/src/jwt/validate.rs:65-88`, `crates/infra/security/src/auth/hook_token.rs:79`, `crates/domain/mcp/src/middleware/rbac.rs:153`, `crates/domain/oauth/` |
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
| CC6.1 Logical access controls over protected information | OAuth2/OIDC at entry; a compile-enforced authorization gate (`with_auth(policy)` — omitting the policy is a build error) that fails closed for unauthenticated requests; a deny-overrides rule resolver; and per-`user_id` filtering in repository queries. **Row-level tenant isolation is not provided** — the database scoping layer is an opt-in seam with no registered provider and no RLS policies shipped, so deployments needing hard tenant separation must register a scope provider and author policies, or run one instance per tenant | `crates/infra/security/src/authz/`, `crates/infra/database/src/scope/mod.rs:11-16`, `crates/domain/users/`, tests in `crates/tests/` |
| CC6.2 Registration and authorisation | Managed by the customer IdP; systemprompt consumes claims | N/A (customer-owned) |
| CC6.3 Access removed on termination | Customer IdP revocation propagates on next token refresh | Token TTL configurable |
| CC6.6 Protects against unauthorised external access | TLS only; audited ingress; no inbound management channel to the binary | `crates/entry/api/` |
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
| CC8.1 Authorises, designs, develops, tests, approves, implements, and documents changes | All development lands on `next`. The release line, `main`, is protected by a ruleset that requires a pull request and grants **no bypass to anyone** — a direct push is refused for maintainers and repository admins alike. Promotion is a deliberate two-step: `just gate` dispatches CI, Quality and Supply Chain against a pinned SHA and waits for all three; `just promote` then freezes that exact commit on a `promote` ref and opens the PR onto `main`, so nothing merged in the interim can ride along ungated. Every push to `next` runs the full gate set: fmt, build, sqlx offline verification, 13 sharded test groups, clippy, rustdoc, 16 source-gate linters, an MSRV check, and `cargo deny`. Release tags are verified against CHANGELOG entries by `just check-release-tag`. | `.github/workflows/{ci,quality,supply-chain}.yml`, `justfile` (`gate`, `promote`, `check-release-tag`), CHANGELOG.md, [stability-contract.md](stability-contract.md) |

### CC9 — Risk Mitigation

| Criterion | Systemprompt support | Evidence |
|-----------|----------------------|----------|
| CC9.1 Identifies, selects, and develops risk mitigation activities | Threat model; continuous dependency scanning with `cargo deny` (advisories, licences, banned crates, registry sources) across all seven workspaces, blocking on pull requests and on a daily schedule. Advisories assessed and accepted are recorded with written justification in `deny.toml` and disclosed in §5 below rather than reported as a clean scan | [threat-model.md](threat-model.md), `.github/workflows/supply-chain.yml`, `deny.toml` |
| CC9.2 Vendor and business partner risk management | Customer's responsibility. A CycloneDX SBOM is generated on demand via `cargo cyclonedx`; automated per-release SBOM publication is **planned, not yet wired** (no `sbom.yml` workflow exists) | `deny.toml`; SBOM generation is currently a manual step |

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
| A.8.12 Data leakage prevention | Detect and prevent | Secrets tagged and redacted in logs; prompt/response persistence off by default |
| A.8.15 Logging | Produce, protect, analyse logs | Structured JSON audit stream, append-only via operator-provisioned DB role least-privilege (optional schema-level trigger published for defense-in-depth), SIEM integration |
| A.8.16 Monitoring activities | Monitor for anomalies | Prometheus metrics, documented alert thresholds |
| A.8.23 Web filtering | Control outbound content | Per-provider `base_url` config supports an egress proxy |
| A.8.24 Use of cryptography | Policy + controls | TLS 1.2+ required at entry. JWT verification via `jsonwebtoken::Validation::new(Algorithm::RS256)`, with any non-RS256 algorithm rejected (`crates/infra/security/src/jwt/validate.rs:70`); the active `kid` is resolved against the in-process `TokenAuthority` cache and the public set published at `/.well-known/jwks.json`. HS256 and `alg: none` are rejected; multi-issuer trust is configured via `profile.security.trusted_issuers`. PKCE `S256` enforced for the OAuth2 code flow (plain rejected, constant-time compare). MCP manifest signatures via Ed25519. OAuth refresh-token ids and authorisation codes are stored as HMAC-SHA-256 digests under the deployment `oauth_at_rest_pepper` (`crates/shared/models/src/secrets.rs:29`). Other secrets-at-rest are expected via customer envelope encryption (Vault / KMS / sops) — the binary does not perform its own symmetric at-rest encryption |
| A.8.25 Secure development lifecycle | Apply secure SDLC | Compile-time SQL verification, fmt/clippy/tests in CI, threat model maintained |
| A.8.26 Application security requirements | Identify and apply | This document + threat model |
| A.8.28 Secure coding | Apply principles | Rust memory safety with `unsafe_code = "deny"` set workspace-wide and no `#[allow(unsafe_code)]` in any production crate. The workspace lint profile additionally denies `unwrap_used` and `expect_used`. Five `unsafe` blocks exist in the published crates, all confined to a single platform-syscall module for subprocess management (`crates/shared/models/src/subprocess/{linux,darwin}.rs`); the desktop bridge carries further FFI blocks for Win32 and macOS process APIs. There is no `unsafe` in any cryptographic path. Coding standards are enforced by 16 CI linters |
| A.8.31 Separation of environments | Dev / test / prod | Profile-based config allows per-environment overrides |
| A.8.32 Change management | Controlled changes | CI + CHANGELOG + stability contract |

## 4. Standard Security Questionnaire Answers

Pre-answers to the questions an enterprise security questionnaire (CAIQ, SIG, SIG Lite, VSAQ) asks most often.

| Question | Answer |
|----------|--------|
| Are you SOC 2 certified? | No, and by design rather than by omission. A SOC 2 report describes the **vendor's** operating environment — the systems, people, and processes that handle customer data. In this deployment model we handle none: the binary runs inside your boundary, against your database, and no customer data reaches us. A SOC 2 on systemprompt.io Ltd would therefore attest to controls that are not in the path of your risk. What is in that path is the software itself, and §2 maps it criterion by criterion so it can be brought inside **your** SOC 2 scope. |
| Are you ISO 27001 certified? | Not at this time. See §3 above for control mappings. |
| Are you HITRUST certified? | Not at this time. HITRUST inherits HIPAA + ISO mappings from §1 and §3. |
| Do you sign BAAs? | A BAA is not applicable to this deployment model. See "Framing" above. |
| Where is customer data stored? | In the customer's Postgres instance, under the customer's control. systemprompt.io as a vendor does not receive or store customer data. |
| Do you encrypt data at rest? | The binary itself does not perform symmetric at-rest encryption of secrets; the deployment model expects the customer to use their existing envelope-encryption infrastructure (Vault / AWS KMS / GCP KMS / Azure Key Vault / sops) to protect the secrets file on disk. This keeps master-key management inside the customer's HSM/KMS rather than in a vendor-supplied binary. Customer data in Postgres is encrypted via customer-configured storage encryption (RDS / Cloud SQL / dm-crypt / TDE). Deployment guide §2 documents the supported patterns. |
| Do you encrypt data in transit? | TLS 1.2+ required at entry; all outbound provider calls over HTTPS. |
| What authentication methods do you support? | OAuth2 / OIDC with PKCE, plus WebAuthn. Customer-supplied IdP. |
| Do you support SSO? | Yes — OIDC-based SSO through the customer's IdP. |
| Do you support audit logging? | Yes. Every governed request produces a structured audit event with full decision trace. |
| How do you handle vulnerabilities? | SECURITY.md defines reporting, SLAs, and coordinated disclosure. `cargo deny` runs on every push, every pull request, and daily, and blocks merges. Advisories we have assessed and accepted are published with justifications in §5.1 rather than suppressed silently. |
| Do you run penetration tests? | We have not commissioned a third-party penetration test to date, and we would rather say so than imply otherwise. Because the binary runs wholly inside your compliance boundary and we never receive your data, the assessment that carries the most weight is one **you** run against **your** deployment — the source is available for review and we support customer-commissioned testing under commercial agreement, including coordination and remediation. A commissioned external assessment is planned; scope and timing can be discussed as part of a commercial engagement, and co-funding with a customer whose programme requires it is welcomed. |
| Do you publish an SBOM? | Not currently attached to releases. A CycloneDX SBOM can be generated on demand from the committed `Cargo.lock` with `cargo cyclonedx`, and we will produce one for you on request. Automated per-release publication is tracked in [rfi-readiness-audit.md §6](rfi-readiness-audit.md). |
| Are releases signed? | `systemprompt-bridge` binaries are signed with Sigstore `cosign` (keyless, OIDC-bound to this repository and workflow) via `.github/workflows/release-sign.yml` on `bridge-v*` tags, with a Rekor transparency-log entry and a published `cosign verify-blob` command. The core platform ships as source and as crates.io packages rather than as binaries we distribute; organisations that repackage it for internal distribution sign the resulting artefact packs under their own key and provenance, which keeps the signing authority with the party that controls the deployment. |
| What is your business continuity plan? | Source-available under BUSL-1.1 with conversion to Apache 2.0 four years after each version's publication. The customer retains indefinite usage rights under licence and can continue operating without vendor involvement. See [stability-contract.md](stability-contract.md). |
| Do you have cyber liability insurance? | Commercial insurance particulars available under NDA with qualified prospects. |

## 5. Evidence Catalog

| Evidence type | Location |
|---------------|----------|
| Source code | This repository (`crates/`) |
| Architecture narrative | `crates/`-level READMEs; repository root `README.md` |
| Security policy and disclosure | `SECURITY.md` |
| Threat model | [threat-model.md](threat-model.md) |
| Deployment and operations | [../guides/deploy-production.md](../guides/deploy-production.md) |
| Stability and compatibility | [stability-contract.md](stability-contract.md), [../reference/compatibility.md](../reference/compatibility.md) |
| Change history | `CHANGELOG.md` |
| Supply-chain continuous verification | `.github/workflows/supply-chain.yml`, `deny.toml` |
| Continuous integration and gating | `.github/workflows/{ci,quality,coverage,coverage-bridge,exercise-suites,release-sign}.yml` |
| Licence | `LICENSE` (BUSL-1.1 → Apache 2.0 four-year conversion) |

Bridge release artefacts are signed with `cosign` keyless (`.github/workflows/release-sign.yml`, `bridge-v*` tags). The core platform ships as source and as crates.io packages and is not signed by us; organisations that repackage it for internal distribution sign the resulting artefact packs under their own key and provenance. Per-release SBOM publication (CycloneDX attachment) is not yet wired; no `sbom.yml` workflow is committed.

### 5.1 Accepted-risk register

We publish the register rather than reporting a clean scan. As of 0.43.0, `deny.toml`
records 14 suppressed advisories, each with a written justification. Two are reachable from
the published crates:

| Advisory | Crate | Assessment |
|----------|-------|------------|
| RUSTSEC-2023-0071 | `rsa` (via `jsonwebtoken`) | Marvin timing attack. No upstream fix exists. The exploitable surface is RSA **private-key decryption**; the JWT plane performs RS256 signature **verification** only, and there is no ES/EdDSA path. Verification is authenticated and CPU-bounded rather than exposed as an unauthenticated high-throughput endpoint. Tracked for removal when a fixed `rsa` ships |
| RUSTSEC-2026-0173 | `proc-macro-error2` (via `tabled`, `validator`) | Unmaintained. Build-time proc-macro only; never linked into the runtime binary, so it carries no runtime security surface |

The remaining twelve are reachable only from the desktop bridge's and test workspaces'
Linux windowing stack (`winit`/`wry`/GTK3), which the bridge does not compile — its
tray and webview stack is declared only for macOS and Windows, but Cargo resolves the Linux
branch into the lockfile regardless of target. No shipped artefact contains that code, and
no upstream release moves `winit` off it.

The register is the complete set: no advisory is suppressed without an entry here.

## 6. Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial public publication. |
| 2026-08-28 | Fidelity pass against `next` @ 0.41.0 after ~29 minors of drift. **Published the accepted-risk register (§5.1)** — the pack had previously stated that exactly one advisory was suppressed when the real figure was 14, which is the kind of understatement an auditor is entitled to treat as material. **Restored the audience-validation claim under §164.312(d)**, removed in May on the belief it was unenforced; it is mandatory, and an empty audience policy is now a hard error. **Corrected CC6.1 to state plainly that row-level tenant isolation is not provided** — the prior "tenant scoping where applicable" wording implied more than the code delivers. Rewrote CC8.1 around the protected-`main` / `just gate` / `just promote` flow, which is the strongest change-management evidence in the repository and had gone uncited. Corrected A.8.28, which claimed no `unsafe` outside crypto primitives — there is none in crypto, and five blocks exist in a platform-syscall module. Repointed dead citations (`auth/validation.rs:92` → `jwt/validate.rs:70`; `secrets.rs:17` → `:29`). Corrected the `cargo audit` references to the `cargo deny` that actually runs, and the licence spelling to BUSL-1.1. Recorded that core-platform artefacts are signed by the receiving organisation, not by us. |
| 2026-05-22 | Removed evidence citations for the non-existent `sbom.yml` workflow (SBOM remains generated on demand). Repointed the audit-table schema citation to `crates/infra/logging/schema/{log,analytics}.sql`. Restated the A.8.24 cryptography control as RS256-only with the real at-rest mitigations; removed the audience-validation claim. Marked the §164.312(c) integrity grant as operator-provisioned. |
| 2026-05-22 | Recorded that `release-sign.yml` exists and signs the bridge binary (cosign keyless, `bridge-v*` tags); reframed release-signing answers as bridge-signed with core-platform signing still planned. |
