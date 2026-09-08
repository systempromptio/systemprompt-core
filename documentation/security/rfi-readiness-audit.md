# RFI Readiness Audit

Snapshot of the codebase's RFI / enterprise-security review posture. Every item below reflects the state of `next` at the audit date; re-run the verification column to refresh.

**Audit date:** 2026-08-28
**Snapshot:** `next`, workspace version 0.41.0.

`next` is the default branch and where all development lands. `main` is release-only and
protected. Earlier revisions of this document were pinned to `main` at 0.12.0; that was
roughly 1,500 commits and 29 minor releases ago, and much of what follows is a correction of
drift rather than a change in posture. The revision log at §8 records what changed and why.

## 0. What external verification we hold

Stated plainly, because it is the first question a security reviewer asks:

**We hold no third-party security certification or assessment.** No SOC 2, no ISO 27001, no
HITRUST, and no commissioned penetration test. Every assurance artefact in this repository is
first-party attestation backed by source you can read and checks you can re-run.

That is a deliberate position, not an oversight:

- **A vendor certification would attest to the wrong thing.** SOC 2 and ISO 27001 audit the
  *vendor's* operating environment. In this deployment model we do not operate anything: the
  binary runs inside your network, against a database you own, and no customer data reaches
  us. A SOC 2 report on systemprompt.io Ltd would describe controls that are not in the path
  of your risk.
- **What *is* in that path is the software**, and the software is source-available. The
  [compliance control matrix](compliance-control-matrix.md) maps it control by control so it
  can be brought inside *your* audit scope.
- **The strongest available assessment is one you run yourself.** You can read every line,
  stand up your own deployment, and test it under your own rules of engagement. We support
  customer-commissioned testing under commercial agreement, including scoping, coordination
  and remediation.

A commissioned third-party assessment is planned and is tracked in §6. Where a customer's
programme requires one on a specific timeline, co-funding that engagement is welcomed and can
be scoped commercially.

## 1. Documentation Artefacts

| Artefact | Location | Status |
|----------|----------|--------|
| Vulnerability disclosure policy | `SECURITY.md` | published |
| Public evaluation pack entry point | `documentation/README.md` | published |
| STRIDE threat model | `documentation/security/threat-model.md` | published |
| Production deployment runbook | `documentation/guides/deploy-production.md` | published |
| Compliance control matrix (HIPAA / SOC 2 / ISO 27001) | `documentation/security/compliance-control-matrix.md` | published |
| Stability contract | `documentation/security/stability-contract.md` | published |
| Compatibility matrix (providers, protocols, runtime) | `documentation/reference/compatibility.md` | published |
| Architecture (layered crates) | `README.md` + `CLAUDE.md` (repository root) | published |
| Change history | `CHANGELOG.md` | active, per-release entries |
| Licence | `LICENSE` — BUSL-1.1 with four-year conversion to Apache 2.0 | published |

## 2. Supply Chain

| Check | Tool | Status | Evidence |
|-------|------|--------|----------|
| RustSec advisory scan | `cargo deny check advisories` | passing, 14 documented ignores | `deny.toml` |
| Licence compliance | `cargo deny check licenses` | passing | `deny.toml` |
| Registry source lock | `cargo deny check sources` | passing (crates.io only, no git sources) | `deny.toml` |
| Duplicate / banned crates | `cargo deny check bans` | passing | `deny.toml` |

`cargo deny` runs against **all seven workspaces** in the repository (root, `bin/bridge`,
`crates/tests` and its four sub-workspaces) from a single shared `deny.toml`, so no workspace
can drift onto its own policy. It executes on every push, every pull request, and a daily
schedule.

### 2.1 Accepted-risk register

`deny.toml` suppresses 14 advisories, each with a written justification recorded inline. We
publish the register rather than reporting a clean scan; the full rationale for each entry is
in the file itself and the two reachable entries are summarised in
[compliance-control-matrix.md §5.1](compliance-control-matrix.md).

**Reachable from the published crates (2):**

