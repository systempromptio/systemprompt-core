# S3. Device composition

**Goal.** The harness is a composition of members. Its `main` is one line; its features are slices; a member outside the kernel can add a tool, a view and a theme.

**Entry.** S1 and S2 closed. The S2 suites are the safety net: this stage changes how the harness is assembled and must not change what it does.

**Specification.** `../architecture/03-composition-contract.md`, `../architecture/02-target-architecture.md` sections 4 to 6.

## Wave A — contract, one lane then one review

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-A1 Neutral contract** | core | `crates/shared/extension/**`; if the fallback is taken, `crates/shared/provider-contracts/**` | Feature-split the composition contract: a default with no database, HTTP or command-line dependency, and `server` holding today's sink. Add the `device` feature: `HarnessExtension`, `HarnessContributions`, the device reserved shapes, `BrandDecl` and `ThemeDecl` as data, mount-point ids |
| **S3-A2 Contract review** **(review)** | core | None | Reserved shapes cannot be claimed by an extension-standing member; admission withholds, never partially admits; no server crate's public API changed |

Serial: every other lane in this stage compiles against S3-A1.

**Wave gate (L1).** Offline check of core; a probe crate depending on the contract with only `device` enabled links no database, HTTP-server or command-line crate.

**Early L2.** One core push run here, because 30 crates depend on the contract crate and a break must be found before three lanes build on it.

## Wave B — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-B1 Composition runtime** | template | A new `extensions/harness/device/harness-composition/**` (destined for core in S4) | `HarnessComposition`: `with`, `without`, `declare`, `admit`, `run`; roles `desktop`, `run`, `serve`; state built in dependency order; slots built; admission re-run on a new manifest |
| **S3-B2 Engine members** | template | `extensions/harness/engine/**` | Each engine crate gains `src/extension.rs`. Every built-in tool, agent, command, prompt section, instruction source and skill source becomes a contribution. `builtin()` is deleted |
| **S3-B3 Device members** | template | `extensions/harness/device/sp-*/**` | Governor, accounts, sync, projection and Drive as members with core standing. Settings blocks typed. Background work declared as tasks |

**Wave gate (L1).** Offline check of `engine` and `device`.

## Wave C — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-C1 Shell members** | template | `extensions/harness/shell/**`, `extensions/harness/shared/**` | The shell owns a view slot and the closed mount-point list. Existing views become contributions. Strings become a bundle. `Services::start` and the untyped map are deleted; the shell resolves handles from built state. `main` becomes one line |
| **S3-C2 Brand and theme** | template | `extensions/harness/brand/**`, `extensions/harness/shared/src/theme/**` | `BrandDecl` replaces the enum. Themes are `ThemeDecl` data, contrast-checked at declare. Packaging reads `package --describe` |
| **S3-C3 Slices and an outside member** | template | Workspace feature tables (through the integrator); `extensions/harness/examples/outside-member/**` | Slices as features. A sample member crate, outside every kernel crate, contributing one tool, one view at a mount point and one theme: the proof that the sink is sufficient and that a type-erased contributed view works with the pinned toolkit |

**Wave gate (L1).** Offline check of what compiles on Linux; the rest at L2.

## Stage gate (L2)

- Every S1 and S2 check, unchanged, on all three systems. No expected trace edited.
- New: a composition-boot test per windowless form the lifted code can express (`run`, `serve` without a window); the outside member's tool runs in a golden scenario; its view and theme appear on the screenshot tour.
- The harness composes itself only through the sink: a gate fails on a kernel crate naming a member crate.

## Exit criteria

1. Stage gate green at one commit.
2. `builtin()`, `Services::start`, the brand enum and the untyped extensions map no longer exist.
3. The outside member builds without editing any kernel crate.
4. `architecture/03-composition-contract.md` matches the signatures that landed.

## Decisions reserved to people

- The neutral contract's shape (feature split, or the fallback), agreed with whoever owns core's `future` branch before S3-A1 starts.
- Merging the programme branch into `future`.
