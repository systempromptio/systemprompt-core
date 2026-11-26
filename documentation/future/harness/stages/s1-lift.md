# S1. Lift the harness repository into core and the template

**Goal.** The harness repository is dissolved: its engine and governance are core crates, its shell is in the template, both build and pass their tests on Windows, macOS and Linux against core `future`, and the repository is archived.

**Entry.** S0 closed.

**Principle.** This stage relocates and adapts. It merges the eleven engine crates into one because nothing should move twice, but it does not redesign: the code keeps its `Governor` contract and its fixed composition. Restructuring starts in S3, after S2 has built the tests that make restructuring safe.

## Where everything lands

| From the harness repository | To | Licence |
|---|---|---|
| `harness-config`, `-store`, `-llm`, `-tools`, `-mcp`, `-workspace`, `-lsp`, `-prompt`, `-engine` | Modules of core `systemprompt-engine`: `config`, `store`, `llm`, `tools`, `mcp`, `workspace`, `lsp`, `prompt`, `run` | BUSL-1.1 |
| `harness-ids` | `systemprompt-identifiers` | BUSL-1.1 |
| `harness-tasks` | Deleted; callers use the owned-task types in `systemprompt-traits` | — |
| `sp-protocol` | Wire types into `systemprompt-models::bridge` where not already there; the rest into `systemprompt-device`; view types to the template | BUSL-1.1 |
| `sp-secure`, `sp-client`, `sp-accounts`, `sp-sync` | Modules of core `systemprompt-device` | BUSL-1.1 |
| `sp-governance`, `sp-projection` | Modules of core `systemprompt-harness` | BUSL-1.1 |
| `sp-drive` | Core `systemprompt-drive` | BUSL-1.1 |
| `harness-ui` | Template `extensions/harness/shared`, `extensions/harness/theme` | MIT |
| `harness-desktop` | Template `extensions/harness/shell`, `extensions/harness/views` | MIT |
| `sp-brand` | Template `extensions/harness/brand` | MIT |
| Test crates | Core's test workspace for the core crates; `extensions/harness/tests` for the shell | As their crate |
| Packaging | Template `packaging/harness/` | MIT |
| The 29 gates | Core's or the template's existing gates, per the lift manifest; the remainder dropped | — |

`systemprompt-device` receives the harness's own sign-in, sealing, client and sync code here. S4 replaces it with the bridge's; until then the bridge is untouched.

## Wave A — two import lanes, parallel (one per repository)

Each is serial inside its repository because it creates every path later lanes own.

| Lane | Repository | Work |
|---|---|---|
| **S1-A1 Core import** | core | Copy the engine, device, governance and Drive sources to the module paths the lift manifest names. One commit, moves only: file contents byte-identical to the source at its pinned commit, which the commit message records. No manifest wiring, so nothing compiles yet and nothing breaks |
| **S1-A2 Template import** | template | Copy the UI, desktop and brand crates, their tests and packaging to their paths. One commit, moves only. Then a second commit listing the workspace members |

**Wave gate (L1).** Core's root workspace still resolves and checks (the imported files are not yet modules). The template's shell workspace resolves.

## Wave B — six lanes, parallel

Four in core, each owning module directories of the new crates; two in the template. `lib.rs` of each new crate and the root manifest belong to the integrator.

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S1-B1 Engine leaves** | core | `systemprompt-engine/src/{config,store,llm}/**`; `crates/shared/identifiers/**` additions | Crate boundaries become module boundaries: paths, visibility per the manifest. Typed ids into `identifiers`. Owned-task type replaced. Model transport compiles as is; moving it onto core's wire codecs is S3 |
| **S1-B2 Engine tools and protocols** | core | `systemprompt-engine/src/{tools,mcp,lsp,workspace}/**` | The same for these modules |
| **S1-B3 Engine loop** | core | `systemprompt-engine/src/{prompt,run}/**` | The same; `NullGovernor` moves to the test workspace |
| **S1-B4 Device, governance, Drive** | core | `systemprompt-device/src/**`; `systemprompt-harness/src/**`; `systemprompt-drive/src/**`; `crates/shared/models/src/bridge/**` additions | The same. Wire types that duplicate `models::bridge` are deleted for it; the string host id becomes `HostKind::Harness` |
| **S1-B5 Shell onto core** | template | `extensions/harness/{shell,views,shared,theme}/**/src/**` | Imports change from the old crate names to core's crates and modules. No behaviour change |
| **S1-B6 Brand and packaging** | template | `extensions/harness/brand/**`; `extensions/harness/theme/src/palettes/**`; `packaging/harness/**` | One brand in the template. The second brand and its palette are exported as a patch for the install that owns it. Packaging reads the single table |