| Advisory | Crate | Basis for acceptance |
|----------|-------|----------------------|
| RUSTSEC-2023-0071 | `rsa`, via `jsonwebtoken` | Marvin timing attack, no upstream fix available. The exploitable surface is RSA *private-key decryption*; the JWT plane performs RS256 signature *verification* only, and no ES/EdDSA acceptance path exists anywhere in the codebase. Verification is authenticated and CPU-bounded rather than an unauthenticated high-throughput endpoint. Removed as soon as a fixed `rsa` ships. |
| RUSTSEC-2026-0173 | `proc-macro-error2`, via `tabled` and `validator` | Unmaintained. Build-time proc-macro only; never linked into the runtime binary, so it carries no runtime security surface. |

**Reachable only from the bridge and test workspaces (12):** `RUSTSEC-2026-0194`,
`RUSTSEC-2026-0195` (`quick-xml` via `wayland-scanner`), `RUSTSEC-2026-0192` (`ttf-parser`),
`RUSTSEC-2024-0370` (`proc-macro-error`), and the GTK3 cluster `RUSTSEC-2024-0412`, `-0413`,
`-0415`, `-0416`, `-0418`, `-0419`, `-0420`, plus `RUSTSEC-2024-0429` (`glib`). All sit under
`winit`/`wry`, whose Linux windowing backend the bridge never compiles — the tray and webview
stack is declared only for macOS and Windows, but Cargo resolves the Linux branch into the
lockfile regardless of target. No shipped artefact contains this code and no upstream release
moves `winit` off it.

The register is complete: no advisory is suppressed without an entry.

### 2.2 Known limitation of the current gate

`cargo deny` does not surface advisories marked `informational = "unsound"`. A local trial run
on 2026-08-28 comparing it against `cargo audit` over the same lockfile and advisory database
found one such advisory reachable through `sqlx` that the gate had reported clean
(`RUSTSEC-2026-0221`, `event-listener`), plus one yanked crate. Both were remediated the same
day by lockfile update. Reinstating `cargo audit` alongside `cargo deny` — sharing one ignore
list, so the two cannot drift — is tracked in §6.

## 3. Continuous Integration

Six workflows, all in `.github/workflows/`:

| Workflow | File | Triggers | What it runs |
|----------|------|----------|--------------|
| CI | `ci.yml` | push + PR on `main`/`next`; manual dispatch with a `ref` input | `cargo fmt --check`, `cargo build --workspace --locked`, sqlx offline cache verification, and a **13-way sharded test matrix** under `cargo-nextest` against a PostgreSQL service |
| Quality | `quality.yml` | push + PR on `main`/`next`; manual dispatch with a `ref` input | clippy (root + bridge), native bridge clippy on macOS and Windows, rustdoc with `-D warnings`, **16 source-gate linters**, an MSRV check against 1.96, unused-dependency detection, and a file-size guard |
| Supply Chain | `supply-chain.yml` | push + PR on `main`/`next`; daily cron; manual dispatch with a `ref` input | `cargo deny` across all seven workspaces |
| Coverage | `coverage.yml` | push + PR on `main`; manual dispatch | Instrumented build, full test workspace, LCOV/JSON artefacts, a coverage floor plus ratchet, Codecov upload via OIDC |
| Coverage (bridge) | `coverage-bridge.yml` | weekly cron; manual dispatch | macOS + Windows coverage for the cfg-gated GUI and keystore code |
| Exercise Suites | `exercise-suites.yml` | weekly cron; manual dispatch | Fuzz-target smoke runs, benchmark compile checks, load-test build |
| Bridge release signing | `release-sign.yml` | `bridge-v*` tags; manual dispatch | Three-target build, SHA256SUMS, Sigstore `cosign` keyless signing, GitHub release publication |

Coverage deliberately runs against `main` rather than `next` — it tracks the released line, so
the published figure always describes what was actually released. It is not on a cron.

**No CodeQL, SBOM, OpenSSF Scorecard, OSV-Scanner or dependency-review workflow is
configured**, and no `dependabot.yml` exists. See §6.

### 3.1 Change management

This is the strongest control in the repository and is worth stating explicitly for SOC 2 CC8.1:

- All development lands on `next`. Every push runs CI, Quality and Supply Chain in full.
- `main` is protected by a ruleset requiring a pull request with **no bypass for anyone** — a
  direct push is refused for maintainers and repository admins alike. Protection is pinned to
  `main` by name, so moving the default branch does not move it.
