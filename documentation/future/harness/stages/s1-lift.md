# S1. Lift the harness repository into core and the template

**Goal.** The harness repository is dissolved: its engine and governance are core crates, its shell is in the template, both build and pass their tests on Windows, macOS and Linux against core `future`, and the repository is archived.

**Entry.** S0 closed.

**Pin.** The harness repository at `a214511`. Every count and placement below is true at that commit; the lift manifest (lane S0-A4) and its delta (lane S0-C2) hold the file-level detail.

**Principle.** This stage relocates and adapts. It merges the eleven engine crates into one because nothing should move twice, but it does not redesign. The code keeps its `Governor` contract. Its composition is not fixed: a three-ring extension contract (engine, instance side, shell) carries governance, Drive, Brain, notifications and Slack as members. S1 keeps that contract, its resolution by priority, its enablement from the signed manifest's `host_extensions` entry and the `Services::start` wiring. S1 changes one thing in it: link-time registration through `inventory` becomes an explicit named registration list, because core's `lint-no-inventory` stays as it is. S5 replaces the whole contract with the device composition (`s5-device-composition.md`), after S2 has built the tests that make restructuring safe.

## Where everything lands

| From the harness repository | To | Licence |
|---|---|---|
| `harness-config`, `-store`, `-llm`, `-tools`, `-mcp`, `-workspace`, `-lsp`, `-prompt`, `-engine` | Modules of core `systemprompt-engine`: `config`, `store`, `llm`, `tools`, `mcp`, `workspace`, `lsp`, `prompt`, `run` | BUSL-1.1 |
| `harness-ids` | `systemprompt-identifiers`, submodule `engine` (`crates/shared/identifiers/src/engine/`): four of its names already exist there with other validation | BUSL-1.1 |
| `harness-tasks` | Deleted; callers use the owned-task types in `systemprompt-traits`, which gains what they need and it lacks (blocking spawns, `OwnedTask::stop`, a child token, a shutdown report, a task error) | — |
| `sp-protocol` | Wire types into `systemprompt-models` (`bridge`, `documents`, `brain`, `notifications`) where not already there; the rest, view types included, into `systemprompt-device` (`protocol/`, `view.rs`) | BUSL-1.1 |
| `sp-secure`, `sp-client`, `sp-accounts` | Modules of core `systemprompt-device`: `secure`, `client`, `accounts` | BUSL-1.1 |
| `sp-sync` | Module `sync` of core `systemprompt-harness`: it depends on governance, projection and the extension contract, which `systemprompt-device` may not name | BUSL-1.1 |
| `sp-governance`, `sp-projection` | Modules of core `systemprompt-harness`: `governance`, `projection` | BUSL-1.1 |
| `sp-extension` | Module `extension` of core `systemprompt-harness`, with the named registration list in place of discovery | BUSL-1.1 |
| `sp-drive`, sync half (`client`, `http`, `sync`, `error.rs`, `hash.rs`, `upload.rs`, `lib.rs`) | Core `systemprompt-drive` | BUSL-1.1 |
| `sp-drive`, engine half (`extension/**`, `agent.rs`) | Core `systemprompt-harness`, `src/members/drive/`: it depends on the engine, which a domain crate may not | BUSL-1.1 |
| `sp-drive` `view.rs` | Template `extensions/harness/views` | MIT |
| `sp-brain`, `sp-notifications`, `sp-slack` | Core `systemprompt-harness`, `src/members/{brain,notifications,slack}/` | BUSL-1.1 |
| `harness-ui` | Template `extensions/harness/shared`, `extensions/harness/theme` | MIT |
| `harness-shell` | Template `extensions/harness/shared/src/contract/`, with a named registration list in place of discovery | MIT |
| `harness-drive-ui`, `harness-brain-ui`, `harness-notifications-ui` | Template `extensions/harness/views/src/{drive,brain,notifications}/` | MIT |
| `harness-desktop` | Template `extensions/harness/shell`, `extensions/harness/views`; its strings to `extensions/harness/shared` | MIT |
| `sp-brand` | Template `extensions/harness/brand` | MIT |
| Test crates (30) | Core's test workspace for the 23 that test core-bound crates; `extensions/harness/tests` for the 7 that test the shell | As their crate |
| `harness-live-tests` | Not lifted. It signs in to a real instance named by environment variables and one of its 8 tests makes a real model call. S2 re-expresses its behaviours over the scripted instance | — |
| Packaging (`packaging/windows`, `packaging/macos`) | Template `packaging/harness/{windows,macos}/` | MIT |
| The 29 gates | Core's or the template's existing gates, per the lift manifest; the remainder dropped | — |

`systemprompt-device` receives the harness's own sign-in, sealing and client code here, and `systemprompt-harness` its sync. S6 replaces them with the bridge's; until then the bridge is untouched.

Template crate order: `brand`, `theme`, `shared`, `views`, `shell`. `shared` depends on core's engine and device crates, because the shell contract's context names them.

`rusqlite` enters core's root workspace pinned to the release whose `libsqlite3-sys` equals the one the root lockfile holds (`rusqlite` 0.39 for `libsqlite3-sys` 0.37).

