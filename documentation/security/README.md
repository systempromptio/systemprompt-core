# Supplier Evidence Pack

This pack answers a supplier security assessment of systemprompt: ISO/IEC 27001:2022 Annex A
5.19–5.23 and 8.25–8.31, SOC 2 CC8.1 and CC9.2, and the CAIQ or vendor questionnaires built on them.

## The supplier relationship

systemprompt is self-hosted software. The customer installs and runs it inside their own
infrastructure and compliance boundary. The vendor:

- operates no hosted service for a self-hosted deployment;
- has no access to the customer's environment, credentials or data;
- receives no telemetry, crash reports, licence checks or update pings from the binary
  ([Security policy § Telemetry](../../SECURITY.md#telemetry)).

The vendor's controls therefore cover **how the software is built and released**, not how it is
operated. [Shared responsibility](shared-responsibility.md) sets out the split.

## Assessor questions and where they are answered

| Question | Document |
|----------|----------|
| How is the software developed, reviewed and changed? | [Secure development policy](secure-development-policy.md) |
| How are dependencies controlled, and are releases signed, attested and accompanied by an SBOM? | [Supply chain](supply-chain.md) |
| How are vulnerabilities found, triaged and fixed, and within what SLA? | [Vulnerability management](vulnerability-management.md), [Security policy](../../SECURITY.md) |
| How will we be told about a security incident or advisory? | [Incident response](incident-response.md) |
| What data does the software handle, where does it go, and how long is it kept? | [Data handling](data-handling.md), [Outbound egress controls](outbound-egress-controls.md) |
| Who is responsible for which control? | [Shared responsibility](shared-responsibility.md) |
| What happens if the vendor stops trading? | [Business continuity](business-continuity.md) |
| Which threats are modelled and mitigated? | [Threat model](threat-model.md) |
| How do features map to SOC 2 / ISO 27001 / HIPAA controls? | [Supplier control mapping](soc2-iso-mapping.md), [Compliance control matrix](compliance-control-matrix.md) |
| Pre-answered questionnaire | [CAIQ v4 response](questionnaires/caiq-v4.md) |
| Technical review reference, accepted risks and limitations | [Security review reference](rfi-readiness-audit.md) |
| API and schema stability commitments | [Stability contract](stability-contract.md) |

## Independently verifiable evidence

Every claim in this pack can be checked without trusting the vendor:

| Evidence | Where |
|----------|-------|
| Source, history and CI definitions | This repository; `.github/workflows/` |
| Gate results for any commit | GitHub Actions runs for that SHA |
| Signed binaries, SBOMs and provenance | GitHub releases (`bridge-v*`); see [Supply chain § Verifying a release](supply-chain.md#verifying-a-release) |
| Static analysis and secret scanning results | Repository Security → Code scanning (CodeQL, Scorecard) |
| Security-practice score | [OpenSSF Scorecard](https://scorecard.dev/viewer/?uri=github.com/systempromptio/systemprompt-core) |
| Dependency exception register | [`deny.toml`](../../deny.toml) |

## Not included

The pack does not contain a SOC 2 report, an ISO 27001 certificate or a third-party penetration
test report. Request current status from the vendor. The [control mapping](soc2-iso-mapping.md)
describes software capabilities and vendor practices; it is not a certification or an
assessment result.
