# Shared Responsibility

The split of security responsibilities for a self-hosted systemprompt deployment. Use it to
scope systemprompt within a customer's ISO/IEC 27001 statement of applicability or SOC 2 system
description. Because systemprompt is software the customer operates, it is normally assessed as a
**software supplier** (A.5.19–5.22, CC9.2), not as a service organisation.

| Area | Vendor | Customer |
|------|--------|----------|
| Secure development and change control | ✔ [Secure development policy](secure-development-policy.md) | |
| Dependency and supply-chain control | ✔ [Supply chain](supply-chain.md) | Verify releases before deployment |
| Vulnerability fixes and advisories | ✔ [Vulnerability management](vulnerability-management.md) | Subscribe to advisories; apply updates |
| Release signing, SBOM, provenance | ✔ | Verify signatures and attestations |
| Secure defaults and documentation | ✔ [Production deployment](../guides/deploy-production.md) | |
| Hosting, network, physical security | | ✔ |
| TLS termination and ingress | | ✔ |
| Identity provider, MFA, joiner/mover/leaver | | ✔ (systemprompt consumes IdP claims) |
| Role and policy configuration | Provides RBAC and governance policies | ✔ Defines and reviews them |
| Database roles, encryption at rest, backups | Documents the required grants and procedures | ✔ |
| Secrets storage and rotation | Documents rotation | ✔ |
| Log retention, SIEM, monitoring | Emits the audit stream and metrics | ✔ |
| Upstream AI provider selection and agreements | | ✔ |
| Incident response in the deployment | Supplies the audit trail; vendor-side notices ([Incident response](incident-response.md)) | ✔ |
| Business continuity of the deployment | Source availability ([Business continuity](business-continuity.md)) | ✔ Backups, DR and restore tests |
