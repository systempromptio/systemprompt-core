# Harness: future architecture and implementation stages

> **Not current functionality.** Everything under `documentation/future/` describes
> work that is designed but not built. Nothing here is shipped, and nothing here
> has been built or run. For what the platform does today, see
> [`documentation/`](../../README.md).

This directory is the technical specification and the staged implementation
sequence for making the desktop harness a first-class part of the platform and
retiring the bridge into it. It is written so that several people or coding
agents can work on it at the same time without colliding.

## The intended state

1. **Two tiers only: core and the extension.**
2. **Core** holds the wire contract, the device composition machinery and every governed device capability, in eight crates, each capability an opt-in feature. Core links no window toolkit.
3. **The extension** is the single build site for any device product: a one-line `main`, a composition, the window and views, brand, theme, packaging and the product's own members. It lives in the template (MIT) and is forked by installs, with the layout installs already use.
4. **The bridge is retired.** Its non-GUI code becomes core device crates; its window is rewritten as native views in the extension; the no-window composition keeps its product name. Core's release stops building, testing and signing a desktop binary.
5. **The standalone harness repository is dissolved**: engine and governance into core, shell into the template.
6. **One runtime, three typed execution modes.** The loop and its tools are implemented once in core and run `Unrestricted` or `Sandboxed` on a desktop, or `Remote` in the instance's sandbox. The instance is the record of every session in every mode, so every client sees them all.

## Layout

| Path | Purpose |
|---|---|
| [`stages/overview.md`](stages/overview.md) | **Start here to do work.** Stages, waves, the crate list, the dependency graph, what runs in parallel |
| [`stages/operating-model.md`](stages/operating-model.md) | How a lane works, what it owns, and the three validation levels |
| `stages/s0-…` to `stages/s9-…` | One file per stage: waves, lanes, owned paths, gates, exit criteria |
| [`testing/`](testing/) | The deterministic test strategy: scripted instance, golden scenarios, comparison against a reference agent, migrated test catalogue, evidence |
| [`architecture/`](architecture/) | The specification the stages implement |
| [`risks-and-decisions.md`](risks-and-decisions.md) | Risks with mitigations; every decision taken; the few still open |

### Architecture reference

| File | Answers |
|---|---|
| `01-current-state.md` | What exists today in the harness repository, the bridge, core `future` and the template |
| `02-target-architecture.md` | The two tiers, the eight crates and their layers, slices, every deployment form |
| `03-composition-contract.md` | The member trait, the sink, phases, standing, reserved shapes |
| `04-host-integrations.md` | Managing third-party coding agents; host identity and what a host-owned host is; `host_extensions`; orchestration |
| `05-bridge-split.md` | The bridge module by module: core, extension, or retired |
| `06-harness-repo-disposition.md` | The harness repository crate by crate |
| `07-template-extension.md` | `extensions/harness/*`; flavour; licence; what a fork owns |
| `08-platforms-and-packaging.md` | Windows, macOS, Linux; packaging and signing |
| `09-quality-and-gates.md` | Standards, gates, which core gates device crates meet and which are scoped, security review |
| `10-release-and-fleet-handover.md` | Release after the change; moving installed bridges |
| `11-execution-modes.md` | One loop in core; the `Unrestricted`, `Sandboxed` and `Remote` modes; the sandbox policy; the instance as the record of every session |
| `12-workbench-and-automation.md` | Parallel sessions with isolated working copies, the integrated terminal, the preview pane, scheduled sessions, memory |

Phone apps built from the same extension are specified in the sibling set, [`../mobile/`](../mobile/README.md).

## Founding decisions

| # | Decision |
|---|---|
| D1 | A harness is a type of extension. One extension point builds every device product |
| D2 | Two tiers only: core and the extension. No separate kernel repository |
| D3 | The bridge is deprecated and moved into the harness. Core stops building it |
| D4 | The bridge's code is split between real core crates on `future` and the extension |
| D5 | Bridge functions extend to other coding agents; the product grows into orchestration above the agents |
| D6 | One composition serves every form: governing individual agents, a custom harness, no harness, headless, locked-down machines. Slices are thin and opt-in |
| D7 | Standard views live in the template, forked, MIT, with the same architecture as existing installations |
| D8 | The harness repository is retired first, then the result is proven, then it is restructured |
| D9 | Testing is deterministic. No stage spends money on model inference. Parity is measured against an unmodified reference agent and against a catalogue of migrated tests |

