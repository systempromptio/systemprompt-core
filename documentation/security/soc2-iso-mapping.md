# Supplier Control Mapping

How systemprompt meets the controls a customer applies to its **software suppliers**. The
[Compliance control matrix](compliance-control-matrix.md) maps the software's runtime features
to controls a deployment needs. This document covers the vendor's own practices.

## ISO/IEC 27001:2022 Annex A

| Control | How systemprompt meets it | Evidence |
|---------|---------------------------|----------|
| A.5.19 Information security in supplier relationships | Supplier role defined: software only, no access to customer data or environment | [README](README.md), [Shared responsibility](shared-responsibility.md) |
| A.5.20 Addressing security within supplier agreements | Security policy, SLAs and notification commitments are published and can be incorporated into the agreement | [Security policy](../../SECURITY.md), [Incident response](incident-response.md) |
| A.5.21 Managing security in the ICT supply chain | Dependency policy, signed releases, SBOM, provenance | [Supply chain](supply-chain.md) |
| A.5.22 Monitoring and review of supplier services | Public CI results, Scorecard, release attestations; this pack is reviewed with each minor release | GitHub Actions, [Scorecard](https://scorecard.dev/viewer/?uri=github.com/systempromptio/systemprompt-core) |
| A.5.23 Information security for use of cloud services | Not applicable to self-hosted use; no vendor cloud service is involved | [Data handling](data-handling.md) |
| A.5.24–5.28 Incident management | Vendor-side incident process and customer notification | [Incident response](incident-response.md) |
| A.5.29–5.30 Continuity | No runtime vendor dependency; public source; licence conversion | [Business continuity](business-continuity.md) |
| A.5.7 Threat intelligence | Daily RustSec advisory scans | [Vulnerability management](vulnerability-management.md) |
| A.8.4 Access to source code | `main` changeable only through gated PRs; write access restricted to the maintainer | [Secure development policy § 1](secure-development-policy.md#1-branches-and-change-flow) |
| A.8.8 Management of technical vulnerabilities | Scanning, CVSS triage, published remediation SLAs | [Vulnerability management](vulnerability-management.md) |
| A.8.25 Secure development life cycle | Documented and automated SDLC | [Secure development policy](secure-development-policy.md) |
| A.8.26 Application security requirements | Threat model maintained with each new surface | [Threat model](threat-model.md) |
| A.8.27 Secure system architecture | Layered architecture enforced by lint; fail-closed authorisation | [Architecture](../concepts/architecture.md) |
| A.8.28 Secure coding | Rust, `unsafe` denied, compile-checked SQL, source-gate linters | [Secure development policy § 3](secure-development-policy.md#3-secure-coding-standards) |
| A.8.29 Security testing in development and acceptance | Test shards, schema ladder, CodeQL, fuzzing, secret scanning | [Secure development policy § 2](secure-development-policy.md#2-automated-gates-on-every-push-and-pull-request) |
| A.8.30 Outsourced development | None; all first-party code is written in this repository; third-party code arrives only as policy-checked dependencies | [Supply chain § 1](supply-chain.md#1-dependency-policy) |
| A.8.31 Separation of development, test and production | Disposable per-shard test databases; no vendor production environment | [Secure development policy § 7](secure-development-policy.md#7-environments) |
| A.8.32 Change management | Gated, frozen-SHA release flow; changelog; semver report | [Secure development policy § 1](secure-development-policy.md#1-branches-and-change-flow) |

## SOC 2 Trust Services Criteria

| Criterion | How systemprompt meets it | Evidence |
|-----------|---------------------------|----------|
| CC3.4 / CC9.1 Risk identification and mitigation | Threat model; continuous dependency, secret and static analysis | [Threat model](threat-model.md), [Vulnerability management](vulnerability-management.md) |
| CC7.1 Detection of vulnerabilities | Daily scans; CodeQL; fuzzing | [Vulnerability management § 1](vulnerability-management.md#1-sources) |
| CC7.3–CC7.5 Incident response | Vendor process and notification | [Incident response](incident-response.md) |
| CC8.1 Change management | Protected `main`, required checks, frozen-SHA promotion, CI-only builds | [Secure development policy](secure-development-policy.md) |
| CC9.2 Vendor risk management | This pack; SBOM; provenance; signed releases | [Supply chain](supply-chain.md) |
