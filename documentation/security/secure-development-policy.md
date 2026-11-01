# Secure Development Policy

How systemprompt source changes are written, verified and released. Addresses ISO/IEC 27001:2022
A.8.25 (secure development life cycle), A.8.26–8.29 (application security requirements,
architecture, secure coding, security testing), A.8.32 (change management) and SOC 2 CC8.1.

Every control listed here is enforced by a file in this repository or by a hosted GitHub
setting. Each row names that file or setting.

## 1. Branches and change flow

| Control | Enforcement |
|---------|-------------|
| All development lands on `next`, the default branch | Repository default branch |
| `main` is release-only and cannot be pushed to directly by anyone, administrators included | GitHub ruleset on `main`: pull request required, no bypass actors |
| A release promotes one frozen, already-gated commit; nothing pushed afterwards can join it | `just promote` freezes the SHA on the `promote` ref; `just merge-release` merges with `--match-head-commit` (`justfile`, `scripts/release-proof.py`) |
| A release PR merges only when its own gate runs are green | Ruleset required checks: `CI passed`, `Quality passed`, `Supply Chain passed` |
| The release tag must match the changelog | `just check-release-tag` |
| Each break in the public API is recorded in the changelog | `semver.yml` (cargo-semver-checks report), `### Breaking` CHANGELOG entries |

## 2. Automated gates on every push and pull request

| Gate | Workflow | What it proves |
|------|----------|----------------|
| Formatting | `ci.yml` | `rustfmt` clean |
| Locked offline build | `ci.yml` | Builds from the committed `Cargo.lock` and per-crate SQLx caches |
| Test suite | `ci.yml` | Sharded unit and integration tests, each shard on a freshly migrated PostgreSQL database |
| Schema upgrade ladder | `schema-ladder.yml` | The two newest releases' databases upgrade cleanly and match a fresh install |
| Lints | `quality.yml` | Clippy with `-D warnings`; workspace lints deny unrestricted `unsafe`, `unwrap` and `expect` |
| Rustdoc | `quality.yml` | Documentation builds with warnings denied |
| Source-gate linters | `quality.yml` | Architectural layering, runtime-SQL allowlist, error-handling, environment-variable, test-seam, fail-open and owned-task rules (`scripts/`) |
| MSRV | `quality.yml` | Builds on the declared minimum Rust version |
| Dependency policy | `supply-chain.yml` | `cargo deny` and `cargo audit` ([Supply chain](supply-chain.md)) |
| Secret scanning | `supply-chain.yml` | `gitleaks` over the tracked tree |
| Static analysis | `codeql.yml` | CodeQL for Rust, JavaScript and workflow files (push to `next`, weekly) |
| Fuzz and load smoke | `exercise-suites.yml` | Fuzz targets, benchmark and load-test suites (weekly) |

Workflows run on GitHub-hosted runners. Every third-party Action is pinned to a full commit
SHA, and every workflow declares a read-only default token, granting write scopes per job only.

## 3. Secure coding standards

- **Memory safety.** Rust throughout. `unsafe` is denied workspace-wide. Platform FFI is the
  only exception, and each use carries a `// SAFETY:` justification.
- **Injection.** Database access uses compile-time checked `sqlx::query!` macros. Runtime SQL is
  confined to an allowlist of administrative and bootstrap paths (`scripts/check-sqlx.sh`).
- **Authorisation by construction.** HTTP routes cannot be registered without an authorisation
  policy; omitting one is a compile error.
- **Fail closed.** A policy evaluated over an incomplete inventory withholds access rather than
  defaulting to permissive (`just lint-fail-open`).
- **Errors.** Typed error enums. A 5xx response body is a fixed message; its cause is logged,
  never returned to the caller.
- **Secrets.** No secrets in source. Configuration reads secrets from a profile-referenced file
  or secrets source. Environment-variable reads are restricted to an audited allowlist
  (`scripts/env-var-allowlist.txt`).
- **Outbound requests.** Every outbound request is checked against SSRF rules when the URL is
  parsed and again when the connection is made
  ([Outbound egress controls](outbound-egress-controls.md)).

## 4. Security requirements and design

New surfaces are assessed against the [threat model](threat-model.md), which is updated in the
same change that introduces the surface. Public behaviour that customers depend on is covered
by the [stability contract](stability-contract.md).

## 5. Database changes

A migration file is immutable once it reaches `next`; a correction is a new, later migration.
Migrations are linted against large-table rules (`just lint-migrations`), and the schema
upgrade ladder proves upgrades from released versions on every push.

## 6. Review and segregation of duties

The project is maintained by its author. Segregation of duties is provided by automation rather
than by a second human reviewer:

- no person can change `main` except through a pull request whose required checks pass on the
  exact commit being merged;
- release artefacts are built only by CI from a tagged commit, never on a workstation, and carry
  a provenance attestation that binds them to the workflow and commit
  ([Supply chain](supply-chain.md));
- every change is public and permanently recorded in the git history.

Customers who require independent human review can request an independent code review (see
[Security review reference § 0](rfi-readiness-audit.md#0-assessment-scope)).

## 7. Environments

There is no vendor-operated production environment for self-hosted deployments. Development
and CI use disposable databases created per test shard. Customer environments are separated by
configuration profiles ([Configuration](../reference/configuration.md)).
