# S0. Foundations

**Goal.** Everything S1 and S2 need exists, and nothing has been moved yet.

**Entry.** None.

## Wave A — five lanes, all parallel

| Lane | Repository | Owned paths | Work | Hand-off |
|---|---|---|---|---|
| **S0-A1 Host** | core | `crates/shared/models/src/bridge/**`; `crates/shared/manifest/src/services/selection.rs`; the gateway bridge routes (`manifest`, `profile`, `data`); marketplace consumer authorization; the well-known bridge document; `crates/app/agent-runs/src/host/mod.rs`; the tests of those crates | Add `HostKind::Harness` (wire id `harness`), `HostProduct::Harness`, kind `DesktopApp`. Add `HostKind::config_owner() -> ConfigOwner { Bridge, Host }`; `harness` is `Host`. Follow every exhaustive match. The bridge's profile-writing and loopback-credential paths filter on `config_owner`, not on a name. Both gateway protocols by default. `agent-runs` answers `Unsupported` for `Harness` until S9. The exhaustive matches reach beyond the owned paths (the bridge, `models/{origin,feedback,mcp}`, two schema migrations, the marketplace installation plan); those edits are forced and are listed in the hand-off | Every match site touched; any site that was a wildcard arm |
| **S0-A2 Crate skeletons** | core | The eight new crate directories (`../architecture/02-target-architecture.md` section 3), each with a manifest, an empty `lib.rs` with its module documentation, and nothing else; the layer gate's table; a new closure gate | Create `systemprompt-engine`, `-device`, `-harness`, `-drive`, `-proxy`, `-machine`, `-hosts`, `-runner` at their layers. Extend the layer gate. Add the closure gate: server kernel crates name no device crate; device crates name no server crate above the shared layer. No existing gate fires on an empty crate, so none is scoped here: the list of gates that assume a server crate, and the rule S1 wave C applies to them, is `../architecture/09-quality-and-gates.md` section 1.1 | The lines the integrator adds to the root manifest, the unit table, the recipe file and the ignore file. The facade and the publish list need nothing: no gate requires a feature for an empty crate, and publishing follows workspace membership |
| **S0-A3 Template scaffold** | template | `extensions/harness/` (an empty cargo workspace: a manifest and a lockfile; toolchain, formatter, lint and supply-chain configuration are found by each tool walking up to the template's root, as the template's test workspace does, so none is copied); `flavours/harness/**`; a new `harness` workflow; `scripts/harness/**`; a recipe file for harness recipes | The shell workspace as a **separate workspace** inside the template with its own lockfile. The flavour entry with `artefact: desktop`. A workflow with Windows, macOS and Linux jobs that for now only check out and print the toolchain; it stores no artefacts. Extend the core-patch script to cover the shell workspace | The one-line imports for the recipe file and the flavour index; the root manifest's `exclude`; every script and gate that enumerates workspaces or lockfiles, split into what applies to an empty workspace now and what waits for its first member in S1 |
| **S0-A4 Lift manifest** (read-only) | harness repository, template, core `future` | None. Produces a working file for the integrator | For every file in the harness repository: its destination (`../architecture/06-harness-repo-disposition.md`). For each of the eleven engine crates: its module name inside `systemprompt-engine`, the order modules may depend on each other, and every item whose visibility must change when a crate boundary becomes a module boundary. Differences to reconcile: toolchain; lint table; each dependency's version against core's and the template's; each of the 29 gates against core's and the template's (already covered, merge, drop); every use of the linked core crates that `future` has changed | The manifest; expected compile breaks; gate collisions |
| **S0-A5 Reference-test catalogue** (read-only) | the reference agent's public repository at the latest stable tag, which this lane pins (`v2.4.2`, confirmed as D47) | None. Produces the catalogue (`../testing/04-test-migration.md`) | Enumerate every test file. For each: area, one-sentence behaviour statement, kind, and a first classification (port, already covered, not applicable, defer). One row per file, split where a file's cases fall into different classes. Record the pinned tag and the licence | The catalogue; counts per area; the cases that need the scripted instance |

