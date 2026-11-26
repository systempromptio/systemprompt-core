# S1. Lift the harness repository into the template

**Goal.** The harness repository's code lives in the template, builds and passes its tests on Windows and macOS against core `future`, and is packaged. The harness repository is archived.

**Entry.** S0 wave A closed (host in core; scaffold in the template; lift manifest written).

**Principle.** This stage relocates and adapts. It does not redesign. The code keeps its three rings, its `Governor` contract and its fixed composition; restructuring starts in S3, after S2 has built the tests that make restructuring safe.

## Target layout after this stage

```
extensions/harness/               its own cargo workspace and lockfile
  engine/    harness-ids, -tasks, -config, -store, -llm, -tools, -mcp,
             -workspace, -lsp, -prompt, -engine              (BUSL-1.1)
  device/    sp-protocol, -secure, -client, -projection, -drive,
             -accounts, -governance, -sync                    (BUSL-1.1)
  shared/    harness-ui                                       (MIT)
  shell/     harness-desktop                                  (MIT)
  brand/     sp-brand                                         (MIT)
  tests/     its own test workspace: one test crate per crate
packaging/harness/                windows (NSIS), later macos and linux
scripts/harness/                  the gates that came with it
```

The `engine/` and `device/` crates are the ones S4 promotes into core. They keep a BUSL-1.1 licence field while they sit in the template; `shared/`, `shell/` and `brand/` are MIT from the import.

The ring rule carries over as paths: `shell`, `shared`, `brand` may depend on `device` and `engine`; `device` on `engine`; `engine` on neither.

## Wave A — one lane, serial

| Lane | Owned paths | Work |
|---|---|---|
| **S1-A1 Import** | Everything under `extensions/harness/`, `packaging/harness/`, `scripts/harness/` | Copy every file to the destination the lift manifest names. One commit, moves only, no edits: crate contents are byte-identical to the source at its pinned commit, which the commit message records. Then a second commit: the workspace `Cargo.toml` listing members at their new paths, and nothing else |

This lane is serial because it creates every path the next wave's lanes own.

**Wave gate (L1).** `cargo metadata` resolves for the harness workspace. No compile yet.

## Wave B — five lanes, parallel

Disjoint by construction: manifests, scripts, workflow and packaging, device sources, brand and palettes.

| Lane | Owned paths | Work | Inputs |
|---|---|---|---|
| **S1-B1 Manifests** | `extensions/harness/Cargo.toml`; every crate's `Cargo.toml`; `extensions/harness/{rust-toolchain.toml,rustfmt.toml,clippy.toml,deny.toml}`; `extensions/harness/tests/Cargo.toml` | Align toolchain, edition, `rust-version` and the lint table with the template's. Reconcile shared dependency versions to the template's where the manifest says they differ. Set the licence field per crate. Point the two core crates at `future` through the core-patch mechanism, inert in the committed manifest. Merge the harness's advisory and licence exceptions into the workspace's own `deny.toml`, each with its reason | Lift manifest: toolchain, lints, dependency table |
| **S1-B2 Gates** | `scripts/harness/**`; `just/harness.just` | For each of the 29 gates, apply the manifest's verdict: keep (repoint at the new paths), merge into an existing template gate, or drop. Rewrite the ring gate for the new directory names. Recipes: `harness-check`, `harness-gates`, `harness-test`, `harness-package` | Lift manifest: gate table |
| **S1-B3 Workflow and packaging** | `.github/workflows/harness.yml`; `packaging/harness/**` | Windows: check, test workspace, release build, pack the installer, silent test-install. macOS: check, test workspace, release build. Linux: formatting, source gates, supply-chain checks, and `cargo check` of the `engine` and `device` crates only. Aggregate job named for required-check use. Caching keyed on the harness lockfile. Artefacts uploaded only on manual runs | The harness repository's workflows |
| **S1-B4 API drift** | `extensions/harness/device/**/src/**`; `extensions/harness/engine/**/src/**` | Fix every compile break against core `future` listed in the manifest. Replace the string host id with `HostKind::Harness`. No behaviour change beyond what the type change forces; each forced change listed in the hand-off | Lift manifest: expected breaks |
| **S1-B5 Brand** | `extensions/harness/brand/**`; `extensions/harness/shared/src/theme/palettes/**`; `packaging/harness/windows/**` brand table | Reduce the template's copy to its own single brand and channels. Export the second brand and its palette as a patch for the install that owns it; it is not carried in the template. Packaging reads the same single table | `architecture/06-harness-repo-disposition.md` |

S1-B3 and S1-B5 both list `packaging/harness/windows/**`: S1-B5 owns only the brand table file; S1-B3 owns the rest. The integrator names the exact file split in the briefs.

**Wave gate (L1).** On Linux: offline `cargo check` of the `engine` and `device` crates and their test crates; the kept gates; formatting. The shell crates are not compiled here: they need a desktop system and are proven at L2.

## Wave C — one consolidated fix round

The integrator pushes once and reads the first three-system run. Failures are sorted by owning lane from wave B and sent as **one** message per lane to fresh lanes on the same branches. Expected classes:

| Class | Owner |
|---|---|
| Windows-only compile errors in `cfg(windows)` code, unseen until a Windows runner | S1-B4 paths |
| Stub and real implementation out of step; unused findings on one side | S1-B4 paths |
| Path or toolchain assumptions in gates and packaging | S1-B2, S1-B3 paths |
| Tests that assumed the old repository layout | Test crate owners per the manifest |
| Lint differences from the reconciled lint table | The crate's lane |

A second fix round is allowed. A third means a defect class was misjudged: stop and write it up.

## Stage gate (L2)

Run on the hosted runners.

| Job | Must pass |
|---|---|
| Windows | Check; the full harness test workspace; release build; installer packed; silent test-install succeeds; installed binary reports its version |
| macOS | Check; the full harness test workspace; release build |
| Linux | Formatting; every kept gate; supply-chain checks; `engine` and `device` check and tests |
| Template's existing workflows | Unchanged and green: the server flavours are not affected by a separate workspace |

The test count on Windows and macOS equals the count in the harness repository at the pinned commit, less any test the manifest retired with a reason. A lower count without a reason fails the gate.

## Exit criteria

1. The stage gate is green on all three runners at one commit.
2. The harness workspace builds against core `future`, not a released version.
3. The second brand has left the template and its patch has been handed to the install that owns it.
4. The template's `README` and flavour page say what the harness flavour is and that it is not yet a supported product.
5. The harness repository's default-branch `README` points at the template, and the repository is archived.

## Decisions reserved to people

- Archiving the harness repository (exit criterion 5).
- The licence fields in this stage's layout: BUSL-1.1 crates inside an MIT repository until S4.
- Whether to preserve the harness repository's history in the template. Recommended: no; a plain import that records the source commit, with the history left readable in the archived repository.

## What this stage deliberately leaves alone

- The fixed composition (`Services::start`), the untyped extensions map, the `builtin()` tool list. S3.
- The duplication between `device/` and the bridge. S5.
- macOS and Linux packaging, signing. S6.
- Any view work. S2 adds only what the screenshot tour needs.