- Promotion is deliberate and two-step. `just gate [REF]` dispatches CI, Quality and Supply
  Chain against a pinned SHA and waits for all three. `just promote [SHA]` then freezes that
  exact commit on a `promote` ref and *opens* a pull request onto `main`; it does not merge.
- The commit is frozen on `promote` rather than the PR being headed at `next` precisely so
  that nothing pushed in the interim can ride along ungated.
- `just check-release-tag` verifies every released CHANGELOG version carries its git tag.

### 3.2 Local verification

To reproduce the build and supply-chain checks from a fresh clone:

```bash
git clone https://github.com/systempromptio/systemprompt-core
cd systemprompt-core
cargo +nightly fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
SQLX_OFFLINE=true cargo build --workspace --locked
cargo deny check
```

All four complete without errors on a clean checkout. Tests require a PostgreSQL instance and
run sharded — `just install-nextest` then `just test-all-shards`, or `just test-shard <group>`
for one group. `scripts/test-shard.sh --list` prints the current groups. A bare
`cargo test --workspace` is **not** the supported path and will not reproduce CI.

## 4. Code and Architecture

| Property | Evidence |
|----------|----------|
| Memory-safe language, `unsafe` denied by lint | Rust edition 2024; `unsafe_code = "deny"` set workspace-wide with **no `#[allow(unsafe_code)]` in any production crate**. Five `unsafe` blocks exist in the published crates, all confined to one platform-syscall module for subprocess management (`crates/shared/models/src/subprocess/{linux,darwin}.rs`). The desktop bridge carries further FFI blocks for Win32 and macOS process APIs. **There is no `unsafe` in any cryptographic path.** |
| Panic-free discipline enforced by lint | `unwrap_used = "deny"` and `expect_used = "deny"` workspace-wide, alongside `clippy::all` and `clippy::suspicious` at deny |
| Request-path SQL is compile-time verified | All request-path queries use the `sqlx::query!` macro family, checked against the live schema at build time. Enforced mechanically by `scripts/check-sqlx.sh` (CI: `just lint-sqlx`), which fails the build on any non-macro `sqlx::query*(` outside a small bootstrap allowlist. Currently passing clean. |
| Typed identifiers (no raw `String` IDs) | `crates/shared/identifiers/`, gated by `just lint-raw-ids` |
| Layer discipline mechanically enforced | `shared → infra → domain → app → entry`, with no domain-to-domain dependencies. Gated by `scripts/lint-layers.sh`; its allowlist is empty by design |
| Authorization cannot be omitted by accident | Attaching the auth context layer is only possible through `RouterExt::with_auth(auth, policy)`, which requires an `AuthzPolicy` in the same call — omitting it is a **compile error**, not a review miss. The gate fails closed: a request with no context is treated as anonymous |
| Mandatory, per-surface JWT audience validation | `crates/infra/security/src/jwt/validate.rs:65-88` — the audience list is applied unconditionally and a policy declaring none is rejected outright, so a permissive configuration cannot be expressed |
| RS256-only JWT plane | `jwt/validate.rs:70` rejects any non-RS256 `alg` before validation is constructed; `kid` is mandatory; no ES/EdDSA path exists |
| Server-side PKCE mandate | `S256` only, `plain` rejected, constant-time comparison (`domain/oauth/.../auth_code/pkce.rs:41-44`) |
| Caller credentials never forwarded upstream | The gateway substitutes the deployment's provider key and unconditionally strips the caller's user identifier from outbound requests |
| Single binary, no dynamic code loading | Extensions register at compile time via `inventory` (`crates/shared/extension/`) |
| Postgres-only persistence | `crates/infra/database/` |
| Secrets never held in ciphertext by the binary | Loaded from a profile-referenced file or environment; customer envelope encryption (KMS / Vault / sops) keeps the master key outside the binary entirely |
| Structured audit pipeline | `crates/infra/logging/schema/{log,analytics}.sql`; every authorization decision, including denials, writes a `governance_decisions` row. Append-only is an operator-provisioned DB role grant, documented in deployment guide §4.1.1, not shipped in migrations |

The main workspace is **33 crates** across five layers: 7 shared, 7 infra, 13 domain, 3 app,
2 entry, plus the `systemprompt` facade. `crates/tests/` and `bin/bridge` are separate
workspaces.

