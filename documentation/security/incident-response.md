# Incident Response (Vendor Side)

What the vendor does when a security incident affects systemprompt software or its release
pipeline, and how customers are told. Addresses ISO/IEC 27001:2022 A.5.24–5.28 and SOC 2 CC7.3–CC7.5
from the supplier side. Incidents inside a customer deployment are handled by the customer's own
process. The vendor has no access to customer environments.

## 1. What counts as a vendor incident

- A vulnerability in released software rated High or Critical.
- Compromise, or suspected compromise, of the source repository, the release workflow, signing
  identity or a published artefact.
- A malicious or compromised dependency that is present in a released build.

## 2. Process

1. **Contain.** Revoke affected credentials and tokens. Disable the affected workflow or
   release. Mark compromised releases as such on GitHub and yank affected crates from crates.io.
2. **Assess.** Identify the affected versions and artefacts using the release SBOMs, provenance
   attestations and git history.
3. **Remediate.** Ship a fixed release through the normal gated flow
   ([Secure development policy](secure-development-policy.md)), within the
   [remediation targets](vulnerability-management.md#3-remediation-targets).
4. **Notify.** See § 3.
5. **Review.** Record the root cause, and add a gate or test that would have caught it to the
   repository.

## 3. Customer notification

| Channel | Content |
|---------|---------|
| GitHub Security Advisory | Affected versions, severity, fixed version, workaround |
| Direct email to customers' registered security contacts | Same content, sent no later than advisory publication; for a release-pipeline compromise, within 72 hours of confirmation |
| CHANGELOG `### Security` | Entry in the fixing release |

Customers provide a security contact address during procurement to receive direct notices.

## 4. Contact

Report a suspected incident to **ed@systemprompt.io**. See the [Security policy](../../SECURITY.md)
for encrypted channels.