Five lanes exceed the cap of four: S0-A5 opens when the first lane hands off.

**Wave gate (L1).**
- Core: offline `cargo check` of the crates S0-A1 touched and of the eight skeletons; the layer and closure gates.
- Template: the workflow file parses; the empty workspace resolves.
- The two inventories are reviewed by reading.

## Wave B — two lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S0-B1 Host review** **(review)** | core | None | Independent review of S0-A1 against `../architecture/04-host-integrations.md` section 3.1: no path where `harness` receives a bridge-written profile or a loopback credential; no wildcard arm that silently includes it; manifest targeting and protocol selection correct |
| **S0-B2 Scripted-instance interface** | core | A new scripted-instance crate in core's test workspace | The crate shell and its public interface only (`../testing/02-scripted-instance.md` section 2): types, the builder, an empty router, a binary entry point. No behaviour yet. This fixes the interface S2 lanes build against |

**Wave gate (L1).** Offline `cargo check` of the new test crate.

The review (S0-B1) did not close: three findings rated high. The specified `config_owner` was absent, replaced by a name list checked in one place; the bridge would mint and its proxy accept a scoped token for `harness`; and the host was on for every instance by default, which fails the manifest parse of every bridge released before the variant. A fix lane closed the first two in code. The third needed a decision.

## Wave C — three lanes, parallel

Opened on 2026-11-27 with the owner's decisions on what waves A and B raised (D42 to D47 in `../risks-and-decisions.md`).

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S0-C1 Host fix, round 2** | core | The host paths of S0-A1, the gateway bridge routes and well-known document, the host-preference repository, the bridge only where a finding names it, their unit tests, the changelog fragments and the upgrade guide | Close every remaining review finding. A host-owned host is opt-in per instance; enabling one raises the manifest's minimum bridge version; host-owned hosts leave the well-known document (D42). The gateway refuses `host-token` attestation for a host-owned client. `host_extensions` tightened to its specification (D44). The guessed mappings removed; the tests the review found missing written |
| **S0-C2 Lift re-pin** (inventory and specification) | core (documents); the harness repository, read from git objects | `../architecture/01-current-state.md`, `../architecture/06-harness-repo-disposition.md`, `s1-lift.md`; a manifest delta held with the integrator | Move the lift's pin to `a214511` (D45) and make every number and placement true at it, with the placements of D46 |
| **S0-C3 Specification catch-up** | core (documents) | The rest of this directory that S0's findings touch | Record the decisions; answer the review's ten specification ambiguities; publish the catalogue's counts and vocabulary; fix the scripted-instance interface as built; the runner's place in the server's crate graph; the gates that assume a server crate |

Exit criterion 1 asks for a closed review. S0-C1 is a fixing lane; closing the review is a reviewer's act (`operating-model.md` section 7), against `../architecture/04-host-integrations.md` section 3 as it now reads.

**Wave gate (L1).** Offline `cargo check` of the crates S0-C1 touched, in the root workspace and the bridge's; formatting; the cheap gates. The documents are reviewed by reading.

## Stage gate (L2)

Run once.

- Core: the push run on the programme branch. It is the first run of the bridge build, the bridge test crates and the new gates' self-tests over this stage's changes: the lanes wrote by reading.
- Template: the `harness` workflow green on all three runners (it does almost nothing yet; this proves the workflow itself).

## Exit criteria

1. `HostKind::Harness` is on core `next` and `future` with green push runs and a closed review: no finding rated high is open. `host_extensions` is on `future` only.
2. The eight crates exist in core, empty, inside the layer and closure gates.
3. The template has an empty shell workspace, a flavour entry and a three-system workflow.
4. The lift manifest (with its delta to the pin `a214511`) and the reference-test catalogue exist and have been read by the integrator, and the specification states their numbers.
5. The scripted-instance crate compiles with its interface fixed.

## Decisions reserved to people

- Merging the programme branch into `future`, and S0-A1 into `next`. On `next` the host-preference migration follows a number that exists only on `future`; whether `next` takes the gap is the integrator's call at that merge.
- Enabling host `harness` on any instance: it raises that instance's minimum bridge version (D42).