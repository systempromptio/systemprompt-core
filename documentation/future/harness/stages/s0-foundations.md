# S0. Foundations

**Goal.** Everything S1 and S2 need exists, and nothing has been moved yet.

**Entry.** None.

## Wave A — five lanes, all parallel

| Lane | Repository | Owned paths | Work | Hand-off |
|---|---|---|---|---|
| **S0-A1 Host** | core | `crates/shared/models/src/bridge/**`; `crates/shared/manifest/src/services/selection.rs`; the gateway bridge routes (`manifest`, `profile`, `data`); marketplace consumer authorization; the well-known bridge document; `crates/app/agent-runs/src/host/mod.rs`; the tests of those crates | Add `HostKind::Harness` (wire id `harness`), `HostProduct::Harness`, kind `DesktopApp`. Add `HostKind::config_owner() -> ConfigOwner { Bridge, Host }`; `harness` is `Host`. Follow every exhaustive match. The bridge's profile-writing and loopback-credential paths filter on `config_owner`, not on a name. Both gateway protocols by default. `agent-runs` answers `Unsupported` for `Harness` until S9 | Every match site touched; any site that was a wildcard arm |
| **S0-A2 Crate skeletons** | core | The eight new crate directories (`../architecture/02-target-architecture.md` section 3), each with a manifest, an empty `lib.rs` with its module documentation, and nothing else; the layer gate's table; a new closure gate | Create `systemprompt-engine`, `-device`, `-harness`, `-drive`, `-proxy`, `-machine`, `-hosts`, `-runner` at their layers. Extend the layer gate. Add the closure gate: server kernel crates name no device crate; device crates name no server crate above the shared layer. Scope by path the gates that cannot apply to device crates, each with its reason | The lines the integrator adds to the root manifest, the facade and the publish list |
| **S0-A3 Template scaffold** | template | `extensions/harness/` (an empty cargo workspace: manifest, toolchain, formatter, lint and supply-chain configuration); `flavours/harness/**`; a new `harness` workflow; `scripts/harness/**`; a recipe file for harness recipes | The shell workspace as a **separate workspace** inside the template with its own lockfile. The flavour entry with `artefact: desktop`. A workflow with Windows, macOS and Linux jobs that for now only check out and print the toolchain; it stores no artefacts. Extend the core-patch script to cover the shell workspace | The one-line imports for the recipe file and the flavour index |
| **S0-A4 Lift manifest** (read-only) | harness repository, template, core `future` | None. Produces a working file for the integrator | For every file in the harness repository: its destination (`../architecture/06-harness-repo-disposition.md`). For each of the eleven engine crates: its module name inside `systemprompt-engine`, the order modules may depend on each other, and every item whose visibility must change when a crate boundary becomes a module boundary. Differences to reconcile: toolchain; lint table; each dependency's version against core's and the template's; each of the 29 gates against core's and the template's (already covered, merge, drop); every use of the linked core crates that `future` has changed | The manifest; expected compile breaks; gate collisions |
| **S0-A5 Reference-test catalogue** (read-only) | the reference agent's public repository at the latest stable tag, which this lane pins | None. Produces the catalogue (`../testing/04-test-migration.md`) | Enumerate every test file. For each: area, one-sentence behaviour statement, kind, and a first classification (port, already covered, not applicable, defer). Record the pinned tag and the licence | The catalogue; counts per area; the cases that need the scripted instance |

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

## Stage gate (L2)

Run once.

- Core: the push run on the programme branch.
- Template: the `harness` workflow green on all three runners (it does almost nothing yet; this proves the workflow itself).

## Exit criteria

1. `HostKind::Harness` is on core `next` and `future` with green push runs and a closed review.
2. The eight crates exist in core, empty, inside the layer and closure gates.
3. The template has an empty shell workspace, a flavour entry and a three-system workflow.
4. The lift manifest and the reference-test catalogue exist and have been read by the integrator.
5. The scripted-instance crate compiles with its interface fixed.

## Decisions reserved to people

- Merging the programme branch into `future`, and S0-A1 into `next`.
