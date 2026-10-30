# Supply Chain

How third-party code enters systemprompt, how releases are built, and how a customer verifies a
release. Addresses ISO/IEC 27001:2022 A.5.19–5.22 and A.8.30 and SOC 2 CC9.2.

## 1. Dependency policy

[`deny.toml`](../../deny.toml) is the single policy for every Cargo workspace in the repository.
`cargo deny` enforces it on every push, every pull request and daily (`supply-chain.yml`).

| Check | Policy |
|-------|--------|
| Sources | crates.io only. An unknown registry or an unknown git source fails the build |
| Licences | Explicit permissive allow-list (Apache-2.0, MIT, BSD, ISC, MPL-2.0 and similar); any other licence fails |
| Advisories | RustSec advisory database. Every accepted advisory is recorded with its reachability analysis and the condition under which it is removed |
| Lockfile | Builds use `--locked` against the committed `Cargo.lock`; `scripts/check-lockfile-registry.sh` rejects non-crates.io entries |
| Vendoring | No vendored dependency sources (`scripts/check-no-vendor.sh`) |

`cargo audit` runs as a second, independent advisory scanner in the same workflow.

### Dependency updates

Renovate ([`renovate.json`](../../renovate.json)) opens grouped minor and patch updates for
Cargo dependencies and GitHub Actions once a month. It opens security updates as soon as an
advisory is published. Each update is a pull request against `next` that runs the full gate set,
and is merged only when every check passes. The merged commit is then gated again as a push to
`next`, like any other change. Major-version upgrades are made by the maintainer. A new advisory
against a dependency in use is also handled under [Vulnerability management](vulnerability-management.md).

## 2. Build and release integrity

| Control | Detail |
|---------|--------|
| Build location | GitHub-hosted runners only; no release artefact is built on a workstation |
| Build input | A tagged commit that has already passed the release gates (`scripts/release-proof.py merged`) |
| Action pinning | Every third-party Action is pinned to a full commit SHA |
| Token scope | Read-only by default; write scopes granted per job |
| Signing | Bridge binaries, checksums and SBOMs are signed with Sigstore `cosign` keyless signing; the certificate is bound to this repository and workflow and logged in the Rekor transparency log |
| Provenance | `actions/attest-build-provenance` attests every release asset (SLSA Build L2) |
| SBOM | CycloneDX 1.5: `systemprompt-bridge-sbom.cdx.json` (bridge) and `systemprompt-bridge-core-sbom.cdx.json` (core workspace) |
| Immutability | Releases are published as GitHub immutable releases; assets cannot be replaced after publication |

SBOMs and provenance attestations are attached from release 0.77.0 onward.

### Crates on crates.io

The `systemprompt-*` crates are published to crates.io from the tagged release commit. crates.io
does not support publisher signatures. Consumers who need signed inputs should build from the
tagged source revision and verify the tag against the GitHub release.

## 3. Verifying a release

```bash
# Build provenance: which workflow and commit produced this file
gh attestation verify systemprompt-bridge-x86_64-unknown-linux-gnu \
  --repo systempromptio/systemprompt-core

# Signature
cosign verify-blob \
  --certificate-identity-regexp 'https://github.com/systempromptio/systemprompt-core/.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --signature systemprompt-bridge-x86_64-unknown-linux-gnu.sig \
  --certificate systemprompt-bridge-x86_64-unknown-linux-gnu.pem \
  systemprompt-bridge-x86_64-unknown-linux-gnu

# Checksums
sha256sum -c SHA256SUMS --ignore-missing
```

The same commands apply to the `*.cdx.json` SBOM files.

## 4. Repository security signals

- **OpenSSF Scorecard** runs weekly and publishes to the OpenSSF API (`scorecard.yml`). The
  *Code-Review* check scores low by design, because the project has one maintainer (see
  [Secure development policy § 6](secure-development-policy.md#6-review-and-segregation-of-duties)).
- **CodeQL** results and **secret-scanning** results appear under the repository's Security tab.
