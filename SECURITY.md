# Security Policy

systemprompt.io is self-hosted AI governance infrastructure. Customers run the binary inside their own compliance boundary, so vulnerabilities in this codebase can affect regulated environments (HIPAA, SOC 2, ISO 27001 programmes). We take reports seriously and prioritise coordinated disclosure.

## Reporting a Vulnerability

Email: **ed@systemprompt.io**

Include, where possible:

- Affected version(s) and commit hash
- Reproduction steps or proof-of-concept
- Impact assessment (confidentiality / integrity / availability)
- Any suggested mitigation

Please do not open public GitHub issues for security reports. If you need an encrypted channel or a Signal handoff, request one in your initial email and we will arrange it.

## Response SLA

| Stage | Target |
|-------|--------|
| Acknowledgement of report | 48 hours |
| Triage and severity assignment | 5 business days |
| Fix for Critical (CVSS 9.0+) | 14 days |
| Fix for High (CVSS 7.0–8.9) | 30 days |
| Fix for Medium / Low | Next scheduled release |
| Coordinated public disclosure | 90 days from report, or on patch release — whichever is earlier |

We will keep you informed throughout triage. If a fix requires more time than the SLA above, we will tell you why and agree an extended timeline in writing.

## Supported Versions

| Version | Security fixes |
|---------|----------------|
| 0.41.x (current minor) | Yes |
| 0.40.x (previous minor) | Critical and High only |
| < 0.40 | No |

While the project is pre-1.0, security fixes land on the current minor release. The
previous minor receives Critical and High fixes until the minor after it ships, at
which point it moves out of support. Production deployments should track the latest
`0.x` minor release.

## Scope

In scope for this policy:

- The `systempromptio/systemprompt-core` repository and all crates it publishes to crates.io under `systemprompt-*`
- The `systemprompt` facade crate
- The `systemprompt-bridge` binary, including its manifest-apply, cloud-backup and credential-helper flows
- Official release binaries attached to GitHub releases

Out of scope:

- The marketing site at `systemprompt.io` (report via the same email, but not governed by this SLA)
- Third-party extensions built against our public trait interfaces
- Vulnerabilities in upstream dependencies that we cannot remediate (we will forward to maintainers)
- Denial-of-service via unrealistic resource consumption against a local development build

## Safe Harbour

We will not pursue legal action against researchers who:

- Make a good-faith effort to avoid privacy violations, data destruction, and service disruption
- Only interact with accounts and data they own or have explicit permission to test
- Report the vulnerability privately to the address above and give us reasonable time to remediate before public disclosure

If your research requires testing against a production deployment not owned by you, obtain written permission from that deployment's owner first.

## Supply Chain

- Dependencies are audited continuously with `cargo deny` against the RustSec advisory
  database, covering advisories, licences, banned crates and registry sources across
  every workspace in the repository. It runs on each push, on each pull request, and on
  a daily schedule — see `.github/workflows/supply-chain.yml`.
- Advisories we have assessed and accepted are recorded, each with a written
  justification, in [`deny.toml`](deny.toml). We publish the register rather than
  reporting a clean scan: as of 0.41.0 it holds two entries reachable from the published
  crates, and twelve reachable only from the desktop bridge's Linux windowing stack,
  which the bridge does not compile.
- `systemprompt-bridge` release binaries are built in GitHub-hosted CI runners from
  tagged commits and signed with Sigstore (`cosign` keyless, OIDC-bound to this
  repository and workflow) — see `.github/workflows/release-sign.yml`. Verify one with:
  ```
  cosign verify-blob \
    --certificate-identity-regexp 'https://github.com/systempromptio/systemprompt-core/.*' \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com \
    --signature <artifact>.sig \
    --certificate <artifact>.pem \
    <artifact>
  ```
- The core platform ships as source and as `systemprompt-*` crates on crates.io; those
  artefacts are not signed by us. Organisations that repackage the platform for internal
  distribution sign the resulting artefact packs under their own key and provenance.
- A CycloneDX SBOM can be generated on demand from the committed `Cargo.lock` with
  `cargo cyclonedx`. It is not currently produced or attached by CI.

## Further Reading

- [Threat Model](documentation/security/threat-model.md)
- [Compliance Control Matrix](documentation/security/compliance-control-matrix.md)
- [RFI Readiness Audit](documentation/security/rfi-readiness-audit.md) — security-review posture, accepted risks, and known gaps
- [Stability Contract](documentation/security/stability-contract.md)
- [Compatibility Matrix](documentation/reference/compatibility.md)
- [Deploy in Production](documentation/guides/deploy-production.md)
