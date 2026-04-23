# Public Documentation

This directory is the public evaluation pack for systemprompt.io — intended for prospective customers, security reviewers, procurement teams, and anyone conducting an RFI, RFP, or due-diligence exercise against the codebase.

It is deliberately separate from `instructions/`, which is local-only engineering documentation and is gitignored. Everything in `documentation/` is stable, versioned with the repository, and safe to cite in a procurement response.

## Contents

### Security

- **[threat-model.md](security/threat-model.md)** — STRIDE-style threat model of the governance pipeline. Components, trust boundaries, threats, mitigations mapped to code paths, residual risk.
- **[deployment-reference-architecture.md](security/deployment-reference-architecture.md)** — Production deployment runbook. HA Postgres, backup/restore, disaster recovery, key rotation, monitoring and SIEM integration, air-gap topology, update and rollback procedures.
- **[compliance-control-matrix.md](security/compliance-control-matrix.md)** — Mapping of HIPAA Security Rule, SOC 2 Trust Services Criteria, and ISO 27001 Annex A controls to architectural features and code paths. The "architecture supports" framing made explicit: customer owns the compliance boundary; systemprompt is a source-available binary running in the customer's environment.
- **[stability-contract.md](security/stability-contract.md)** — What is stable in systemprompt vs. what tracks upstream AI provider APIs. Versioning policy, deprecation policy, API guarantees.
- **[compatibility-matrix.md](security/compatibility-matrix.md)** — Upstream AI provider APIs, MCP spec revisions, and A2A protocol versions supported by each systemprompt release.

## For RFI / Security Review Teams

If you are conducting a vendor evaluation, start with [compliance-control-matrix.md](security/compliance-control-matrix.md). It answers the majority of standard security questionnaire questions and links to the supporting evidence in this repository.

For architectural questions about how specific controls are implemented, cross-reference the threat model and the file paths it cites in `crates/`. The code is source-available under BSL-1.1 — every claim in these documents can be verified by reading the implementation.

For operational questions (HA, DR, monitoring), see the deployment reference architecture.

Contact: **ed@systemprompt.io** for all licensing, security, and RFI correspondence.

## Reading Order

1. Repository [README.md](../README.md) — product overview and positioning
2. [compliance-control-matrix.md](security/compliance-control-matrix.md) — fastest path to answer "does this meet our controls?"
3. [threat-model.md](security/threat-model.md) — how the architecture defends against realistic threats
4. [deployment-reference-architecture.md](security/deployment-reference-architecture.md) — how it runs in production
5. [stability-contract.md](security/stability-contract.md) and [compatibility-matrix.md](security/compatibility-matrix.md) — what we guarantee over time

## License

All content in this directory is published under the same BSL-1.1 licence as the rest of the repository. See [LICENSE](../LICENSE).
