# CAIQ v4 Response

A domain-by-domain response to the Cloud Security Alliance Consensus Assessments Initiative
Questionnaire v4 (CCM v4 domains), for a **self-hosted** systemprompt deployment. Many CAIQ
questions concern a provider-operated cloud service. For a self-hosted deployment those controls
sit with the customer and are marked *Customer*. A completed question-level spreadsheet is
available on request.

| Domain | Response | Evidence |
|--------|----------|----------|
| **A&A** Audit & Assurance | No vendor-operated service to audit. Source, CI definitions and gate results are public and independently verifiable. No SOC 2 report or ISO certificate is held | [README](../README.md) |
| **AIS** Application & Interface Security | Documented SDLC with automated gates; threat model; compile-enforced authorisation; compile-checked SQL; CodeQL; fuzzing | [Secure development policy](../secure-development-policy.md), [Threat model](../threat-model.md) |
| **BCR** Business Continuity & Resilience | No runtime vendor dependency; public reproducible source; licence conversion. Deployment backup and DR: *Customer*, procedures documented | [Business continuity](../business-continuity.md), [Production deployment §4–5](../../guides/deploy-production.md) |
| **CCC** Change Control & Configuration | Protected `main`, required checks, frozen-SHA promotion, changelog, semver report. Deployment configuration via version-controllable profiles | [Secure development policy § 1](../secure-development-policy.md#1-branches-and-change-flow) |
| **CEK** Cryptography, Encryption & Key Mgmt | RS256 JWTs with mandatory `kid`; TLS and at-rest encryption: *Customer*; secrets via customer KMS or Vault; release signing via Sigstore | [Data handling § 5](../data-handling.md#5-encryption), [Supply chain](../supply-chain.md) |
| **DCS** Datacenter Security | *Customer* (N/A to vendor: no hosting) | [Shared responsibility](../shared-responsibility.md) |
| **DSP** Data Security & Privacy | Vendor has no access to customer data and receives no telemetry. Retention jobs and per-user purge are provided | [Data handling](../data-handling.md) |
| **GRC** Governance, Risk & Compliance | Published security policy, SDLC, vulnerability and incident procedures; reviewed with each minor release | This pack |
| **HRS** Human Resources | Single-maintainer project; no staff with access to customer environments. Compensating controls are automated | [Secure development policy § 6](../secure-development-policy.md#6-review-and-segregation-of-duties) |
| **IAM** Identity & Access Mgmt | Product: OAuth2/OIDC with PKCE, WebAuthn, RBAC, customer IdP. Vendor: repository write access restricted to the maintainer; `main` has no bypass | [Compliance control matrix § CC6](../compliance-control-matrix.md) |
| **IPY** Interoperability & Portability | Open protocols (OAuth2/OIDC, MCP, A2A, OpenTelemetry); data in the customer's PostgreSQL | [Stability contract](../stability-contract.md) |
| **IVS** Infrastructure & Virtualization | *Customer* | [Production deployment](../../guides/deploy-production.md) |
| **LOG** Logging & Monitoring | Structured audit stream, Prometheus metrics, OTLP export; SIEM and retention: *Customer* | [Production deployment §7](../../guides/deploy-production.md) |
| **SEF** Security Incident Mgmt | Vendor-side incident process and notification commitments | [Incident response](../incident-response.md) |
| **STA** Supply Chain Mgmt, Transparency & Accountability | Dependency policy, SBOM, provenance, signed releases, shared-responsibility matrix | [Supply chain](../supply-chain.md), [Shared responsibility](../shared-responsibility.md) |
| **TVM** Threat & Vulnerability Mgmt | Daily advisory, secret and static scanning; CVSS triage; published SLAs; GitHub Security Advisories | [Vulnerability management](../vulnerability-management.md) |
| **UEM** Universal Endpoint Mgmt | *Customer*. The desktop bridge's managed-policy and update controls are documented | [Security policy § Telemetry](../../../SECURITY.md#telemetry) |
