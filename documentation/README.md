# Public Documentation

This directory is the public evaluation pack for systemprompt.io — intended for prospective customers, security reviewers, procurement teams, and anyone conducting an RFI, RFP, or due-diligence exercise against the codebase.

It is deliberately separate from `internal/`, which is local-only engineering documentation (architecture notes, release runbooks, audits, evaluations) and is gitignored. Everything in `documentation/` is stable, versioned with the repository, and safe to cite in a procurement response.

## Contents

- **[compliance-control-matrix.md](compliance-control-matrix.md)** — HIPAA Security Rule, SOC 2 Trust Services Criteria, and ISO/IEC 27001 Annex A controls mapped to architectural features and code paths. Makes the "architecture supports" framing explicit: the customer owns the compliance boundary; systemprompt is a source-available binary running in the customer's environment. Includes pre-answers to the common enterprise security questionnaires (CAIQ, SIG, VSAQ).
- **[threat-model.md](threat-model.md)** — STRIDE-style threat model of the governance pipeline: components, trust boundaries, assets, threats, mitigations mapped to code paths, and residual risk.
- **[deployment-reference-architecture.md](deployment-reference-architecture.md)** — Production deployment and operations reference: HA Postgres, backup/restore, disaster recovery, key rotation, monitoring and SIEM integration, air-gap topology, and update/rollback procedures.
- **[stability-contract.md](stability-contract.md)** — What is stable in systemprompt versus what tracks upstream AI provider APIs. Versioning policy, deprecation policy, and the per-version customer commitments.
- **[compatibility-matrix.md](compatibility-matrix.md)** — Upstream AI provider APIs, MCP spec revisions, A2A protocol versions, and runtime versions supported by each systemprompt release.
- **[rfi-readiness-audit.md](rfi-readiness-audit.md)** — A dated snapshot of the codebase's RFI / enterprise-security review posture: documentation artefacts, supply-chain checks, CI status, test coverage, and an honest list of known gaps. Reproducible from a clean clone.

## For RFI / security review teams

If you are conducting a vendor evaluation, start with [compliance-control-matrix.md](compliance-control-matrix.md). It answers the majority of standard security-questionnaire questions and links to the supporting evidence in this repository.

For architectural questions about how specific controls are implemented, cross-reference the threat model and the file paths it cites in `crates/`. The code is source-available under BSL-1.1 — every claim in these documents can be verified by reading the implementation.

For operational questions (HA, DR, monitoring, air-gap), see the deployment reference architecture.

Contact: **ed@systemprompt.io** for all licensing, security, and RFI correspondence.

## Reading order

1. Repository [README.md](../README.md) — product overview and positioning
2. [compliance-control-matrix.md](compliance-control-matrix.md) — fastest path to answer "does this meet our controls?"
3. [threat-model.md](threat-model.md) — how the architecture defends against realistic threats
4. [deployment-reference-architecture.md](deployment-reference-architecture.md) — how it runs in production
5. [stability-contract.md](stability-contract.md) and [compatibility-matrix.md](compatibility-matrix.md) — what we guarantee over time
6. [rfi-readiness-audit.md](rfi-readiness-audit.md) — current evaluation posture and known gaps

## License

All content in this directory is published under the same BSL-1.1 licence as the rest of the repository. See [LICENSE](../LICENSE).
</content>