## Wave A — two import lanes, parallel (one per repository)

Each is serial inside its repository because it creates every path later lanes own.

| Lane | Repository | Work |
|---|---|---|
| **S1-A1 Core import** | core | Copy the engine, device, governance, extension, member and Drive sources to the module paths the lift manifest names. One commit, moves only: file contents byte-identical to the source at `a214511`, which the commit message records. No manifest wiring, so nothing compiles yet and nothing breaks. `harness-tasks`, the crate manifests and `harness-live-tests` are not copied |
| **S1-A2 Template import** | template | Copy the UI, shell-contract, feature-screen, desktop and brand crates, their tests and both packaging directories to their paths. One commit, moves only. Then a second commit listing the workspace members |

**Wave gate (L1).** Core's root workspace still resolves and checks (the imported files are not yet modules). The template's shell workspace resolves.

## Wave B — six lanes, parallel

Four in core, each owning module directories of the new crates; two in the template. `lib.rs` of each new crate and the root manifest belong to the integrator. Before the wave opens the integrator lands the owned-task additions in `crates/shared/traits/src/background_tasks.rs`, in one commit: B2, B3 and B4 all compile against them and none of them owns that file.

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S1-B1 Engine leaves** | core | `systemprompt-engine/src/{config,store,llm}/**`; `crates/shared/identifiers/src/engine/**` and its one `mod` line | Crate boundaries become module boundaries: paths, visibility per the manifest. Typed ids into the `engine` submodule of `identifiers`. Model transport compiles as is; moving it onto core's wire codecs is S5 |
| **S1-B2 Engine tools and protocols** | core | `systemprompt-engine/src/{tools,mcp,lsp,workspace}/**` | The same for these modules |
| **S1-B3 Engine loop** | core | `systemprompt-engine/src/{prompt,run}/**` | The same; `NullGovernor` moves to the test workspace |
| **S1-B4 Device, governance, members, Drive** | core | `systemprompt-device/src/{secure,client,accounts,protocol}/**`, `view.rs`; `systemprompt-harness/src/{projection,extension,governance,sync}/**` and `src/members/{drive,brain,notifications,slack}/**`; `systemprompt-drive/src/**`; additions under `crates/shared/models/src/{bridge,documents,brain,notifications}/**` | The same. Module order inside `systemprompt-harness`: `projection`, `extension`, `governance`, `members/*`, `sync`. The named registration list replaces `inventory` in `extension` and lists the five members. Wire types that duplicate `systemprompt-models` are deleted for it; the string host id becomes `HostKind::Harness`; the manifest's `host_extensions` is read from core's type |
| **S1-B5 Shell onto core** | template | `extensions/harness/{shell,views,shared,theme}/**/src/**`, which includes `shared/src/contract/**` and `views/src/{drive,brain,notifications}/**` | Imports change from the old crate names to core's crates and modules. The shell half's named registration list replaces `inventory` and pairs by extension id with core's. No behaviour change |
| **S1-B6 Brand and packaging** | template | `extensions/harness/brand/**`; `extensions/harness/theme/src/palettes/**`; `packaging/harness/{windows,macos}/**` | One brand in the template. The second brand and its palette are exported as a patch for the install that owns it. Both packagers read the single table |

Six lanes exceed the cap: B5 and B6 open as core lanes hand off.

**Module dependency order inside `systemprompt-engine`**, carried over from the old ring order and enforced by a module-layer gate added in wave C: `config`, `store`, `llm`, `tools`, `mcp`, `workspace`, `lsp`, `prompt`, `run`. A module may use only those before it.

**Wave gate (L1).** Offline `cargo check` of core's root workspace on Linux. The template is not checked here: it cannot compile until wave C points it at local core.

## Wave C — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S1-C1 Core tests** | core | Core's test workspace paths for the four crates | Relocate the 23 engine, device, governance, extension, member and Drive test crates (877 test attributes); imports follow the new module paths. Tests that run natively stay database-free and join the native list. `harness-tasks-tests` is ported onto the `systemprompt-traits` additions or retired with a reason. `harness-live-tests` is not relocated |
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

The combined test count equals the harness repository's at the pinned commit, less any test retired with a reason. At `a214511` the repository holds 1,154 test attributes (`#[test]` and `#[tokio::test]`, a static count). The 8 in `harness-live-tests` are retired here: they need a real instance and return at once without one, and S2 replaces them with scripted-instance tests. The figure to match is therefore 1,146 (877 in core, 269 in the template), less attributes a runner's platform excludes and less any further test the manifest or its delta retires with a reason. A lower count without a reason fails the gate.

No job in either repository sets a variable that names a live instance or a model, and no S1 test reaches a model provider.

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

- The three-ring extension contract, its priority shadowing, the typed handles map, `Services::start`, the built-in tool list. S5.
- The engine extension contract inside `systemprompt-engine` at `run/extension`. S5.
- The duplication between `systemprompt-device` and the bridge. S6.
- macOS and Linux packaging beyond the imported bundle script, signing, Linux desktop. S7.