Six lanes exceed the cap: B5 and B6 open as core lanes hand off.

**Module dependency order inside `systemprompt-engine`**, carried over from the old ring order and enforced by a module-layer gate added in wave C: `config`, `store`, `llm`, `tools`, `mcp`, `workspace`, `lsp`, `prompt`, `run`. A module may use only those before it.

**Wave gate (L1).** Offline `cargo check` of core's root workspace on Linux. The template is not checked here: it cannot compile until wave C points it at local core.

## Wave C — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S1-C1 Core tests** | core | Core's test workspace paths for the four crates | Relocate the engine, device, governance and Drive test crates; imports follow the new module paths. Tests that run natively stay database-free and join the native list |
| **S1-C2 Core gates and workflow** | core | Gate scripts and lists; core's workflows | The harness gates the manifest marks *merge*; the module-layer gate for `systemprompt-engine`; a scoped native job that runs the device and engine tests on Windows and macOS only when those crates changed; facade features for the four crates |
| **S1-C3 Template manifests and gates** | template | `extensions/harness/**/Cargo.toml`; configuration files; `scripts/harness/**`; the recipe file | Depend on core's crates through the patch mechanism, inert in the committed manifest. Toolchain and lint table aligned with the template's. Advisory and licence exceptions for the window toolkit recorded here with reasons, not in core |
| **S1-C4 Template workflow** | template | The `harness` workflow | Windows: check, shell tests, release build, installer packed, silent test-install. macOS: check, shell tests, release build. Linux: formatting, gates, supply-chain checks. Each resolves core from `future`. No artefacts stored |

**Wave gate (L1).** Offline check of core's root and test workspaces on Linux; the kept gates; formatting. The shell is proven at L2.

## Wave D — one consolidated fix round

The integrator pushes both repositories once and reads the first three-system runs. Failures are sorted by owning lane and sent as **one** message per lane to fresh lanes. Expected classes:

| Class | Owner |
|---|---|
| Windows-only compile errors, unseen until a Windows runner | The module's lane |
| Stub and real implementation out of step | The module's lane |
| Visibility that a crate boundary used to enforce and a module boundary does not | S1-B1 to B3 paths |
| Tests that assumed the old layout | S1-C1 |
| Lint differences from core's lint table and gates | The module's lane |

The imported code must meet core's standards and every core gate. Where it does not, the remediation is this wave's work, not a relaxation.

A second fix round is allowed. A third means a defect class was misjudged: stop and write it up.

## Stage gate (L2)

| Where | Must pass |
|---|---|
| Core push run | Lints on all targets; the full test workspace including the relocated tests; documentation; supply chain; the scoped native job on Windows and macOS |
| Template, Windows | Check; shell tests; release build; installer packed; silent test-install; installed binary reports its version |
| Template, macOS | Check; shell tests; release build |
| Template, Linux | Formatting; gates; supply-chain checks |
| Template's existing workflows | Unchanged and green |

The combined test count equals the harness repository's at the pinned commit (942), less any test the manifest retired with a reason. A lower count without a reason fails the gate.

## Exit criteria

1. The stage gate is green in both repositories at commits that reference each other.
2. Core contains no window-toolkit dependency; the closure gate holds with an empty exception list.
3. The template contains no engine or governance code.
4. The second brand has left the template and its patch has been handed to the install that owns it.
5. The harness repository's `README` points at core and the template, and the repository is archived.

## Decisions reserved to people

- Archiving the harness repository.
- Relicensing the shell, UI and brand crates from BUSL-1.1 to MIT as they enter the template.
- Merging the programme branch into `future`.

## What this stage deliberately leaves alone

- The fixed composition, the untyped extensions map, the built-in tool list. S3.
- The duplication between `systemprompt-device` and the bridge. S4.
- macOS and Linux packaging, signing, Linux desktop. S5.
