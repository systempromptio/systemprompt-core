# S0. Foundations

**Goal.** Everything S1 and S2 need exists, and nothing has been moved yet.

**Entry.** None.

**Why it is small.** Only one change to core is required before the harness can build against `future` and sign in to an instance: the host. Everything else in this stage is scaffolding and reading.

## Wave A — four lanes, all parallel

| Lane | Repository | Owned paths | Work | Hand-off |
|---|---|---|---|---|
| **S0-A1 Host** | core | `crates/shared/models/src/bridge/**`; `crates/shared/manifest/src/services/selection.rs`; the gateway bridge routes (`manifest`, `profile`, `data`); marketplace consumer authorization; the well-known bridge document; `crates/app/agent-runs/src/host/mod.rs`; the tests of those crates | Add `HostKind::Harness` (wire id `harness`), `HostProduct::Harness`, kind `DesktopApp`. Add `HostKind::config_owner() -> ConfigOwner { Bridge, Host }`; `harness` is `Host`. Follow every exhaustive match. The bridge's profile-writing and loopback-credential paths filter on `config_owner`, not on a name. Both gateway protocols by default. `agent-runs` answers `Unsupported` for `Harness` until S8 | Every match site touched, listed; any site that was a wildcard arm |
| **S0-A2 Template scaffold** | template | `extensions/harness/` (empty workspace root: `Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`); `flavours/harness/**`; `.github/workflows/harness.yml`; `scripts/harness/**`; `just/harness.just` | Create the harness cargo workspace as a **separate workspace** inside the template with its own lockfile. Add the flavour entry with `artefact: desktop`. Add a workflow with three jobs (`windows-latest`, `macos-latest`, `ubuntu-latest`) that for now only check out and print the toolchain. Extend the core-patch script to cover the harness workspace | The exact one-line import the integrator adds to `justfile` and to `flavours/index.yaml` |
| **S0-A3 Lift manifest** (read-only) | harness repository, template, core `future` | None. Produces a working file for the integrator | For every file in the harness repository: its destination path in the template (`architecture/06-harness-repo-disposition.md` sections 2 and 6, `architecture/07-template-extension.md` section 1). Differences to reconcile: toolchain and `rust-version`; lint table; each shared dependency's version against the template's; licence field per crate; each of the 29 gates against the template's existing gates (keep, merge, drop); every use of the linked core crates that `future` has changed | The manifest; a list of compile breaks to expect against `future`; a list of gate collisions |
| **S0-A4 Reference-test catalogue** (read-only) | the reference agent's public repository at a pinned tag | None. Produces the catalogue (`../testing/04-test-migration.md`) | Enumerate every test file. For each: area, one-sentence behaviour statement, kind (unit, integration, end-to-end, desktop main process, benchmark), and a first classification (port, already covered, not applicable, defer). Record the pinned tag and the licence | The catalogue; counts per area; the cases that need the scripted instance |

**Wave gate (L1).**
- Core: offline `cargo check` of the crates S0-A1 touched; the layer gate.
- Template: the workflow file parses; the empty workspace resolves.
- Nothing else. The two inventories are reviewed by reading.

## Wave B — two lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S0-B1 Host review** **(review)** | core | None | Independent review of S0-A1 against `architecture/04-host-integrations.md` section 3.1: no path where `harness` receives a bridge-written profile or a loopback credential; no wildcard arm that silently includes it; manifest targeting and protocol selection correct |
| **S0-B2 Scripted-instance skeleton** | template | `extensions/harness/tests/support/scripted-instance/**` | The crate shell and its public API only (`../testing/02-scripted-instance.md` section 2): types, the builder, an empty router. No behaviour yet. This fixes the interface S2 lanes build against |

**Wave gate (L1).** Offline `cargo check` of the new test crate.

## Stage gate (L2)

Run once.

- Core: the push run on the programme branch (lints, the test workspace, documentation, supply chain).
- Template: the `harness` workflow green on all three runners (it does almost nothing yet; this proves the workflow itself).

## Exit criteria

1. Core `future` (or a programme branch off it, see below) contains `HostKind::Harness` with a green push run and a closed review.
2. The template has an empty harness workspace, a flavour entry and a three-system workflow.
3. The lift manifest and the reference-test catalogue exist and have been read by the integrator.
4. The scripted-instance crate compiles with its interface fixed.

## Decisions reserved to people

- Whether S0-A1 lands on `next` as well as `future`. Recommended: yes; it is small, and it lets a released instance serve the host.
- Merging the programme branch into `future`.

## Optional, in parallel, by a person

Launch the harness repository's current Windows build once on a real machine and sign in to a local instance built from S0-A1. It is not on the critical path, because S1 proves the same on hosted runners and S2 proves behaviour deterministically, but the first human look at the running app is worth having early.
