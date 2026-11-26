# S4. Device composition

**Goal.** The harness is a composition of members. Its `main` is one line; its features are slices; a member outside the kernel can add a tool, a view and a theme.

**Entry.** S3 closed. The S2 suites are the safety net: this stage changes how the harness is assembled and must not change what it does.

**Specification.** `../architecture/03-composition-contract.md`, `../architecture/02-target-architecture.md` sections 4 to 6.

## Wave A — contract, one lane then one review

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-A1 Shared machinery** | core | `crates/shared/provider-contracts/**`; `crates/shared/extension/**`; `systemprompt-harness/src/contract/**` | Move the composition machinery (slots, standing, typed state, provide and factory types, dependency ordering, withholding) down into `systemprompt-provider-contracts`, untangling its one reference to a database handle. `systemprompt-extension` re-exports it, so no server crate changes. In `systemprompt-harness`: `HarnessExtension`, `HarnessContributions`, the device reserved shapes, `BrandDecl` and `ThemeDecl` as data, mount-point ids, and the `harness.members` block with its admission rule |
| **S4-A2 Contract review** **(review)** | core | None | Reserved shapes cannot be claimed by an extension-standing member; admission withholds, never partially admits; `harness.members` can narrow and never widen; no server crate's public interface changed |

Serial: every other lane in this stage compiles against S4-A1.

**Wave gate (L1).** Offline check of core; `systemprompt-harness` links no database, HTTP-server or SQL-parser crate (the closure gate proves it).

**Early L2.** One core push run here, because 44 crates depend on the contract crate and a break must be found before three lanes build on it.

## Wave B — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-B1 Composition runtime** | core | `systemprompt-harness/src/composition/**` | `HarnessComposition`: `with`, `without`, `declare`, `admit`, `run`; roles `desktop`, `run`, `serve`; state built in dependency order; slots built; admission re-run on a new manifest |
| **S4-B2 Engine members** | core | `systemprompt-engine/**` | `src/extension.rs`. Every built-in tool, agent, command, prompt section, instruction source and skill source becomes a contribution to the server-side engine's composition; each device-executed tool also contributes its executor to the tool host. The built-in tool list function is deleted |
| **S4-B3 Device members** | core | `systemprompt-device/**`, `systemprompt-drive/**`, `systemprompt-harness/src/{governor,projection}/**` | Accounts, sync, projection, the tool host and Drive as members with core standing. The governor is not a device member: it runs in process on the server since S3. Settings blocks typed. Background work declared as tasks |

**Wave gate (L1).** Offline check of core.

## Wave C — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-C1 Shell members** | template | `extensions/harness/shell/**`, `extensions/harness/shared/**` | The shell owns a view slot and the closed mount-point list. Existing views become contributions. Strings become a bundle. `Services::start` and the untyped map are deleted; the shell resolves handles from built state. `main` becomes one line |
| **S4-C2 Brand and theme** | template | `extensions/harness/brand/**`, `extensions/harness/shared/src/theme/**` | `BrandDecl` replaces the enum. Themes are `ThemeDecl` data, contrast-checked at declare. Packaging reads `package --describe` |
| **S4-C3 Slices and an outside member** | core, template | Facade feature tables (through the integrator); `extensions/harness/examples/outside-member/**` | Slices as facade features; `harness::reference()`. A sample member crate, outside every kernel crate, contributing one tool, one view at a mount point and one theme: the proof that the sink is sufficient and that a type-erased contributed view works with the pinned toolkit |

**Wave gate (L1).** Offline check of what compiles on Linux; the rest at L2.

## Stage gate (L2)

- Every S1 and S2 check, unchanged, on all three systems. No expected trace edited.
- New: a composition-boot test per windowless form the lifted code can express (`run`, `serve` without a window); the outside member's tool runs in a golden scenario; its view and theme appear on the screenshot tour.
- The harness composes itself only through the sink: a gate fails on a kernel crate naming a member crate.

## Exit criteria

1. Stage gate green at one commit.
2. The built-in tool list function, the fixed service wiring, the brand enum and the untyped extensions map no longer exist.
3. The outside member builds without editing any kernel crate.
4. `architecture/03-composition-contract.md` matches the signatures that landed.

## Decisions reserved to people

- The timing of S4-A1 on `future`, agreed with whoever owns that branch: it touches two crates every server crate depends on.
- Merging the programme branch into `future`.
