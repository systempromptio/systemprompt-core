# S4. Promote the trust core into core

**Goal.** Two tiers. Everything that carries trust leaves the template for core; the template keeps only the product surface.

**Entry.** S3 closed.

**Specification.** `../architecture/02-target-architecture.md` section 3, `../architecture/06-harness-repo-disposition.md`.

## What moves

| From the template | To core | Becomes the slice |
|---|---|---|
| `extensions/harness/engine/*` | Engine crates | `engine` |
| `sp-governance` | | `governor` |
| `sp-projection` | | part of `device` |
| `sp-accounts`, `sp-client`, `sp-secure`, `sp-sync`, `sp-protocol` | Device crates | `device` |
| `sp-drive` | | `drive` |
| `harness-composition` | The device composition | — |
| Their test crates | Core's test workspace | — |
| The scripted instance | Core's test workspace, exported for the template's use | — |

`sp-accounts`, `sp-client`, `sp-secure` and `sp-sync` duplicate bridge code. They are promoted as they are, so that this stage stays a relocation; S5 replaces them with the bridge's implementation. Two device-client implementations already exist today in two repositories; this stage does not add one.

What stays in the template: `shared`, `shell`, `brand`, the golden scenarios, the parity job, the migrated view tests, the tour, packaging.

## Wave A — design, one lane

| Lane | Repository | Work |
|---|---|---|
| **S4-A1 Crate list and layers** | core (documents only) | Fix the device crate list and each crate's layer. Prefer merging into an existing crate where one owns the concern (`identifiers`, `traits`, `client`, `models`). Extend the layer gate's table; specify the closure gate (server kernel names no device crate; device crates name no server crate above the shared layer). Scope by path the gates that cannot apply to device crates |

Output is an updated `architecture/02-target-architecture.md` section 3. It is a decision reserved to people before wave B opens.

## Wave B — three lanes, parallel, bottom-up

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-B1 Leaves** | core | `crates/shared/identifiers/**`, `crates/shared/traits/**`, the new engine leaf crates | Typed ids into `identifiers`; owned-task type deleted for the existing one; configuration, store and the model transport on core's wire codecs |
| **S4-B2 Engine** | core | The new engine crates | Tools, MCP, LSP, workspace, the loop with prompts merged in |
| **S4-B3 Device** | core | The new device crates | Protocol types into the bridge wire module where they are not already there; client, accounts, sealing, sync, projection, governor, Drive, the composition |

Moves and edits in separate commits. Each lane's first commit relocates; later commits adapt manifests and paths.

**Wave gate (L1).** Offline check of core's root workspace; layer and closure gates.

## Wave C — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-C1 Tests** | core | Core's test workspace paths for the moved crates | Relocate the test crates and the scripted instance. Device tests that run natively stay database-free and join the native list |
| **S4-C2 Facade and gates** | core | The facade crate; gate scripts and lists | Slices as facade features; `harness::reference()`; the device-only gates (no window toolkit in core, literals spelled once, platform confinement); core's workflow gains a scoped native job that runs only when a device crate changed |
| **S4-C3 Template onto core** | template | `extensions/harness/Cargo.toml`; `extensions/harness/{engine,device}` (deleted); shell imports | Delete the promoted crates from the template; depend on core's; licence of the workspace becomes MIT throughout |

**Wave gate (L1).** Offline check of both repositories with the template patched to local core.

## Stage gate (L2)

- Core: the full push run, including the scoped native job on Windows and macOS.
- Template: every S1, S2 and S3 check, unchanged, on three systems, now building the trust core from core.
- A crate-closure and size report for the reference composition, compared with the S1 baseline.

## Exit criteria

1. Stage gate green in both repositories at commits that reference each other.
2. The template contains no BUSL-1.1 crate.
3. Core contains no window-toolkit dependency.
4. The closure gate holds with an empty exception list.

## Decisions reserved to people

- The crate list (wave A).
- The licence change of the shell crates' remaining files, if any still carry the old header.
- Merging into `future`; whether device crates are published with the next core release or held back until S5.