### 4.1 Test estate and quality gates

- The `crates/tests/` workspace declares **92 member crates** across unit tests per layer,
  integration, contract, concurrency and property suites, plus shared test utilities. Fuzz,
  bench and load-test crates are standalone workspaces exercised by `exercise-suites.yml`.
- CI runs the suite in **14 shards** (`shared`, `infra`, `domain`, `app-runtime`,
  `app-scheduler`, `app-generator`, `entry-api`, `entry-cli`, `bridge`, `integration-api`,
  `integration-cli`, `integration-rest-1`, `integration-rest-2`, `edge`) under `cargo-nextest`, each against a fresh,
  freshly-migrated database. `scripts/test-shard.sh` is the single source of truth for the
  shard definitions, shared by CI and the local recipes.
- Four fuzz targets are maintained: `a2a_request`, `config_loading`, `identifier_validation`,
  `jsonrpc_parse`.
- **Coverage.** `coverage.yml` enforces a floor of **88.5% aggregate line coverage** plus a
  ratchet that fails the run on any drop greater than 0.5 points against the previous
  successful run. The floor is the citable figure: a passing run means coverage is at or above
  it. Per-crate figures are deliberately not reproduced here — the previous revision of this
  document carried a table that was months stale by the time anyone read it. Re-run
  `just coverage` for a current breakdown.
- **Current status:** the coverage workflow has been failing since 2026-08-23 in its
  instrumented-test step; the most recent successful measurement was 2026-08-07. This is a
  tooling failure rather than a coverage regression, and is tracked in §6.

## 5. Pre-answered Enterprise Security Questionnaire

Full pre-answers are in [compliance-control-matrix.md §4](compliance-control-matrix.md).
Headline answers:

- **Certifications**: none, by design — see §0. Control-level mappings are provided instead.
- **BAA**: not applicable. systemprompt.io as a vendor does not create, receive, maintain, or
  transmit PHI; the binary runs inside the customer's compliance boundary.
- **Data location**: the customer's own Postgres, under customer control. The vendor never
  sees customer data.
- **Encryption**: TLS 1.2+ in transit, enforced at entry. Secrets-at-rest via the customer's
  envelope-encryption infrastructure — the master key never enters the binary. Database
  encryption at rest is customer-managed.
- **SSO**: OIDC through the customer's IdP. WebAuthn also supported.
- **Multi-tenancy**: per-user filtering and a deny-overrides authorization resolver. **Core
  ships no row-level tenant isolation** — see §6.
- **SBOM**: generated on demand from the committed `Cargo.lock` with `cargo cyclonedx`. Not
  currently attached by CI.
- **Release integrity**: bridge binaries are Sigstore-signed with a Rekor transparency-log
  entry. The core platform ships as source and crates.io packages; organisations that
  repackage it sign the resulting artefact packs under their own key and provenance.
- **Penetration testing**: none commissioned to date. See §0.
- **Business continuity**: source-available under BUSL-1.1 with automatic conversion to
  Apache 2.0 four years after each version's publication. The customer keeps indefinite usage
  rights and can operate without vendor involvement.

## 6. Known Gaps (Honest List)

Artefacts and controls an enterprise reviewer might reasonably ask for that are **not** in
place. None are blocking for an RFI response; all are addressable under a commercial
engagement timeline.