D10 to D49 (crate list, publishing, contract placement, layout, naming, Linux, deprecation, evidence, signing, execution modes, and what S0 decided about the host, the lift and the reference pin) are in [`risks-and-decisions.md`](risks-and-decisions.md) section 3.

## Status

| Stage | State |
|---|---|
| S0 Foundations | Done. The eight crate skeletons, the closure gate, the scripted-instance interface, the template scaffold, both inventories and the lift's pin at `a214511` are on `future`. Host `harness` is on `future`; merging the host variant to `next` is a person's decision |
| S1 Lift | Code complete; stage gate green (core CI, Quality and Supply Chain at `78b09798e9`; the template's `Harness` workflow on Windows, macOS and Linux and its `Gates` at `9120a496`). On `future`: both imports at `a214511`; `systemprompt-engine`, `-device`, `-drive` and `-harness` compile on core's types and meet core's gates, with the module-order, no-window and foreign-name gates; 858 of the 877 lifted tests relocated (19 retired with reasons) and passing, on the native Windows and macOS job; the device reads a manifest tolerantly (unknown hosts and malformed ids skipped, everything else strict); `ExecutionSource::HookHarness` with mcp migration 022; the shell, views, theme, brand and shared crates in the template (one brand; the second exported as a patch), its tests a workspace of their own, 269 tests passing. Open: RUSTSEC-2026-0459 (bundled SQLite in the engine's store) is ignored with a reason until rusqlite can move; archiving the harness repository and handing the second brand to its install are a person's acts |
| S2 Parity and proof | In progress, waves A–D done on macOS; closing needs the first full run on real machines. On `future`: the scripted instance (sign-in, signed manifest, hooks, heartbeat, both model protocols, title reply, plans, stop and transport faults); the headless `run` and `login --stdin` (`systemprompt_harness::command`), dispatched by the template's `harness` binary; 24 golden scenarios, all matching on both protocols through the golden runner on macOS, Linux and a Windows 11 machine, each scenario's command taken in the dialect of the shell the harness detects; the comparator and parity ledger; the reference runner against OpenCode v2.4.2, its Linux parity job green with no credential and nothing outside loopback; migration batches 1–3 in core (`crates/tests/migrated/{tools,session,config,llm,mcp}`) and batch 4 in the template (`extensions/harness/tests/migrated/desktop/{process,session,app,composer,panels}`), each with a per-case `cases.md`; the screenshot tour and evidence scripts; the defects the tests found fixed (scanner depth, timeout kill, function bodies, revert on close, store first-open and file mode). Open: per-case counts to publish in `testing/04` and `testing/03`; open and manual cases left in the `cases.md` files; the tour has captured nothing yet (no machine with screen-recording and accessibility granted to the runner; the harness keychain prompt after a rebuild blocks `login --stdin`, which has no timeout); the evidence run with screenshots on the Windows 11 machine (in hand) and on a real macOS machine, which is a person's act |
| S3 Execution environment and sandbox | Not started |
| S4 Remote runtime and the session record | Not started |
| S5 Device composition | Not started |
| S6 Bridge split and integration | Not started |
| S7 Platforms, packaging, signing | Wave A in, opened early on disjoint paths: every target-system condition of `systemprompt-device`, `-engine`, `-machine` and `-harness` sits in a `platform` module behind one contract with Windows, macOS and Linux implementations (`scripts/lint-platform-confinement.py`, on the gate list); the Linux sealer keeps a sign-in between processes in the kernel keyring (O9), the Secret Service store is the `secret-service` feature; one instance per data root hands a second launch's arguments to the first (a named pipe on Windows, a Unix socket elsewhere). Not started: wave B (Linux headless, macOS shell, Linux shell packaging), wave C (after S6) |
| S8 Fleet handover and bridge removal | Not started |
| S9 Server half and orchestration | Not started |

Update this table in the commit that closes a stage.