| Gap | Why it matters | Plan |
|-----|----------------|------|
| **No row-level tenant isolation** | The database scoping layer is an opt-in seam: with no registered provider it degenerates to an unscoped transaction, and no RLS policies ship in any migration. Isolation rests on per-`user_id` repository filtering and the authz resolver | Deployments needing hard separation should register a scope provider and author RLS policies, or run one instance per tenant. Named as a priority target for external assessment |
| Third-party penetration test report | Large regulated buyers frequently require one | Commission before first enterprise deployment, or invite the customer to run their own. Co-funding welcomed — see §0 |
| SOC 2 Type I / II attestation | A common procurement checkbox, though see §0 for why it attests to the wrong scope in this model | Revisit when customer count and team size justify the audit cost |
| CodeQL static analysis | A credible third-party SAST signal. The repository is public, so CodeQL is free | Enable default setup and cite once it has produced a run |
| SBOM CI workflow | A CI-attached CycloneDX SBOM per release is a common procurement requirement | Author the workflow; until then the SBOM is generated on demand |
| Secret scanning and push protection | Both are free for this public repository and are currently **disabled** | Enable; low effort, immediate value |
| No `dependabot.yml`; Actions pinned to floating tags | Every workflow references Actions by mutable tag rather than commit SHA, which is a real supply-chain exposure and an OpenSSF Scorecard finding | Add Dependabot for `cargo` and `github-actions`; the latter also migrates the pins |
| `cargo audit` not run alongside `cargo deny` | The gate is blind to `unsound` informational advisories — demonstrated, not theoretical (§2.2) | Reinstate with a shared ignore list |
| Coverage workflow failing since 2026-08-23 | The published coverage figure cannot currently be refreshed | Diagnose the instrumented-test step; unrelated to code coverage itself |
| Documented PostgreSQL floor is untested | Documentation requires 18+; every CI container is PostgreSQL 16 | Reconcile — either raise the CI containers or lower the documented floor to what is tested |
| SSRF guard does not resolve DNS | The outbound URL guard is parse-time only, so a hostname resolving to a link-local metadata address is not blocked, and redirects are not re-validated | Design fix under evaluation; named as a priority target for external assessment |
| Cyber liability + E&O insurance certificate | Typical procurement checkbox | Particulars available under NDA; bind before contract signature |
| Formal incident-response playbook beyond `SECURITY.md` | A full IR runbook for customer-facing incidents | Draft alongside the first paid customer |

## 7. Verification

Every claim in this document is checkable from a clean checkout. The commands in §3.2
reproduce the build and supply-chain posture; `deny.toml` carries the accepted-risk register;
`.github/workflows/` carries the CI claims; and the file:line citations throughout §4 point at
the enforcing code. Where this document states a gap, the absence is verifiable too — for
example, `ls .github/workflows/` shows no CodeQL or SBOM workflow.

A CycloneDX SBOM can be generated on demand with `cargo cyclonedx`; this is a manual step and
is not produced by CI.

## 8. Revision

| Date | Change |
|------|--------|
| 2026-04-23 | Initial audit following an enterprise RFI inbound. 37 Dependabot advisories resolved to 1 LOW documented ignore; public evaluation pack shipped. |
| 2026-05-22 | Fidelity pass against `main` (0.11.1). Corrected the RUSTSEC-2023-0071 rationale to the real RS256-only mitigations. Corrected the test-workspace figure and flagged the per-crate coverage table as a dated snapshot. Noted `validate_aud=false` as a tracked open item. |
| 2026-05-22 | Recorded `release-sign.yml` and reframed the signing gap as core-platform signing. |
| 2026-05-27 | Re-pinned snapshot to 0.12.0. Recorded the authz surface refactor to an attribute bag and the `RuleBasedHook` promotion. |
| 2026-08-28 | Full fidelity pass against `next` @ 0.41.0 after ~1,500 commits and 29 minor releases of drift. **Corrected the accepted-risk register from a claimed "exactly one advisory" to the actual 14, and published it (§2.1)** — the prior understatement was the single most material defect in the pack. **Corrected the "zero `unsafe` outside crypto primitives" claim**, which was wrong in both directions: there is no `unsafe` in crypto, and five blocks exist in a platform-syscall module. **Closed two gaps that code had already fixed but the pack still advertised as open** — mandatory per-surface audience validation and the server-side PKCE mandate. **Removed core-platform release signing as a gap**: organisations that repackage the platform sign artefact packs under their own provenance, which is the correct place for that authority. Rewrote §3 for the real six-workflow, `next`-based, gate-and-promote flow, and added §3.1 because the change-management story is materially stronger than the pack had claimed. Corrected the test estate to 92 crates and 13 shards, and replaced the stale per-crate coverage table with the enforced floor. Added §0 to answer the external-verification question directly rather than leaving it to be inferred. **Added the absence of row-level tenant isolation and the SSRF guard's DNS limitation to the gaps list** — both are real, both were previously unstated. Recorded the `cargo deny` blind spot to unsound advisories, found by a local scanner trial the same day. |
